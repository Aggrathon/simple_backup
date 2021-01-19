use std::{
    io::ErrorKind,
    io::Read,
    path::{Path, PathBuf},
};

use crate::{compression::CompressionDecoder, config::Config};

#[allow(unused_variables)]
pub fn restore(
    source: &str,
    output: &str,
    regex: Vec<&str>,
    all: bool,
    force: bool,
    verbose: bool,
    flatten: bool,
    dry: bool,
) {
    panic!("Restoring is not implemented");
}

#[allow(unused_variables)]
pub fn browse(source: &str, regex: Vec<&str>) {
    panic!("Browsing is not implemented");
}

pub fn get_config_from_backup<P: AsRef<Path>>(path: P) -> std::io::Result<Config> {
    let mut dec = CompressionDecoder::read(&path)?;
    let mut entries = dec.entries()?;
    let entry = entries.next();
    if entry.is_none() {
        return Err(std::io::Error::new(ErrorKind::Other, "The backup is empty"));
    }
    let mut entry = entry.unwrap()?;
    if entry.0 != PathBuf::from("config.yml") {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "The backup does not start with a config file",
        ));
    }
    let mut s = String::new();
    entry.1.read_to_string(&mut s)?;
    Config::from_yaml(&s).map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))
}
