use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::File;
use std::io::Write;
use time::OffsetDateTime;

use crate::common::*;
use crate::terminal::*;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum TextAction {
    None,
    Insert {
        start: Coord,
        stop: Coord,
        text: Vec<String32>,
    },
    Remove {
        start: Coord,
        stop: Coord,
        text: Vec<String32>,
    },
    InsertLines {
        start: usize,
        stop: usize,
        lines: Vec<String32>,
    },
    RemoveLines {
        start: usize,
        stop: usize,
        lines: Vec<String32>,
    },
    InsertChar {
        pos: Coord,
        cha: char,
    },
    RemoveChar {
        pos: Coord,
        cha: char,
    },
    Composite {
        actions: Vec<TextAction>,
        name: String,
    },
}

impl fmt::Display for TextAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TextAction::*;
        let s = match self {
            None => "Original version".to_owned(),
            Insert {
                start,
                stop: _,
                text,
            } => {
                format!("Insert at {start:?}: {}", concat_lines(text))
            }
            Remove { start, stop, .. } => format!("Remove from {start:?} to {stop:?}"),
            InsertLines {
                start,
                stop: _,
                lines,
            } => format!("Insert at line {start}: {}", concat_lines(lines)),
            RemoveLines { start, stop, .. } => {
                if start == stop {
                    format!("Remove line {start}")
                } else {
                    format!("Remove lines {start} to {stop}")
                }
            }
            InsertChar { pos, cha } => format!("Insert {cha} at {pos:?}"),
            RemoveChar { pos, .. } => format!("Remove character at {pos:?}"),
            Composite { name, .. } => name.to_owned(),
        };
        write!(f, "{}", s)
    }
}

