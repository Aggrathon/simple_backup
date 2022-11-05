/// This module contains the config object (including serialisation, deserialisation, and parsing command line arguments)
use std::fs::File;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use path_absolutize::Absolutize;
use serde::{Deserialize, Serialize};

use crate::backup::BACKUP_FILE_EXTENSION;
use crate::parse_date;
use crate::parse_date::{create_backup_file_name, naive_now};
use crate::utils::default_dir;
use crate::utils::{clamp, BackupIterator};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Config {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub regex: Vec<String>,
    pub output: PathBuf,
    pub incremental: bool,
    pub quality: i32,
    pub local: bool,
    pub threads: u32,
    #[serde(with = "parse_date")]
    pub time: Option<NaiveDateTime>,
    #[serde(skip)]
    pub origin: PathBuf,
}

impl Config {
    /// Create an empty config
    pub fn new() -> Self {
        Config {
            include: vec![],
            exclude: vec![],
            regex: vec![],
            output: PathBuf::new(),
            incremental: true,
            quality: 21,
            local: false,
            threads: 4,
            time: None,
            origin: PathBuf::new(),
        }
    }

    pub fn set_quality(&mut self, quality: i32) {
        self.quality = clamp(quality, 1, 22);
    }

    pub fn set_threads(&mut self, threads: u32) {
        self.threads = clamp(threads, 1, num_cpus::get() as u32);
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
                Some(p) => p.to_path_buf(),
                None => PathBuf::from("."),
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
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn yaml() {
        let mut config = Config::new();
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
}
