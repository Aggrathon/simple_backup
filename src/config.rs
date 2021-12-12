/// This module contains the config object (including serialisation, deserialisation, and parsing command line arguments)
use std::fs::File;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use clap::{ArgMatches, Values};
use path_absolutize::Absolutize;
use serde::{Deserialize, Serialize};

use crate::parse_date;
use crate::parse_date::{create_backup_file_name, naive_now};
use crate::utils::{clamp, BackupIterator};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub regex: Vec<String>,
    pub output: String,
    pub incremental: bool,
    pub quality: i32,
    pub local: bool,
    pub threads: u32,
    #[serde(with = "parse_date")]
    pub time: Option<NaiveDateTime>,
    #[serde(skip)]
    pub origin: Option<String>,
}

impl Config {
    /// Create an empty config
    pub fn new() -> Self {
        Config {
            include: vec![],
            exclude: vec![],
            regex: vec![],
            output: ".".to_string(),
            incremental: true,
            quality: 21,
            local: false,
            threads: 4,
            time: None,
            origin: None,
        }
    }

    /// Create a config from commandline arguments
    pub fn from_args(args: &ArgMatches) -> Self {
        Config {
            include: args
                .values_of("include")
                .unwrap_or(Values::default())
                .map(|x| x.to_string())
                .collect(),
            exclude: args
                .values_of("exclude")
                .unwrap_or(Values::default())
                .map(|x| x.to_string())
                .collect(),
            regex: args
                .values_of("regex")
                .unwrap_or(Values::default())
                .map(|x| x.to_string())
                .collect(),
            output: args.value_of("output").unwrap_or(".").to_string(),
            incremental: args.is_present("incremental"),
            quality: match args
                .value_of("quality")
                .and_then(|v| Some(v.parse::<i32>().expect("Could not parse number")))
            {
                Some(i) => clamp(i, 1, 22),
                None => 21,
            },
            threads: match args
                .value_of("threads")
                .and_then(|v| Some(v.parse::<u32>().expect("Could not parse number")))
            {
                Some(i) => clamp(i, 1, num_cpus::get() as u32),
                None => 1,
            },
            local: args.is_present("local"),
            time: args
                .value_of("time")
                .and_then(|v| Some(parse_date::try_parse(v).expect("Could not parse time")))
                .unwrap_or(None),
            origin: None,
        }
    }

    pub fn set_quality(&mut self, quality: i32) {
        self.quality = clamp(quality, 1, 22);
    }

    pub fn set_threads(&mut self, threads: u32) {
        self.threads = clamp(threads, 1, num_cpus::get() as u32);
    }

    /// Read a config from a yaml file
    pub fn read_yaml<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let reader = File::open(&path)?;
        let mut conf: Config =
            serde_yaml::from_reader(reader).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
        conf.origin = Some(path.as_ref().to_string_lossy().to_string());
        Ok(conf)
    }

    /// Write the config to a yaml file
    pub fn write_yaml<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        self.sort();
        let writer = File::create(path)?;
        serde_yaml::to_writer(writer, &self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Parse a yaml string to a config
    pub fn from_yaml<S: AsRef<str>>(yaml: S) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml.as_ref())
    }

    /// serialise the config as a yaml string
    pub fn to_yaml(&mut self) -> serde_yaml::Result<String> {
        self.sort();
        serde_yaml::to_string(&self)
    }

    pub fn sort(&mut self) {
        self.include.sort_unstable();
        self.exclude.sort_unstable();
        self.regex.retain(|s| !s.is_empty());
    }

    /// Get the path for a new backup
    pub fn get_output(&self) -> PathBuf {
        if self.output.ends_with(".tar.zst") {
            PathBuf::from(&self.output)
        } else if self.output.len() == 0 {
            match self.origin.as_ref() {
                Some(p) => {
                    let p = PathBuf::from(p);
                    match p.parent() {
                        Some(p) => p.join(create_backup_file_name(naive_now())),
                        None => p.join(create_backup_file_name(naive_now())),
                    }
                }
                None => Path::new(".").join(create_backup_file_name(naive_now())),
            }
        } else {
            Path::new(&self.output).join(create_backup_file_name(naive_now()))
        }
    }

    pub fn get_dir(&self) -> PathBuf {
        let path = if self.output.ends_with(".tar.zst") {
            match PathBuf::from(&self.output).parent() {
                Some(p) => p.to_path_buf(),
                None => PathBuf::from("."),
            }
        } else if self.output.len() == 0 {
            match self.origin.as_ref() {
                Some(p) => match PathBuf::from(p).parent() {
                    Some(p) => p.to_path_buf(),
                    None => PathBuf::from("."),
                },
                None => PathBuf::from("."),
            }
        } else {
            PathBuf::from(&self.output)
        };
        if self.local {
            path
        } else if path.is_absolute() {
            path
        } else {
            match path.absolutize() {
                Ok(p) => p.to_path_buf(),
                Err(_) => PathBuf::new(),
            }
        }
    }

    /// Iterate over old backups
    pub fn get_backups(&self) -> BackupIterator {
        if self.output.ends_with(".tar.zst") {
            BackupIterator::exact(PathBuf::from(&self.output))
        } else {
            BackupIterator::timestamp(Path::new(&self.output))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn yaml() {
        let mut config = Config::new();
        let yaml = config.to_yaml().unwrap();
        let mut config2 = Config::from_yaml(&yaml).unwrap();
        let yaml2 = config2.to_yaml().unwrap();
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