impl TextAction {
    fn inverse(&self) -> Self {
        use TextAction::*;
        match self {
            None => None,
            Insert { start, stop, text } => Remove {
                start: *start,
                stop: *stop,
                text: text.clone(),
            },
            Remove { start, stop, text } => Insert {
                start: *start,
                stop: *stop,
                text: text.clone(),
            },
            InsertLines { start, stop, lines } => RemoveLines {
                start: *start,
                stop: *stop,
                lines: lines.clone(),
            },
            RemoveLines { start, stop, lines } => InsertLines {
                start: *start,
                stop: *stop,
                lines: lines.clone(),
            },
            InsertChar { pos, cha } => RemoveChar {
                pos: *pos,
                cha: *cha,
            },
            RemoveChar { pos, cha } => InsertChar {
                pos: *pos,
                cha: *cha,
            },
            Composite { actions, name } => Composite {
                actions: actions.iter().rev().map(|act| act.inverse()).collect(),
                name: format!("Undo {}", name),
            },
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct UndoNode {
    action: TextAction,
    children: Vec<UndoNode>,
    time: time::OffsetDateTime,
}

impl UndoNode {
    pub fn empty_root() -> Self {
        Self {
            action: TextAction::None,
            children: Vec::new(),
            time: OffsetDateTime::now_utc(),
        }
    }

    pub fn new(action: TextAction) -> Self {
        Self {
            action,
            children: Vec::new(),
            time: OffsetDateTime::now_utc(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct History {
    root: UndoNode,
    pub location: Vec<usize>, // List of indices describing path in tree
    latest_leaf: Option<Vec<usize>>,
    pub scroll: usize,
    pub locked: bool,
    pub recording: Option<Vec<TextAction>>,
}

impl History {
    pub fn new() -> Self {
        Self {
            root: UndoNode::empty_root(),
            location: Vec::new(),
            latest_leaf: None,
            scroll: 1,
            locked: false,
            recording: None,
        }
    }

    pub fn from_save() -> Option<Self> {
        let result = std::fs::read_to_string("savefile");
        if let Ok(ser) = result {
            let hist = serde_json::from_str(&ser).unwrap();
            Some(hist)
        } else {
            None
        }
    }

    pub fn save(&self) {
        let ser = serde_json::to_string(&self).unwrap();
        let mut f = File::create("savefile").unwrap();
        f.write_all(ser.as_bytes()).unwrap();
    }

    pub fn add_node(&mut self, action: TextAction) {
        if self.locked {
            return;
        } else if let Some(record) = &mut self.recording {
            record.push(action);
            return;
        }
        let mut cur = &mut self.root;
        for index in &self.location {
            cur = &mut cur.children[*index];
        }
        let node = UndoNode::new(action);
        cur.children.push(node);
        let mut new_location = self.location.clone();
        new_location.push(cur.children.len() - 1);
        self.location = new_location;
        self.latest_leaf = None;
    }

    fn get_current_node(&self) -> &UndoNode {
        self.get_node(&self.location)
    }

    fn get_node(&self, indices: &Vec<usize>) -> &UndoNode {
        let mut cur = &self.root;
        for index in indices {
            cur = &cur.children[*index];
        }
        cur
    }

    pub fn undo(&mut self) -> Option<TextAction> {
        if !self.location.is_empty() {
            if self.latest_leaf == None {
                self.latest_leaf = Some(self.location.clone());
            }
            let action = self.get_current_node().action.inverse();
            self.location.pop();
            Some(action)
        } else {
            None
        }
    }

    fn goto_child(&mut self, i: usize) -> Option<TextAction> {
        if i < self.get_current_node().children.len() {
            self.location.push(i);
            Some(self.get_current_node().action.clone())
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<TextAction> {
        if let Some(leaf_path) = &self.latest_leaf {
            if leaf_path.len() > self.location.len() {
                return self.goto_child(leaf_path[self.location.len()]);
            }
        }
        None
    }

    fn goto_node(&mut self, new_loc: Vec<usize>) -> Vec<TextAction> {
        let mut actions = Vec::new();
        while !self.location.is_empty()
            && (self.location.len() > new_loc.len()
                || new_loc[..self.location.len() - 1] != self.location[..])
        {
            let action = self.undo().unwrap();
            actions.push(action);
        }
        while self.location.len() < new_loc.len() {
            println!("{:?}", self.location);
            println!("{:?}", new_loc);
            let action = self.goto_child(new_loc[self.location.len()]).unwrap();
            actions.push(action);
        }
        self.latest_leaf = None;
        actions
    }

    pub fn goto_row(&mut self, row: usize) -> Option<Vec<TextAction>> {
        let new_loc = self.node_at_row(row)?;
        Some(self.goto_node(new_loc))
    }

    fn node_at_row(&self, row: usize) -> Option<Vec<usize>> {
        let mut next_nodes = vec![(&self.root, Vec::new())]; //node, location
        let mut cur_row = 1;
        while !next_nodes.is_empty() && cur_row <= row {
            let mut next_time = i128::MAX;
            let mut node_index = 0;
            for (i, (node, _)) in (&next_nodes).iter().enumerate() {
                if node.time.unix_timestamp_nanos() < next_time {
                    node_index = i;
                    next_time = node.time.unix_timestamp_nanos();
                }
            }
            let node = next_nodes[node_index].0;
            let loc = next_nodes[node_index].1.clone();
            if cur_row == row {
                return Some(loc);
            }
            cur_row += 1;
            if node.children.is_empty() {
                if node_index != next_nodes.len() - 1 {
                    cur_row += 1;
                }
            } else {
                cur_row += node.children.len() - 1;
                for (i, child) in (&node.children).iter().enumerate().rev() {
                    let mut child_loc = loc.clone();
                    child_loc.push(i);
                    next_nodes.insert(node_index + 1, (&child, child_loc));
                }
            }
            next_nodes.remove(node_index);
        }
        None
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn start_record(&mut self) {
        if self.is_recording() {
            panic!("Was recording.")
        }
        self.recording = Some(vec![]);
    }

    pub fn stop_record(&mut self) {
        if let Some(records) = self.recording.take() {
            if records.is_empty() {
                return;
            }
            let first = records[0].clone();
            let line = match first {
                TextAction::InsertChar { pos, .. } => pos.0 as i32,
                TextAction::RemoveChar { pos, .. } => pos.0 as i32,
                _ => -1,
            };
            let action = TextAction::Composite {
                actions: records,
                name: format!("Edit text at line {line}"),
            };
            self.add_node(action);
        } else {
            panic!("Wasn't recording.")
        }
    }

    pub fn snip_record(&mut self) {
        if self.is_recording() {
            self.stop_record();
            self.start_record();
        }
    }
}

pub fn draw_history(hist: &History, surf: &impl Surface) {
    let mut next_nodes = vec![&hist.root];
    let mut row = 1;
    while !next_nodes.is_empty() {
        let mut next_time = i128::MAX;
        let mut node_index = 0;
        for (i, node) in (&next_nodes).iter().enumerate() {
            if node.time.unix_timestamp_nanos() < next_time {
                node_index = i;
                next_time = node.time.unix_timestamp_nanos();
            }
        }
        let node = next_nodes[node_index];
        let mut line_length = 0;
        if row >= hist.scroll {
            surf.goto(row - hist.scroll + 1, 1);
            for i in 0..next_nodes.len() {
                if i == node_index {
                    if node.children.len() == 0 {
                        print!("▶"); //└");
                    } else {
                        print!("▶"); //├");
                    }
                } else {
                    print!("│");
                }
                line_length += 1;
            }
            if node == hist.get_current_node() {
                surf.set_fg_color(Color::Cyan);
            }
            let message = format!("{}", node.action);
            let reduced = preview_lines(&split_text(&message), surf.cols() - line_length - 1);
            print!(" {}", reduced);
            surf.reset_colors();
        }
        if node.children.len() == 0 && node_index != next_nodes.len() - 1 {
            row += 1;
            if row >= hist.scroll {
                surf.goto(row - hist.scroll + 1, 1);
                for i in 0..next_nodes.len() {
                    if i == node_index {
                        print!("🮣");
                    } else if i == next_nodes.len() - 1 {
                        print!("🮠")
                    } else if i > node_index {
                        print!("🮨");
                    } else {
                        print!("│");
                    }
                }
            }
        } else if node.children.len() > 1 {
            for j in 1..node.children.len() {
                row += 1;
                if row >= hist.scroll {
                    surf.goto(row - hist.scroll + 1, 1);
                    for i in 0..next_nodes.len() + j {
                        if i == next_nodes.len() + j - 1 {
                            print!("🮢");
                        } else if i > node_index {
                            print!("🮩");
                        } else if i == node_index {
                            print!("├");
                        } else {
                            print!("│")
                        }
                    }
                }
            }
        }
        //print!("{:?}", node.time.elapsed());
        row += 1;
        if row >= hist.scroll + surf.rows() {
            break;
        }
        for child in (&node.children).iter().rev() {
            next_nodes.insert(node_index + 1, &child);
        }
        next_nodes.remove(node_index);
    }
}
