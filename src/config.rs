use std::{
    fs::File,
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
};

use chrono::{Local, NaiveDateTime};
use clap::{ArgMatches, Values};
use serde::{Deserialize, Serialize};

use crate::{
    backup::{Backup, BackupError},
    parse_date,
    utils::BackupIterator,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub regex: Vec<String>,
    pub output: String,
    pub name: String,
    pub verbose: bool,
    pub force: bool,
    pub incremental: bool,
    pub quality: u32,
    pub local: bool,
    #[serde(with = "parse_date")]
    pub time: Option<NaiveDateTime>,
}

impl Config {
    #[allow(dead_code)]
    fn new() -> Self {
        Config {
            include: vec![],
            exclude: vec![],
            regex: vec![],
            output: ".".to_string(),
            name: "backup".to_string(),
            verbose: false,
            force: false,
            incremental: false,
            quality: 11,
            local: false,
            time: None,
        }
    }

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
            name: args.value_of("name").unwrap_or("backup").to_string(),
            verbose: args.is_present("verbose"),
            force: args.is_present("force"),
            incremental: args.is_present("incremental"),
            quality: args
                .value_of("quality")
                .and_then(|v| Some(v.parse::<u32>().expect("Could not parse number")))
                .unwrap_or(11),
            local: args.is_present("local"),
            time: args
                .value_of("time")
                .and_then(|v| Some(parse_date::try_parse(v).expect("Could not parse time")))
                .unwrap_or(None),
        }
    }

    pub fn read_yaml(path: &str) -> std::io::Result<Self> {
        let reader = File::open(path)?;
        Ok(serde_yaml::from_reader(reader).map_err(|e| Error::new(ErrorKind::InvalidData, e))?)
    }

    pub fn write_yaml(&mut self, path: &str) {
        self.sort();
        let writer = File::create(path).expect("Could not create the config file");
        serde_yaml::to_writer(writer, &self).expect("Could not serialise config");
    }

    pub fn from_yaml<S: AsRef<str>>(yaml: S) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml.as_ref())
    }

    pub fn to_yaml(&mut self) -> String {
        self.sort();
        serde_yaml::to_string(&self).expect("Could not serialise config")
    }

    pub fn sort(&mut self) {
        self.include.sort();
        self.exclude.sort();
    }

    pub fn get_output(&self) -> PathBuf {
        if self.output.ends_with(".tar.br") {
            PathBuf::from(&self.output)
        } else {
            Path::new(&self.output).join(format!(
                "{}_{}.tar.br",
                self.name,
                Local::now().format("%Y-%m-%d_%H-%M-%S")
            ))
        }
    }

    pub fn get_previous(&self) -> BackupIterator {
        if self.output.ends_with(".tar.br") {
            BackupIterator::exact(PathBuf::from(&self.output))
        } else {
            BackupIterator::with_name(Path::new(&self.output), self.name.to_string())
        }
    }

    pub fn from_path<S: AsRef<str>>(path: S) -> Result<Config, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        if path.ends_with(".yml") {
            Ok(Config::read_yaml(path)?)
        } else {
            let path = PathBuf::from(path);
            if path.metadata()?.is_file() {
                Backup::read(path)?.get_config()
            } else {
                match BackupIterator::with_timestamp(&path).get_latest(true) {
                    None => Err(Box::new(BackupError::NoBackup(path.to_path_buf()))),
                    Some(path) => Backup::read(path)?.get_config(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn yaml() {
        let mut config = Config::new();
        let yaml = config.to_yaml();
        let mut config2 = Config::from_yaml(&yaml).unwrap();
        let yaml2 = config2.to_yaml();
        assert_eq!(config.include, config2.include);
        assert_eq!(config.exclude, config2.exclude);
        assert_eq!(config.regex, config2.regex);
        assert_eq!(config.output, config2.output);
        assert_eq!(config.name, config2.name);
        assert_eq!(config.verbose, config2.verbose);
        assert_eq!(config.force, config2.force);
        assert_eq!(config.incremental, config2.incremental);
        assert_eq!(config.quality, config2.quality);
        assert_eq!(config.local, config2.local);
        assert_eq!(config.time, config2.time);
        assert_eq!(yaml, yaml2);
    }
}
