use std::{
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
};

use crate::{compression::CompressionDecoder, config::Config, files::FileInfo, parse_date};

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
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "The backup is empty",
        ));
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

pub fn get_list_from_backup<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    let mut dec = CompressionDecoder::read(&path)?;
    let mut entries = dec.entries()?;
    let entry = entries.next();
    if entry.is_none() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "The backup is empty",
        ));
    }
    let entry = entries.next();
    if entry.is_none() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "The file list is missing",
        ));
    }
    let mut entry = entry.unwrap()?;
    if entry.0 != PathBuf::from("files.csv") {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "The second file of the backup is not the file list",
        ));
    }
    let mut s = String::new();
    entry.1.read_to_string(&mut s)?;
    Ok(s)
}

pub fn parse_file_list(
    list: &str,
) -> std::iter::Map<std::str::Lines<'_>, fn(&str) -> Result<FileInfo, &str>> {
    list.lines().map(|l| {
        let mut split = l.splitn(2, ',');
        let time = split.next().ok_or("File info is missing")?;
        let string = split.next().ok_or("Could not split at ','")?;
        Ok(FileInfo::new_str(string, parse_date::try_parse(time)?))
    })
}

// pub struct FileListIterator {
//     list: String,
// }

// impl Iterator for FileListIterator {
//     type Item = Result<FileInfo, String>;

//     fn next(&mut self) -> Option<Self::Item> {
//         self.list.lines()
//         todo!()
//     }
// }
