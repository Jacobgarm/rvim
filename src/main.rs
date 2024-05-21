use std::cmp::{max, min};
use std::env;
use std::io::prelude::*;
use std::io::{stdin, stdout};
use std::process::{Command, Stdio};
use std::sync::RwLock;

use lazy_static::lazy_static;

use notify::{RecursiveMode, Watcher};

use wl_clipboard_rs::copy::{self, Options, Source};
use wl_clipboard_rs::paste::{self, get_contents, ClipboardType, Seat};

use termion::event::{Event, Key, MouseButton, MouseEvent};
use termion::input::TermRead;

mod common;
use common::*;
mod terminal;
use terminal::*;
mod config;
use config::*;
mod buffer;
use buffer::*;
mod undo;
use undo::*;

lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new({
        if let Some(conf) = Config::from_file() {
            conf
        } else {
            Config::default()
        }
    });
    static ref LOG: RwLock<String> = RwLock::new(String::new());
}

#[allow(unused_macros)]
macro_rules! log {
    ($($t:tt)*) => {
        let s = &format!($($t)*);
        let mut log = LOG.write().unwrap();
        (*log).push_str(s);
        (*log).push_str("\n\r");
    };
}

struct Process {
    buffers: Vec<Buffer>,
    active_buffer: usize,
}

impl Process {
    fn get_active_buffer(&mut self) -> &mut Buffer {
        &mut self.buffers[self.active_buffer]
    }
}

fn up(buf: &mut Buffer, term: &Terminal, n: usize) {
    if buf.cursor.0 > 1 {
        buf.cursor.0 -= min(n, buf.cursor.0 - 1);

        update_cursor(buf, term);
        buf.history.snip_record();
    }
}

fn down(buf: &mut Buffer, term: &Terminal, n: usize) {
    if buf.cursor.0 < buf.contents.len() {
        buf.cursor.0 = min(buf.contents.len(), buf.cursor.0 + n);

        update_cursor(buf, term);
        buf.history.snip_record();
    }
}
fn left(buf: &mut Buffer, term: &Terminal, n: usize) {
    if buf.cursor.1 > 1 {
        buf.cursor.1 -= min(n, buf.cursor.1 - 1);
        buf.cursor_col_goal = buf.cursor.1;
        update_scroll(buf, term);
        buf.history.snip_record();
    }
}
fn right(buf: &mut Buffer, term: &Terminal, n: usize) {
    let mx = buf.contents[buf.cursor.0 - 1].len() + if buf.mode == Mode::Normal { 0 } else { 1 };
    if buf.cursor.1 < mx {
        buf.cursor.1 = min(buf.cursor.1 + n, mx);
        buf.cursor_col_goal = buf.cursor.1;
        update_scroll(buf, term);
        buf.history.snip_record();
    }
}

fn update_cursor(buf: &mut Buffer, term: &Terminal) {
    if buf.cursor.0 > buf.contents.len() {
        buf.cursor.0 = buf.contents.len();
    }

    let mx = max(
        buf.contents[buf.cursor.0 - 1].len() + if buf.mode == Mode::Normal { 0 } else { 1 },
        1,
    );
    buf.cursor.1 = min(buf.cursor_col_goal, mx);
    update_scroll(buf, term)
}
fn update_scroll(buf: &mut Buffer, term: &Terminal) {
    if buf.cursor.0 > 10 && buf.cursor.0 < 10 + buf.scroll.0 {
        buf.scroll.0 = max(buf.cursor.0 - 10, 1);
    } else if buf.cursor.0 > buf.scroll.0 + term.rows() - 12 {
        buf.scroll.0 = buf.cursor.0 + 12 - term.rows();
    }

    if buf.scroll.1 >= buf.cursor.1 {
        buf.scroll.1 = max(buf.cursor.1 - 1, 1);
    } else if buf.scroll.1 + term.cols() - 7 <= buf.cursor.1 {
        buf.scroll.1 = buf.cursor.1 + 8 - term.cols();
    }
}

