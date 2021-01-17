use std::convert::TryFrom;
use std::{convert, fs};

use clap::{ArgMatches, Values};
use fs::read_to_string;
use yaml_rust::{Yaml, YamlLoader};

pub struct Config {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub regex: Vec<String>,
    pub output: String,
    pub name: String,
    pub threads: u32,
    pub verbose: bool,
    pub force: bool,
    pub dry: bool,
}

impl Config {
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
            dry: false,
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
            dry: args.is_present("dry"),
        }
    }

    pub fn from_yaml(path: &str) -> Self {
        let yaml = fs::read_to_string(path).expect("Could not read the config file");
        let docs = YamlLoader::load_from_str(&yaml).expect("Could not parse the config file");
        Config {
            includes: yaml_values(&docs, "include"),
            excludes: yaml_values(&docs, "exclude"),
            regex: yaml_values(&docs, "regex"),
            output: yaml_value_str(&docs, "output", ".").to_string(),
            name: yaml_value_str(&docs, "name", "backup").to_string(),
            threads: yaml_value_u32(&docs, "threads", 4),
            verbose: yaml_value_bool(&docs, "verbose", false),
            force: yaml_value_bool(&docs, "force", false),
            dry: yaml_value_bool(&docs, "dry", false),
        }
    }
}

fn yaml_values(docs: &Vec<Yaml>, key: &str) -> Vec<String> {
    let tmp = &docs[0][key];
    if !tmp.is_badvalue() {
        if tmp.is_array() {
            return tmp
                .as_vec()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect();
        } else if !tmp.is_null() {
            return vec![tmp.as_str().unwrap().to_string()];
        }
    }
    vec![]
}

fn yaml_value_bool(docs: &Vec<Yaml>, key: &str, default: bool) -> bool {
    let tmp = &docs[0][key];
    if !tmp.is_badvalue() {
        tmp.as_bool()
            .expect(&format!("\"{}\": {:#?} is not a boolean!", key, tmp))
    } else {
        default
    }
}

fn yaml_value_u32(docs: &Vec<Yaml>, key: &str, default: u32) -> u32 {
    let tmp = &docs[0][key];
    if !tmp.is_badvalue() {
        let out = tmp.as_i64().expect(&format!(
            "\"{}\": {:#?} is not a positive integer!",
            key, tmp
        ));
        u32::try_from(out).expect(&format!(
            "\"{}\": {:#?} is not a positive integer!",
            key, tmp
        ))
    } else {
        default
    }
}

fn yaml_value_str<'a>(docs: &'a Vec<Yaml>, key: &'a str, default: &'a str) -> &'a str {
    let tmp = &docs[0][key];
    if !tmp.is_badvalue() {
        tmp.as_str()
            .expect(&format!("\"{}\": {:#?} is not a string!", key, tmp))
    } else {
        default
    }
}
