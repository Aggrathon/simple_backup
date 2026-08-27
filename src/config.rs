/// This module contains the config object (including serialisation, deserialisation, and parsing command line arguments)
use std::fs::File;
use std::io::{Error, ErrorKind};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use path_absolutize::Absolutize;
use serde::{Deserialize, Serialize};

use crate::backup::BACKUP_FILE_EXTENSION;
use crate::parse_date;
use crate::parse_date::{create_backup_file_name, naive_now};
use crate::utils::{BackupIterator, default_dir, num_cpus};

pub const QUALITY_RANGE: RangeInclusive<u8> = 0u8..=22;

fn cpus_u32() -> u32 {
    num_cpus() as u32
}

fn true_bool() -> bool {
    true
}

fn twenty_u8() -> u8 {
    20
}

fn one_u32() -> u32 {
    1
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Config {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub regex: Vec<String>,
    pub output: PathBuf,
    #[serde(default = "true_bool")]
    pub incremental: bool,
    #[serde(default = "twenty_u8")]
    pub quality: u8,
    #[serde(default)]
    pub local: bool,
    #[serde(default = "cpus_u32")]
    pub threads: u32,
    #[serde(with = "parse_date", default)]
    pub time: Option<NaiveDateTime>,
    #[serde(default = "one_u32")]
    pub link_depth: u32,
    #[serde(skip)]
    pub origin: PathBuf,
}

impl Config {
    /// Create an empty config
    #[allow(unused)]
    pub fn new() -> Self {
        Config {
            include: vec![],
            exclude: vec![],
            regex: vec![],
            output: PathBuf::new(),
            incremental: true,
            quality: 20,
            local: false,
            threads: num_cpus() as u32,
            time: None,
            origin: PathBuf::new(),
            link_depth: 1,
        }
    }

    pub fn get_output(&self, home: bool) -> PathBuf {
        if !self.output.as_os_str().is_empty() {
            self.output.clone()
        } else if !self.origin.as_os_str().is_empty() {
            self.origin.clone()
        } else if home {
            default_dir()
        } else {
            PathBuf::from(".")
        }
    }

    /// Read a config from a yaml file
    pub fn read_yaml(path: PathBuf) -> std::io::Result<Self> {
        let reader = File::open(&path)?;
        let mut conf: Config =
            serde_yaml::from_reader(reader).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
        if !QUALITY_RANGE.contains(&conf.quality) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Compression Quality must be 0-22",
            ));
        };
        conf.origin = path;
        Ok(conf)
    }

    /// Write the config to a yaml file
    pub fn write_yaml<P: AsRef<Path>>(&mut self, path: P, time: bool) -> std::io::Result<()> {
        self.sort();
        let t = self.time;
        if !time {
            self.time = None;
        }
        let writer = File::create(path)?;
        let res = serde_yaml::to_writer(writer, &self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e));
        if !time {
            self.time = t;
        }
        res
    }

    /// Parse a yaml string to a config
    pub fn from_yaml<S: AsRef<str>>(yaml: S) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml.as_ref())
    }

    /// serialise the config as a yaml string
    pub fn as_yaml(&mut self) -> serde_yaml::Result<String> {
        self.sort();
        serde_yaml::to_string(&self)
    }

    pub fn sort(&mut self) {
        self.include.sort_unstable();
        self.exclude.sort_unstable();
        self.regex.retain(|s| !s.is_empty());
    }

    fn is_output_file(&self) -> bool {
        if let Some(n) = self.output.file_name() {
            return n.to_string_lossy().ends_with(BACKUP_FILE_EXTENSION);
        }
        false
    }

    /// Get the path for a new backup
    pub fn get_new_output(&self) -> PathBuf {
        if self.is_output_file() {
            self.output.clone()
        } else {
            self.get_dir().join(create_backup_file_name(naive_now()))
        }
    }

    pub fn get_dir(&self) -> PathBuf {
        let mut path = self.get_output(false);
        if path.is_file() {
            path = match path.parent() {
                Option::Some(p) => p.to_path_buf(),
                Option::None => PathBuf::from("."),
            };
        }
        if self.local || path.is_absolute() {
            path
        } else {
            match path.absolutize() {
                Ok(p) => p.to_path_buf(),
                Err(_) => PathBuf::from("."),
            }
        }
    }

    /// Iterate over old backups
    pub fn get_backups(&self) -> BackupIterator {
        if self.is_output_file() {
            BackupIterator::file(self.output.clone())
        } else {
            BackupIterator::dir(self.get_dir())
        }
    }

    pub fn add_default_ignores(&mut self) {
        let regexes = [
            r"[\\/]\$RECYCLE.BIN$",
            r"[\\/]Thumbs\.db$",
            r"[\\/]\.?[Tt]rash(-.*|es|ed)?$",
            r"\.?(Te?mp|te?mp|TE?MP)$",
            r"\.?~?locks?(\..*#)?$",
            r"~$",
            r"\.bak\d*$",
            r"([Ll]og|LOG)(\.|\d|s|txt|old|dat|html)*$",
            r"[\\/][Cc]rash(ed?s?| ?Reports?| ?[Pp]ad| ?_?[Dd]umps?|[pP]lan|\.dmp|\.mem|\.txt|\.log)*$",
            r"[Cc]ached?s?$",
            r"[\\/]\.DS_Store$",
            r"[Tt]elemetry",
            r"[\\/][Dd]umps?$",
            r"[\\/][Ss]entry$",
            r"[\\/][Dd]esktop\.ini$",
            r"\.part$",
            r"\.crdownload$",
        ];
        self.regex.extend(regexes.iter().map(|s| s.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::files::{FileCrawler, FileInfo};

    #[test]
    fn yaml() {
        let mut config = Config::new();
        config.add_default_ignores();
        let yaml = config.as_yaml().unwrap();
        let mut config2 = Config::from_yaml(&yaml).unwrap();
        let yaml2 = config2.as_yaml().unwrap();
        assert_eq!(config.include, config2.include);
        assert_eq!(config.exclude, config2.exclude);
        assert_eq!(config.regex, config2.regex);
        assert_eq!(config.output, config2.output);
        assert_eq!(config.incremental, config2.incremental);
        assert_eq!(config.quality, config2.quality);
        assert_eq!(config.local, config2.local);
        assert_eq!(config.time, config2.time);
        assert_eq!(yaml, yaml2);
    }

    #[test]
    fn default_ignores() -> std::io::Result<()> {
        let mut config = Config::new();
        config.add_default_ignores();
        let fc = FileCrawler::new(["src"], config.exclude, config.regex, false, 1)?;
        assert!(fc.check_path(&mut FileInfo::from("src/cash"), Some(true)));
        assert!(!fc.check_path(&mut FileInfo::from("src/cache"), Some(true)));
        Ok(())
    }
}