fn redraw(buffer: &Buffer, term: &mut Terminal) {
    term.reset_colors();
    term.clear();

    if buffer.show_history {
        let hist_surface = Window {
            parent: &term,
            rect: Rect {
                top: 1,
                left: 1,
                bottom: term.rows() - 2,
                right: 38,
            },
        };
        draw_history(&buffer.history, &hist_surface);
        let sep = Rect {
            top: 1,
            left: 39,
            bottom: term.rows() - 2,
            right: 39,
        };
        draw_fill(sep, Color::Gray, term);
    }

    let buf_surface = Window {
        parent: &term,
        rect: Rect {
            top: 1,
            left: if buffer.show_history { 40 } else { 1 },
            bottom: term.rows() - 2,
            right: term.cols(),
        },
    };

    draw_contents(buffer, &buf_surface);

    let rect = Rect {
        top: buffer.cursor.0 + 2,
        left: buffer.cursor.1 + 8,
        bottom: buffer.cursor.0 + 4,
        right: buffer.cursor.1 + 30,
    };
    let text = "Hi! This is a popup.".to_owned();
    //draw_popup(rect, text, term);

    draw_status(buffer, term);

    //term.goto(term.rows(), 1);
    //term.reset_colors();
    //print!("{}", preview_lines(&buffer.clip, 50));
    draw_cursor(buffer, &buf_surface);
    term.reset_colors();
    term.flush();
}

fn draw_fill(bx: Rect, color: Color, surf: &impl Surface) {
    surf.set_bg_color(color);
    for row in bx.top..=bx.bottom {
        surf.goto(row, bx.left);
        for _ in bx.left..=bx.right {
            print!(" ");
        }
    }
    surf.reset_colors();
}

fn draw_text_box(rect: Rect, text: String, term: &mut Terminal) {
    let mut row = rect.top;
    for line in text.split('\n') {
        term.goto(row, rect.left);
        print!("{}", line);
        row += 1;
    }
}

fn draw_popup(rect: Rect, text: String, term: &mut Terminal) {
    term.goto(rect.top, rect.left);
    print!("╭");
    for _ in 0..rect.width() - 2 {
        print!("─");
    }
    print!("╮");

    for row in rect.top + 1..=rect.bottom - 1 {
        term.goto(row, rect.left);
        print!("│");
        for _ in 0..rect.width() - 2 {
            print!(" ");
        }
        print!("│");
    }

    term.goto(rect.bottom, rect.left);
    print!("╰");
    for _ in 0..rect.width() - 2 {
        print!("─");
    }
    print!("╯ ");
    let inner_rect = Rect {
        top: rect.top + 1,
        left: rect.left + 1,
        bottom: rect.bottom - 1,
        right: rect.right - 1,
    };
    draw_text_box(inner_rect, text, term);
}

