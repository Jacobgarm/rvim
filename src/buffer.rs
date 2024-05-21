use std::fmt;
use std::fs::File;
use std::io::prelude::*;

use crate::common::*;
use crate::terminal::*;
use crate::undo::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
}

impl Mode {
    pub fn get_colors(&self) -> (Color, Color) {
        match self {
            Mode::Normal => (Color::Magenta, Color::Red),
            Mode::Insert => (Color::Blue, Color::Cyan),
            Mode::Visual => (Color::Yellow, Color::White),
            Mode::VisualLine => (Color::Yellow, Color::White),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Mode::Normal => "Normal",
                Mode::Insert => "Insert",
                Mode::Visual => "Visual",
                Mode::VisualLine => "Visual-Line",
            }
        )
    }
}

pub struct Buffer {
    pub contents: Vec<String32>,
    pub file: File,
    pub clip: Vec<String32>,
    pub clip_lines: bool,
    pub scroll: Coord,
    pub cursor: Coord,
    pub cursor_col_goal: usize,
    pub selection_start: Coord,
    pub mode: Mode,
    pub history: History,
    pub show_history: bool,
}

impl Buffer {
    pub fn from_file(mut file: File, undofile: bool) -> std::io::Result<Self> {
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let mut lines = split_text(&contents);
        lines.pop();

        let hist = if undofile {
            let result = History::from_save();
            result.unwrap_or(History::new())
        } else {
            History::new()
        };

        Ok(Self {
            contents: lines,
            file,
            clip: vec![Vec::new()],
            clip_lines: false,
            scroll: (1, 1),
            cursor: (1, 1),
            selection_start: (1, 1),
            cursor_col_goal: 1,
            mode: Mode::Normal,
            history: hist,
            show_history: false,
        })
    }

    pub fn set_mode(&mut self, mode: Mode) {
        use Mode::*;
        match (self.mode, mode) {
            (a, b) if a == b => return,
            (Insert, Normal) => {
                if self.cursor.1 > 1 {
                    self.cursor.1 -= 1;
                }
                if self.cursor_col_goal > 1 {
                    self.cursor_col_goal -= 1;
                }
                self.history.stop_record();
            }
            (Visual, Normal) => {
                if self.cursor.1 > self.contents[self.cursor.0 - 1].len() && self.cursor.1 > 1 {
                    self.cursor.1 -= 1;
                }
            }
            (Insert, Visual | VisualLine) => {
                self.history.stop_record();
                self.selection_start = self.cursor;
            }

            (Normal, Visual | VisualLine) => {
                self.selection_start = self.cursor;
            }
            (_, Insert) => {
                self.history.start_record();
            }
            _ => (),
        }

        self.mode = mode;
    }
}
