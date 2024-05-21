use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::prelude::*;

pub fn config_path() -> String {
    let home = env::var("HOME");
    let mut front = if let Ok(path) = home {
        path
    } else {
        "/home/jacob".to_owned()
    };
    front.push_str("/.config/rvim/config.toml");
    front
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Config {
    pub relative_number: bool,
    pub wrap: bool,
    pub tab_width: u8,
    pub undofile: bool,
    pub clipboard: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            relative_number: false,
            wrap: false,
            tab_width: 4,
            undofile: false,
            clipboard: false,
        }
    }
}

impl Config {
    pub fn from_file() -> Option<Self> {
        let io_result = File::open(config_path());
        let mut file = match io_result {
            Ok(f) => f,
            Err(..) => return None,
        };
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let s = contents.clone();
        let toml_result = toml::from_str::<Config>(&s);
        let config = match toml_result {
            Ok(conf) => conf,
            Err(..) => return None,
        };
        Some(config)
    }
}