fn draw_contents<S>(buffer: &Buffer, surf: &S)
where
    S: Surface,
{
    let mut cur_content_line = buffer.scroll.0;
    let mut cur_screen_line = 1;
    let mut in_selection = false;
    let (sel_start, sel_stop) = selected_bounds(buffer);

    if cur_content_line > sel_start.0 && cur_content_line <= sel_stop.0 {
        in_selection = true;
    }

    loop {
        let number = if CONFIG.read().unwrap().relative_number {
            if cur_content_line < buffer.cursor.0 {
                buffer.cursor.0 - cur_content_line
            } else if cur_content_line == buffer.cursor.0 {
                cur_content_line
            } else {
                cur_content_line - buffer.cursor.0
            }
        } else {
            cur_content_line
        };

        surf.goto(cur_screen_line, 1);
        surf.reset_colors();
        if cur_content_line != buffer.cursor.0 {
            surf.set_fg_color(Color::Gray);
        }
        print!("{:>5} ", number);
        surf.reset_colors();
        let line = &buffer.contents[cur_content_line as usize - 1];
        if buffer.mode == Mode::Visual {
            if line.len() < buffer.scroll.1 {
                if (
                    cur_content_line,
                    buffer.contents[cur_content_line - 1].len() + 1,
                ) == sel_start
                {
                    in_selection = true;
                }

                if in_selection {
                    surf.set_bg_color(Color::Gray);
                }
                print!(" ");
                if (
                    cur_content_line,
                    buffer.contents[cur_content_line - 1].len() + 1,
                ) == sel_stop
                {
                    in_selection = false;
                }
            } else {
                if in_selection {
                    surf.set_bg_color(Color::Gray);
                }
                for x in buffer.scroll.1 - 1..min(line.len() + 1, buffer.scroll.1 + surf.cols() - 7)
                {
                    if (cur_content_line, x + 1) == sel_start {
                        in_selection = true;
                        surf.set_bg_color(Color::Gray);
                    }
                    if x < line.len() {
                        print!("{}", &line[x]);
                    }
                    if (cur_content_line, x + 1) == sel_stop {
                        in_selection = false;
                        surf.reset_colors();
                    }
                }
            }
            surf.reset_colors();
        } else if buffer.mode == Mode::VisualLine {
            if cur_content_line == sel_start.0 {
                in_selection = true;
            }
            if in_selection {
                surf.set_bg_color(Color::Gray);
            }

            if line.len() >= buffer.scroll.1 {
                for x in buffer.scroll.1 - 1..min(line.len(), buffer.scroll.1 + surf.cols() - 7) {
                    print!("{}", &line[x]);
                }
            } else {
                print!(" ");
            }

            if cur_content_line == sel_stop.0 {
                in_selection = false;
            }
        } else if line.len() >= buffer.scroll.1 {
            for x in buffer.scroll.1 - 1..min(line.len(), buffer.scroll.1 + surf.cols() - 7) {
                print!("{}", &line[x]);
            }
        }
        cur_content_line += 1;
        cur_screen_line += 1;
        if cur_content_line > buffer.contents.len() || cur_screen_line > surf.rows() {
            break;
        }
    }
}

fn draw_status(buffer: &Buffer, term: &impl Surface) {
    term.goto(term.rows() - 1, 1);
    term.set_bg_color(Color::Gray);
    for _ in 0..term.cols() {
        print!(" ");
    }
    let (pri, sec) = buffer.mode.get_colors();
    term.goto(term.rows() - 1, 1);
    term.set_color(Color::Black, pri);
    print!(" {} ", buffer.mode);
    term.set_color(pri, sec);
    print!("");
    term.set_color(sec, Color::Gray);
    print!("");
    term.set_color(Color::Black, pri);
    let pos_str = format!(" {}:{} ", buffer.cursor.0, buffer.cursor.1);
    term.goto(term.rows() - 1, term.cols() - pos_str.len() + 1);
    print!("{}", pos_str);
}

fn draw_cursor(buffer: &Buffer, surf: &impl Surface) {
    surf.goto(
        buffer.cursor.0 - buffer.scroll.0 + 1,
        buffer.cursor.1 + 7 - buffer.scroll.1,
    );
    surf.bar(buffer.mode == Mode::Insert);
}

fn insert_key(buf: &mut Buffer, key: Key) {
    match key {
        Key::Char(cha) => {
            if cha == '\n' {
                buf.history.snip_record();
            }
            insert_char(buf, cha, buf.cursor);
        }
        Key::Backspace => {
            if buf.cursor.0 > 1 || buf.cursor.1 > 1 {
                let pos = if buf.cursor.1 == 1 {
                    (buf.cursor.0 - 1, buf.contents[buf.cursor.0 - 2].len() + 1)
                } else {
                    (buf.cursor.0, buf.cursor.1 - 1)
                };
                remove_char(buf, pos)
            }
            // if buf.cursor.1 > 1 {
            //     buf.contents[buf.cursor.0 - 1].remove(buf.cursor.1 - 2);
            //     buf.cursor.1 -= 1;
            // } else if buf.cursor.0 > 1 {
            //     buf.cursor.1 = buf.contents[buf.cursor.0 - 2].len() + 1;
            //     let mut tail = buf.contents.remove(buf.cursor.0 - 1);
            //     buf.contents[buf.cursor.0 - 2].append(&mut tail);
            //     buf.cursor.0 -= 1;
            // }
        }
        Key::Delete => {
            if buf.cursor.0 < buf.contents.len()
                || buf.cursor.1 <= buf.contents[buf.cursor.0 - 1].len()
            {
                remove_char(buf, buf.cursor)
            }

            // if buf.cursor.1 <= buf.contents[buf.cursor.0 - 1].len() {
            //     buf.contents[buf.cursor.0 - 1].remove(buf.cursor.1 - 1);
            // } else if buf.cursor.0 < buf.contents.len() - 1 {
            //     let mut tail = buf.contents.remove(buf.cursor.0);
            //     buf.contents[buf.cursor.0 - 1].append(&mut tail);
            // }
        }
        _ => (),
    }
    buf.cursor_col_goal = buf.cursor.1;
}

