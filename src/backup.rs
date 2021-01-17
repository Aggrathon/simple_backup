use core::panic;

use chrono::NaiveDateTime;

use crate::config::Config;

#[allow(unused_variables)]
pub fn backup(config: &Config, dry: bool, time: NaiveDateTime) {
    dbg!(config);
    panic!("Backupping is not yet implemented");
}

pub fn get_previous_time<'a>(config: &Config, time: &str) -> NaiveDateTime {
    if !config.incremental {
        NaiveDateTime::from_timestamp(0, 0)
    } else if time == "" {
        panic!("Incremental backup is not implemented");
    } else {
        panic!("Incremental backup is not implemented");
        // TODO: NaiveDateTime::parse_from_str
    }
}

#[allow(unused_variables)]
pub fn restore(
    source: &str,
    output: &str,
    regex: Vec<&str>,
    all: bool,
    force: bool,
    verbose: bool,
    flatten: bool,
    threads: u32,
    dry: bool,
) {
    panic!("Restoring is not yet implemented");
}

#[allow(unused_variables)]
pub fn browse(source: &str, regex: Vec<&str>) {
    panic!("Browsing is not yet implemented");
}
