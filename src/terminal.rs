use crate::common::*;
use std::io::{prelude::*, Stdout};
use std::process::{Command, Stdio};
use termion::input::MouseTerminal;
use termion::raw::{IntoRawMode, RawTerminal};

pub trait Surface {
    fn cols(&self) -> usize;
    fn rows(&self) -> usize;

    fn goto(&self, row: usize, col: usize) {
        print!("\x1b[{};{}H", row, col);
    }

    fn set_color(&self, fg: Color, bg: Color) {
        print!("\x1b[{};{}m", fg.fg_code(), bg.bg_code());
    }

    fn set_fg_color(&self, fg: Color) {
        print!("\x1b[{}m", fg.fg_code());
    }

    fn set_bg_color(&self, bg: Color) {
        print!("\x1b[{}m", bg.bg_code());
    }

    fn reset_colors(&self) {
        print!("\x1b[39;49m");
    }

    fn bar(&self, enabled: bool) {
        if enabled {
            print!("\x1b[\x35 q");
        } else {
            print!("\x1b[\x31 q");
        }
    }
}

pub struct Terminal {
    pub stdout: MouseTerminal<RawTerminal<Stdout>>,
    rows: usize,
    cols: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Gray,
    BrightMagenta,
}

impl Color {
    pub fn fg_code(self) -> u8 {
        use Color::*;
        match self {
            Black => 30,
            Red => 31,
            Green => 32,
            Yellow => 33,
            Blue => 34,
            Magenta => 35,
            Cyan => 36,
            White => 37,
            Gray => 90,
            BrightMagenta => 95,
        }
    }

    pub fn bg_code(self) -> u8 {
        self.fg_code() + 10
    }
}

impl Terminal {
    pub fn from_stdout(stdout: Stdout) -> Self {
        let mut term = Terminal {
            stdout: MouseTerminal::from(stdout.into_raw_mode().unwrap()),
            cols: 0,
            rows: 0,
        };
        term.update_size();
        term
    }

    pub fn flush(&mut self) {
        self.stdout.flush().unwrap();
    }

    fn get_size() -> (usize, usize) {
        let output = Command::new("stty")
            .arg("-F")
            .arg("/dev/stderr")
            .arg("size")
            .stderr(Stdio::inherit())
            .output()
            .unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        let mut data = stdout.split_whitespace();
        let rs = data.next().unwrap().parse::<usize>().unwrap();
        let cs = data.next().unwrap().parse::<usize>().unwrap();
        (rs, cs)
    }

    pub fn update_size(&mut self) {
        let size = Terminal::get_size();
        self.rows = size.0;
        self.cols = size.1;
    }

    pub fn clear(&self) {
        print!("\x1b[2J");
    }
}

impl Surface for Terminal {
    fn cols(&self) -> usize {
        self.cols
    }

    fn rows(&self) -> usize {
        self.rows
    }
}

pub struct Window<'a> {
    pub parent: &'a Terminal,
    pub rect: Rect,
}

impl Surface for Window<'_> {
    fn cols(&self) -> usize {
        self.rect.width()
    }

    fn rows(&self) -> usize {
        self.rect.height()
    }

    fn goto(&self, row: usize, col: usize) {
        print!(
            "\x1b[{};{}H",
            row + self.rect.top - 1,
            col + self.rect.left - 1
        );
    }
}