fn selected_bounds(buf: &Buffer) -> (Coord, Coord) {
    if buf.selection_start.0 < buf.cursor.0
        || (buf.selection_start.0 == buf.cursor.0 && buf.selection_start.1 <= buf.cursor.1)
    {
        (buf.selection_start, buf.cursor)
    } else {
        (buf.cursor, buf.selection_start)
    }
}

fn get_selected_text(buf: &Buffer) -> Vec<String32> {
    let (start, stop) = selected_bounds(buf);
    if buf.mode == Mode::VisualLine {
        get_lines(buf, start.0, stop.0)
    } else {
        get_text(buf, start, stop)
    }
}

fn get_text(buf: &Buffer, start: Coord, stop: Coord) -> Vec<String32> {
    if start.0 == stop.0 {
        let line = &buf.contents[start.0 - 1];
        let mut text = vec![line[start.1 - 1..min(stop.1, line.len())].to_owned()];
        if stop.1 > line.len() {
            text.push(Vec::new());
        }
        return text;
    }

    let mut strings = Vec::new();
    let first_line = &buf.contents[start.0 - 1];
    strings.push(first_line[start.1 - 1..].to_owned());

    for line in &buf.contents[start.0..stop.0 - 1] {
        strings.push(line.to_owned());
    }

    let last_line = &buf.contents[stop.0 - 1];
    strings.push(last_line[..min(stop.1, last_line.len())].to_owned());
    if stop.1 > last_line.len() {
        strings.push(Vec::new());
    }

    strings
}

fn get_lines(buf: &Buffer, start: usize, stop: usize) -> Vec<String32> {
    let mut lines = Vec::new();
    for line in &buf.contents[start - 1..stop] {
        lines.push(line.to_owned());
    }

    return lines;
}

fn yank_selected(buf: &mut Buffer) {
    buf.clip = get_selected_text(buf);
    buf.clip_lines = buf.mode == Mode::VisualLine;
    if CONFIG.read().unwrap().clipboard {
        let opts = Options::new();
        opts.copy(
            Source::Bytes(concat_lines(&buf.clip).into_bytes().into()),
            copy::MimeType::Autodetect,
        )
        .unwrap();
    };
}

fn remove_text(buf: &mut Buffer, start: Coord, stop: Coord) {
    let action = TextAction::Remove {
        start,
        stop,
        text: get_text(buf, start, stop),
    };
    buf.history.add_node(action);
    let last_line = &buf.contents[stop.0 - 1];
    let mut extra = 0;
    let mut tail = if stop.1 > last_line.len() {
        if stop.0 == buf.contents.len() {
            vec![]
        } else {
            extra = 1;
            buf.contents[stop.0].to_owned()
        }
    } else {
        last_line[min(stop.1, last_line.len())..].to_owned()
    };
    buf.contents[start.0 - 1].truncate(start.1 - 1);
    buf.contents[start.0 - 1].append(&mut tail);
    for _ in start.0..stop.0 + extra {
        buf.contents.remove(start.0);
    }
}

fn remove_lines(buf: &mut Buffer, start: usize, stop: usize) {
    let action = TextAction::RemoveLines {
        start,
        stop,
        lines: get_lines(buf, start, stop),
    };
    buf.history.add_node(action);

    for _ in start..=stop {
        buf.contents.remove(start - 1);
    }
    if buf.contents.is_empty() {
        buf.contents.push(String32::new());
    }
}

