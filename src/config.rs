use clap::{ArgMatches, Values};
use serde::{Deserialize, Serialize};
use std::fs::File;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub regex: Vec<String>,
    pub output: String,
    pub name: String,
    pub threads: u32,
    pub verbose: bool,
    pub force: bool,
    pub incremental: bool,
}

impl Config {
    #[allow(dead_code)]
    fn new() -> Self {
        Config {
            includes: vec![],
            excludes: vec![],
            regex: vec![],
            output: ".".to_string(),
            name: "backup".to_string(),
            threads: 4,
            verbose: false,
            force: false,
            incremental: false,
        }
    }

    pub fn from_args(args: &ArgMatches) -> Self {
        Config {
            includes: args
                .values_of("include")
                .unwrap_or(Values::default())
                .map(|x| x.to_string())
                .collect(),
            excludes: args
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
            threads: args
                .value_of("threads")
                .unwrap_or("4")
                .parse::<u32>()
                .unwrap_or(4),
            verbose: args.is_present("verbose"),
            force: args.is_present("force"),
            incremental: args.is_present("incremental"),
        }
    }

    pub fn from_yaml(path: &str) -> Self {
        let reader = File::open(path).expect("Could not open the config file");
        serde_yaml::from_reader(reader).expect("Could not read config file")
    }

    pub fn write_yaml(&self, path: &str) {
        let writer = File::create(path).expect("Could not create the config file");
        serde_yaml::to_writer(writer, &self).expect("Could not serialise config");
    }

    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(&self).expect("Could not serialise config")
    }
}
