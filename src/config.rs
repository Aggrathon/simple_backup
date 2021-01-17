use crate::utils::parse_date;
use chrono::NaiveDateTime;
use clap::{ArgMatches, Values};
use serde::{Deserialize, Serialize};
use std::fs::File;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub regex: Vec<String>,
    pub output: String,
    pub name: String,
    pub threads: u32,
    pub verbose: bool,
    pub force: bool,
    pub incremental: bool,
    pub level: i32,
    pub local: bool,
    #[serde(with = "parse_date")]
    pub time: NaiveDateTime,
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
            threads: 4,
            verbose: false,
            force: false,
            incremental: false,
            level: 1,
            local: false,
            time: NaiveDateTime::from_timestamp(0, 0),
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
            threads: args
                .value_of("threads")
                .and_then(|v| Some(v.parse::<u32>().expect("Could not parse number")))
                .unwrap_or(4),
            verbose: args.is_present("verbose"),
            force: args.is_present("force"),
            incremental: args.is_present("incremental"),
            level: args
                .value_of("level")
                .and_then(|v| Some(v.parse::<i32>().expect("Could not parse number")))
                .unwrap_or(1),
            local: args.is_present("local"),
            time: args
                .value_of("time")
                .and_then(|v| Some(parse_date::try_parse(v).expect("Could not parse time")))
                .unwrap_or(NaiveDateTime::from_timestamp(0, 0)),
        }
    }

    pub fn from_yaml(path: &str) -> Self {
        let reader = File::open(path).expect("Could not open the config file");
        serde_yaml::from_reader(reader).expect("Could not read config file")
    }

    pub fn write_yaml(&mut self, path: &str) {
        self.sort();
        let writer = File::create(path).expect("Could not create the config file");
        serde_yaml::to_writer(writer, &self).expect("Could not serialise config");
    }

    pub fn to_yaml(&mut self) -> String {
        self.sort();
        serde_yaml::to_string(&self).expect("Could not serialise config")
    }

    pub fn sort(&mut self) {
        self.include.sort();
        self.exclude.sort();
    }
}