fn remove_char(buf: &mut Buffer, pos: Coord) {
    let action;
    if pos.1 <= buf.contents[pos.0 - 1].len() {
        action = TextAction::RemoveChar {
            pos,
            cha: buf.contents[pos.0 - 1][pos.1 - 1],
        };

        buf.contents[pos.0 - 1].remove(pos.1 - 1);
    } else {
        action = TextAction::RemoveChar { pos, cha: '\n' };

        let mut tail = buf.contents.remove(pos.0);
        buf.contents[pos.0 - 1].append(&mut tail);
    }
    buf.cursor = pos;
    buf.history.add_node(action);
}

fn remove_selected(buf: &mut Buffer) {
    let (start, stop) = selected_bounds(buf);
    if buf.mode == Mode::VisualLine {
        remove_lines(buf, start.0, stop.0);
        buf.cursor.0 = start.0;
    } else {
        remove_text(buf, start, stop);
        buf.cursor = start;
    }
    buf.cursor_col_goal = buf.cursor.1;
}

fn replace_selected(buf: &mut Buffer, text: &mut Vec<String32>) {
    let (start, _) = selected_bounds(buf);
    let text_len = text.len();
    remove_selected(buf);
    if buf.mode == Mode::Visual {
        insert_text(buf, text, start);
    } else {
        insert_lines(buf, text, start.0)
    }
    buf.cursor.0 = start.0 + text_len - 1;
    buf.cursor.1 = max(1, buf.contents[buf.cursor.0 - 1].len());
}

fn paste_clip(buf: &mut Buffer, after: bool) {
    if CONFIG.read().unwrap().clipboard {
        let result = get_contents(
            ClipboardType::Regular,
            Seat::Unspecified,
            paste::MimeType::Text,
        );
        if let Ok((mut pipe, _)) = result {
            let mut contents = Vec::new();
            pipe.read_to_end(&mut contents).unwrap();

            let string = String::from_utf8_lossy(&contents);
            let lines = split_text(&string);
            if lines != buf.clip {
                buf.clip = lines;
                buf.clip_lines = false;
            }
        }
    };

    let mut text = buf.clip.clone();
    if buf.clip_lines {
        let row = buf.cursor.0 + if after { 1 } else { 0 };
        buf.cursor.0 += text.len();
        insert_lines(buf, &mut text, row);
    } else {
        let col = buf.cursor.1
            + if after && buf.contents[buf.cursor.0 - 1].len() != 0 {
                1
            } else {
                0
            };
        insert_text(buf, &mut text, (buf.cursor.0, col));
    }
    buf.cursor_col_goal = buf.cursor.1;
}

fn insert_text(buf: &mut Buffer, text: &mut Vec<String32>, pos: Coord) {
    buf.cursor.0 += text.len() - 1;
    let history_text = text.clone();
    let first = buf.contents[pos.0 - 1][..pos.1 - 1].to_owned();
    let mut last = buf.contents[pos.0 - 1][pos.1 - 1..].to_owned();
    let last_row = pos.0 + text.len() - 2;

    if text.len() == 1 {
        let mut line = first;
        buf.cursor.1 += text[0].len();
        line.append(&mut text[0]);
        line.append(&mut last);
        buf.contents[pos.0 - 1] = line;
    } else {
        buf.cursor.1 = max(1, text[text.len() - 1].len());
        buf.contents[pos.0 - 1] = first;
        buf.contents[pos.0 - 1].append(&mut text.remove(0));
        for i in 1..text.len() + 1 {
            buf.contents.insert(pos.0 - 1 + i, text.remove(0));
        }
        buf.contents[last_row].append(&mut last);
    }
    let action = TextAction::Insert {
        start: pos,
        stop: buf.cursor,
        text: history_text,
    };
    buf.history.add_node(action);
}

fn insert_lines(buf: &mut Buffer, lines: &mut Vec<String32>, row: usize) {
    let action = TextAction::InsertLines {
        start: row,
        stop: row + lines.len() - 1,
        lines: lines.clone(),
    };
    buf.history.add_node(action);
    for _ in 0..lines.len() {
        buf.contents.insert(row - 1, lines.pop().unwrap());
    }
}

