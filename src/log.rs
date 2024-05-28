use lazy_static::lazy_static;
use std::sync::RwLock;

lazy_static! {
    pub static ref LOG: RwLock<String> = RwLock::new(String::new());
}

macro_rules! log {
    ($($t:tt)*) => {{
        let s = &format!($($t)*);
        let mut log = LOG.write().unwrap();
        (*log).push_str(s);
        (*log).push_str("\n\r");
        //drop(log);
    }};
}
pub(crate) use log;

pub fn print_log() {
    let log = LOG.read().unwrap();
    if !log.is_empty() {
        print!("Logs:\n\r");
        print!("{}", log);
    }
}
