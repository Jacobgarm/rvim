use lazy_static::lazy_static;
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::sync::RwLock;

use crate::log::*;

lazy_static! {
    pub static ref CONFIG: RwLock<Config> = RwLock::new({
        if let Some(conf) = Config::from_file() {
            conf
        } else {
            Config::default()
        }
    });
}

pub fn config_path() -> Option<String> {
    if let Ok(conf_path) = env::var("XDG_CONFIG_HOME") {
        Some(conf_path + "/rvim/config.toml")
    } else if let Ok(home_path) = env::var("HOME") {
        Some(home_path + "/.config/rvim/config.toml")
    } else {
        None
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Config {
    pub logging: bool,
    pub relative_number: bool,
    pub wrap: bool,
    pub tab_width: u8,
    pub undofile: bool,
    pub clipboard: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            logging: true,
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
        let io_result = File::open(config_path()?);
        let mut file = match io_result {
            Ok(f) => f,
            Err(..) => {
                println!("No config file found");
                return None;
            }
        };
        log!("Config file found");
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let s = contents.clone();
        let toml_result = toml::from_str::<Config>(&s);
        let config = match toml_result {
            Ok(conf) => conf,
            Err(err) => {
                log!("Config parsing error:\n{}", err);
                return None;
            }
        };
        log!("Config succesfuly parsed");
        Some(config)
    }
}