fn insert_char(buf: &mut Buffer, cha: char, pos: Coord) {
    let action = TextAction::InsertChar { pos, cha };
    buf.history.add_node(action);
    if cha == '\n' {
        let line = &buf.contents[pos.0 - 1];
        let tail = (&line[pos.1 - 1..]).to_owned();
        buf.contents[pos.0 - 1].truncate(pos.1 - 1);
        buf.contents.insert(pos.0, tail);
        buf.cursor = (buf.cursor.0 + 1, 1);
    } else {
        buf.contents[pos.0 - 1].insert(pos.1 - 1, cha);
        buf.cursor.1 += 1;
    }
}

fn do_text_action(buf: &mut Buffer, action: TextAction) {
    use TextAction::*;
    buf.history.locked = true;
    match action {
        None => (),
        Insert {
            start,
            stop: _,
            text,
        } => insert_text(buf, &mut text.clone(), start),

        Remove {
            start,
            stop,
            text: _,
        } => remove_text(buf, start, stop),
        InsertLines {
            start,
            stop: _,
            lines,
        } => insert_lines(buf, &mut lines.clone(), start),
        RemoveLines {
            start,
            stop,
            lines: _,
        } => remove_lines(buf, start, stop),
        InsertChar { pos, cha } => insert_char(buf, cha, pos),
        RemoveChar { pos, cha: _ } => remove_char(buf, pos),
        Composite { actions, name: _ } => {
            for action in actions {
                do_text_action(buf, action)
            }
        }
    }
    buf.history.locked = false;
}

