use std::{
    fs::{File, ReadDir},
    path::{Path, PathBuf},
};

use chrono::{Local, NaiveDateTime};
use clap::{ArgMatches, Values};
use serde::{Deserialize, Serialize};

use crate::utils::parse_date;

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

    pub fn read_yaml(path: &str) -> Self {
        let reader = File::open(path).expect("Could not open the config file");
        serde_yaml::from_reader(reader).expect("Could not read config file")
    }

    pub fn write_yaml(&mut self, path: &str) {
        self.sort();
        let writer = File::create(path).expect("Could not create the config file");
        serde_yaml::to_writer(writer, &self).expect("Could not serialise config");
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
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

    pub fn get_previous(&self) -> PreviousIterator {
        if self.output.ends_with(".tar.br") {
            PreviousIterator {
                output: Some(self.get_output()),
                error: None,
                dir: None,
                pattern: String::new(),
            }
        } else {
            match Path::new(&self.output).read_dir() {
                Err(e) => PreviousIterator {
                    output: None,
                    error: Some(e),
                    dir: None,
                    pattern: String::new(),
                },
                Ok(d) => PreviousIterator {
                    output: None,
                    error: None,
                    dir: Some(d),
                    pattern: [self.name.as_str(), "_%Y-%m-%d_%H-%M-%S.tar.br"]
                        .iter()
                        .map(|s| *s)
                        .collect(),
                },
            }
        }
    }
}

pub struct PreviousIterator {
    output: Option<PathBuf>,
    error: Option<std::io::Error>,
    dir: Option<ReadDir>,
    pattern: String,
}

impl Iterator for PreviousIterator {
    type Item = std::io::Result<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(e) = std::mem::replace(&mut self.error, None) {
            Some(Err(e))
        } else if let Some(p) = std::mem::replace(&mut self.output, None) {
            if p.exists() {
                Some(Ok(p))
            } else {
                Some(Err(p.metadata().unwrap_err()))
            }
        } else if let Some(dir) = &mut self.dir {
            for entry in dir {
                if entry.is_err() {
                    return Some(entry.map(|e| e.path()));
                }
                let entry = entry.unwrap();
                if NaiveDateTime::parse_from_str(
                    &entry.file_name().to_string_lossy(),
                    &self.pattern,
                )
                .is_ok()
                {
                    return Some(Ok(entry.path()));
                }
            }
            None
        } else {
            None
        }
    }
}
