pub type String32 = Vec<char>;
pub type Coord = (usize, usize); // (row, col) = (y, x)

pub fn split_text(s: &str) -> Vec<String32> {
    s.split("\n").map(|s| s.chars().collect()).collect()
}

pub fn concat_lines(lines: &Vec<String32>) -> String {
    lines
        .iter()
        .map(|l| l.clone().into_iter().collect::<String>())
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn preview_lines(lines: &Vec<String32>, length: usize) -> String {
    let mut preview = String::new();

    for line in lines {
        preview.push_str(&line.clone().into_iter().collect::<String>());
        preview.push('|');
        if preview.len() > length {
            preview = preview[..length - 3].to_owned();
            preview.push_str("...");
            break;
        }
    }
    if preview.len() > 0 && &preview[preview.len() - 1..] == "|" {
        preview.pop();
    }
    preview
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rect {
    pub top: usize,
    pub left: usize,
    pub bottom: usize,
    pub right: usize,
}

impl Rect {
    fn size(&self) -> (usize, usize) {
        (self.height(), self.width())
    }

    pub fn height(&self) -> usize {
        self.bottom - self.top + 1
    }

    pub fn width(&self) -> usize {
        self.right - self.left + 1
    }

    fn contains(&self, pos: Coord) -> bool {
        self.top <= pos.0 && pos.0 <= self.bottom && self.left <= pos.1 && pos.1 <= self.right
    }

    fn to_relative(&self, pos: Coord) -> Coord {
        (pos.0 - self.top, pos.1 - self.left)
    }

    fn from_relative(&self, pos: Coord) -> Coord {
        (pos.0 + self.top, pos.1 + self.left)
    }
}