fn handle_event(buf: &mut Buffer, term: &Terminal, evt: Event) -> bool {
    if let Event::Mouse(mevt) = evt {
        handle_mouse_event(buf, term, mevt);
        return false;
    }
    if evt == Event::Key(Key::Esc) {
        buf.set_mode(Mode::Normal);
        return false;
    }
    if buf.mode == Mode::Insert {
        if let Event::Key(key) = evt {
            match key {
                Key::Up => up(buf, term, 1),
                Key::Down => down(buf, term, 1),
                Key::Left => left(buf, term, 1),
                Key::Right => right(buf, term, 1),
                _ => {
                    insert_key(buf, key);
                    update_scroll(buf, term);
                }
            }
        }
        return false;
    } else if buf.mode == Mode::Visual || buf.mode == Mode::VisualLine {
        match evt {
            Event::Key(Key::Char('h')) => left(buf, term, 1),
            Event::Key(Key::Char('l')) => right(buf, term, 1),
            Event::Key(Key::Char('k')) => up(buf, term, 1),
            Event::Key(Key::Char('j')) => down(buf, term, 1),
            Event::Key(Key::Char('y')) => {
                yank_selected(buf);
                buf.set_mode(Mode::Normal);
            }
            Event::Key(Key::Char('d')) => {
                yank_selected(buf);
                remove_selected(buf);
                buf.set_mode(Mode::Normal);
                update_cursor(buf, term);
            }
            #[cfg(feature = "talculia")]
            Event::Key(Key::Char('c')) => {
                if buf.cursor.0 == buf.selection_start.0 {
                    let text = concat_lines(&get_selected_text(buf));
                    let context = talculia::Context::default();
                    let parsed = talculia::parse(&talculia::preparse(text)).unwrap();
                    let result = parsed.evaluate(&context);
                    let new_text = format!("{}", result);
                    let mut splitted = split_text(&new_text);
                    replace_selected(buf, &mut splitted);
                    buf.set_mode(Mode::Normal);
                    update_scroll(buf, term);
                }
            }
            Event::Key(Key::Char('e')) => {
                if buf.cursor.0 == buf.selection_start.0 {
                    let text = concat_lines(&get_selected_text(buf));
                    let python = Command::new("python")
                        .arg("-c")
                        .arg(format!("print({})", text))
                        .stdout(Stdio::piped())
                        .output()
                        .unwrap();
                    if !python.status.success() {
                        return false;
                    }
                    let mut new_text = std::str::from_utf8(&python.stdout).unwrap();
                    new_text = &new_text[..new_text.len() - 1];
                    let mut splitted = split_text(&new_text);
                    replace_selected(buf, &mut splitted);
                    buf.set_mode(Mode::Normal);
                    update_scroll(buf, term);
                }
            }
            _ => (),
        }
        return false;
    }
    match evt {
        Event::Key(Key::Char('q')) => return true,
        Event::Key(Key::Char('i')) => buf.set_mode(Mode::Insert),
        Event::Key(Key::Char('a')) => {
            buf.set_mode(Mode::Insert);
            right(buf, term, 1)
        }
        Event::Key(Key::Char('o')) => {
            buf.contents.insert(buf.cursor.0, String32::new());
            buf.cursor = (buf.cursor.0 + 1, 1);
            buf.set_mode(Mode::Insert);
        }
        Event::Key(Key::Char('O')) => {
            buf.contents.insert(buf.cursor.0 - 1, String32::new());
            buf.cursor = (buf.cursor.0, 1);
            buf.set_mode(Mode::Insert);
        }
        Event::Key(Key::Char('v')) => buf.set_mode(Mode::Visual),
        Event::Key(Key::Char('V')) => buf.set_mode(Mode::VisualLine),
        Event::Key(Key::Char('H')) => {
            buf.show_history = !buf.show_history;
        }
        Event::Key(Key::Char('h')) => left(buf, term, 1),
        Event::Key(Key::Char('l')) => right(buf, term, 1),
        Event::Key(Key::Char('k')) => up(buf, term, 1),
        Event::Key(Key::Char('j')) => down(buf, term, 1),
        Event::Key(Key::Char('x')) => {
            if buf.contents[buf.cursor.0 - 1].len() != 0 {
                remove_char(buf, buf.cursor);
            }
        }
        Event::Key(Key::Char('d')) => {
            buf.set_mode(Mode::VisualLine);
            yank_selected(buf);
            remove_selected(buf);
            buf.set_mode(Mode::Normal);
            update_cursor(buf, term);
        }
        Event::Key(Key::Char('y')) => {
            buf.set_mode(Mode::VisualLine);
            yank_selected(buf);
            buf.set_mode(Mode::Normal);
        }
        Event::Key(Key::Char('p')) => {
            paste_clip(buf, true);
            update_cursor(buf, term);
        }
        Event::Key(Key::Char('P')) => {
            paste_clip(buf, false);
            update_cursor(buf, term);
        }
        Event::Key(Key::Char('u')) => {
            let result = buf.history.undo();
            if let Some(action) = result {
                do_text_action(buf, action);
                update_cursor(buf, term)
            }
        }
        Event::Key(Key::Char('r')) => {
            let result = buf.history.redo();
            if let Some(action) = result {
                do_text_action(buf, action);
                update_cursor(buf, term)
            }
        }

        _ => (),
    }
    return false;
}

