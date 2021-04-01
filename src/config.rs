use std::{
    fs::File,
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
};

use chrono::NaiveDateTime;
use clap::{ArgMatches, Values};
use serde::{Deserialize, Serialize};

use crate::{
    parse_date,
    parse_date::{create_backup_file_name, naive_now},
    utils::BackupIterator,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub regex: Vec<String>,
    pub output: String,
    pub incremental: bool,
    pub quality: u32,
    pub local: bool,
    #[serde(with = "parse_date")]
    pub time: Option<NaiveDateTime>,
    #[serde(skip)]
    pub origin: Option<String>,
}

impl Config {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Config {
            include: vec![],
            exclude: vec![],
            regex: vec![],
            output: ".".to_string(),
            incremental: false,
            quality: 11,
            local: false,
            time: None,
            origin: None,
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
            origin: None,
        }
    }

    pub fn read_yaml<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let reader = File::open(&path)?;
        let mut conf: Config =
            serde_yaml::from_reader(reader).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
        conf.origin = Some(path.as_ref().to_string_lossy().to_string());
        Ok(conf)
    }

    pub fn write_yaml<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        self.sort();
        let writer = File::create(path)?;
        serde_yaml::to_writer(writer, &self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn from_yaml<S: AsRef<str>>(yaml: S) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml.as_ref())
    }

    pub fn to_yaml(&mut self) -> serde_yaml::Result<String> {
        self.sort();
        serde_yaml::to_string(&self)
    }

    pub fn sort(&mut self) {
        self.include.sort();
        self.exclude.sort();
    }

    pub fn get_output(&self) -> PathBuf {
        if self.output.ends_with(".tar.br") {
            PathBuf::from(&self.output)
        } else {
            Path::new(&self.output).join(create_backup_file_name(naive_now()))
        }
    }

    pub fn get_backups(&self) -> BackupIterator {
        if self.output.ends_with(".tar.br") {
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