fn handle_mouse_event(buf: &mut Buffer, term: &Terminal, mevt: MouseEvent) {
    match mevt {
        MouseEvent::Press(MouseButton::Left, mcol, mrow) => {
            let mut hcol = mcol;
            if buf.show_history {
                if mcol <= 39 {
                    let row = mrow as usize + buf.history.scroll - 1;
                    let result = buf.history.goto_row(row);

                    buf.set_mode(Mode::Normal); // Done after retrieving actions since exiting
                                                // Insert mode may change undo tree

                    if let Some(actions) = result {
                        for action in actions {
                            do_text_action(buf, action);
                        }
                    }
                    update_cursor(buf, term);
                    return;
                }
                hcol -= 39;
            }
            if buf.mode == Mode::Visual {
                buf.set_mode(Mode::Normal);
            }
            buf.history.snip_record();
            let col = if hcol <= 6 {
                1
            } else {
                hcol as usize + buf.scroll.1 - 7
            };
            let row = mrow as usize + buf.scroll.0 - 1;
            buf.cursor = (row, col);
            buf.cursor_col_goal = col;
            update_cursor(buf, term)
        }
        MouseEvent::Press(MouseButton::WheelUp, _, _) => {
            buf.scroll.0 = max(4, buf.scroll.0) - 3;
            if buf.cursor.0 > buf.scroll.0 + term.rows() - 12 {
                buf.cursor.0 = buf.scroll.0 + term.rows() - 12;
            }
            update_cursor(buf, term);
            if buf.history.scroll > 1 {
                buf.history.scroll -= 1;
            }
        }
        MouseEvent::Press(MouseButton::WheelDown, _, _) => {
            buf.scroll.0 = min(buf.contents.len(), buf.scroll.0 + 3);
            if buf.cursor.0 < 10 + buf.scroll.0 {
                buf.cursor.0 = 10 + buf.scroll.0;
            }
            update_cursor(buf, term);
            buf.history.scroll += 1;
        }
        MouseEvent::Press(MouseButton::WheelLeft, _, _) => {
            buf.scroll.0 = min(buf.contents.len(), buf.scroll.0 + 3);
            if buf.cursor.0 < 10 + buf.scroll.0 {
                buf.cursor.0 = 10 + buf.scroll.0;
            }
            update_cursor(buf, term);
        }
        MouseEvent::Hold(mcol, mrow) => {
            buf.set_mode(Mode::Visual);

            let mut col = if mcol <= 6 {
                1
            } else {
                mcol as usize + buf.scroll.1 - 7
            };
            if buf.show_history {
                if col <= 40 {
                    return;
                }
                col -= 39;
            }
            let row = mrow as usize + buf.scroll.0 - 1;

            buf.cursor = (row, col);
            buf.cursor_col_goal = col;
            update_cursor(buf, term)
        }

        _ => (),
    }
}

fn write_buffer(buf: &mut Buffer) -> std::io::Result<()> {
    let mut file = &buf.file;
    file.set_len(0)?;
    file.rewind()?;
    let mut char_buf = [0; 4];
    for line in &buf.contents {
        for char in line {
            let s = char.encode_utf8(&mut char_buf);
            file.write_all(s.as_bytes())?;
        }
        file.write_all("\n".as_bytes())?;
    }
    if CONFIG.read().unwrap().undofile {
        buf.history.save();
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    println!("{:?}", split_text(""));
    let mut watcher = notify::recommended_watcher(|res| match res {
        Ok(_) => {
            let mut conf = CONFIG.write().unwrap();
            if let Some(new_conf) = Config::from_file() {
                *conf = new_conf;
            }
        }
        Err(e) => println!("watch error: {:?}", e),
    })
    .unwrap();

    watcher
        .watch(
            std::path::Path::new(&config_path()),
            RecursiveMode::NonRecursive,
        )
        .unwrap_or(());

    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        panic!("No file specified");
    }

    let undofile = CONFIG.read().unwrap().undofile;

    let mut buffers = Vec::new();
    for arg in &args[1..] {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(arg)?;

        let buffer = Buffer::from_file(file, undofile)?;
        buffers.push(buffer);
    }

    let mut process = Process {
        buffers,
        active_buffer: 0,
    };

    let stdin = stdin();
    let mut term = Terminal::from_stdout(stdout());
    print!("\x1b[?47h"); // Save terminal state
                         //print!("\x1b[s");
    redraw(&process.get_active_buffer(), &mut term);

    for c in stdin.events() {
        term.update_size();
        let evt = c.unwrap();
        if let Event::Key(Key::F(n)) = evt {
            process.active_buffer = min(n as usize - 1, process.buffers.len() - 1);
        }

        let mut buf = process.get_active_buffer();
        if buf.mode == Mode::Normal && evt == Event::Key(Key::Char('w')) {
            write_buffer(&mut buf)?;
        } else {
            let quit = handle_event(&mut buf, &term, evt);
            if quit {
                break;
            }
            redraw(&buf, &mut term);
        }
    }
    print!("\x1b[?47l"); // Restore terminal state
                         //print!("\x1b[u");
    term.goto(term.rows().into(), 1);
    let log = LOG.read().unwrap();
    if !log.is_empty() {
        print!("Logs:\n\r");
        print!("{}", log);
    }
    Ok(())
}
