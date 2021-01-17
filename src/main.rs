#[macro_use]
extern crate clap;

mod backup;
mod config;

use backup::backup;
use clap::{App, Arg, SubCommand};
use config::Config;

fn arg_include<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .short("i")
        .long("include")
        .value_name("PATH")
        .help("A path (file or directory) to include in the backup")
        .takes_value(true)
        .multiple(true)
}

fn arg_exclude<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .short("e")
        .long("exclude")
        .value_name("PATH")
        .help("A path (file or directory) to exclude from the backup")
        .takes_value(true)
        .multiple(true)
}

fn arg_output<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .short("o")
        .long("output")
        .value_name("PATH")
        .help(
            "Where should the backup be stored (either a direcory or a file ending in `.tar.zstd`)",
        )
        .takes_value(true)
        .default_value(".")
}

fn arg_name<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .short("n")
        .long("name")
        .value_name("NAME")
        .help("Prefix to the backup filenames")
        .takes_value(true)
        .default_value("backup")
}

fn arg_force<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .short("f")
        .long("force")
        .help("Overwrite existing files")
}

fn arg_verbose<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .short("v")
        .long("verbose")
        .help("Increase verbosity")
}

fn arg_regex<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .short("r")
        .long("regex")
        .value_name("REGEX")
        .help("Use regex to specify exclusion filters")
        .takes_value(true)
        .multiple(true)
}

fn arg_threads<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .short("t")
        .long("threads")
        .value_name("NUM")
        .help("How many threads should be used for compression")
        .takes_value(true)
        .default_value("4")
        .validator(|v: String| match v.parse::<u32>() {
            Ok(_) => Ok(()),
            Err(_) => Err(String::from("The value must be a number")),
        })
}

fn arg_dry<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .short("d")
        .long("dry")
        .help("Only display the output, don't write anything to disk.")
}

fn arg_conffile<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
        .value_name("CONFIG_FILE")
        .help("The path to the config file")
        .takes_value(true)
        .required(true)
        .validator(|v| {
            if v.ends_with(".yml") {
                Ok(())
            } else {
                Err("The filename for the config file must end in `.yml`".to_string())
            }
        })
}

fn arg_<'a>(name: &'a str) -> Arg<'a, 'a> {
    Arg::with_name(name)
}

fn main() {
    let matches = App::new(crate_name!())
        // .author(crate_authors!())
        .version(crate_version!())
        .about(crate_description!())
        .arg(arg_include("include"))
        .arg(arg_exclude("exclude"))
        .arg(arg_output("output"))
        .arg(arg_name("name"))
        .arg(arg_force("force"))
        .arg(arg_verbose("verbose"))
        .arg(arg_regex("regex"))
        .arg(arg_threads("threads"))
        .arg(arg_dry("dry"))
        .subcommand(
            SubCommand::with_name("backup")
                .about("Backup using arguments from a config file")
                .arg(arg_conffile("file"))
                .arg(arg_dry("dry")),
        )
        .subcommand(SubCommand::with_name("restore").about("Restore from a backup"))
        .subcommand(SubCommand::with_name("browse").about("List file in a backup"))
        .subcommand(SubCommand::with_name("gui").about("Start a graphical user interface"))
        .subcommand(
            SubCommand::with_name("config")
                .about("Create a config file. The flags and options are added to the config file")
                .arg(arg_conffile("file"))
                .arg(arg_include("include"))
                .arg(arg_exclude("exclude"))
                .arg(arg_output("output"))
                .arg(arg_name("name"))
                .arg(arg_force("force"))
                .arg(arg_verbose("verbose"))
                .arg(arg_regex("regex"))
                .arg(arg_threads("threads"))
                .arg(arg_dry("dry")),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("restore") {
        panic!("Restoring is not yet implemented");
    } else if let Some(matches) = matches.subcommand_matches("browse") {
        panic!("Browsing is not yet implemented");
    } else if let Some(matches) = matches.subcommand_matches("gui") {
        panic!("GUI is not yet implemented");
    } else if let Some(matches) = matches.subcommand_matches("backup") {
        let mut config = Config::from_yaml(matches.value_of("file").unwrap());
        backup(&config, matches.is_present("dry"));
    } else if let Some(matches) = matches.subcommand_matches("config") {
        let config = Config::from_args(&matches);
        if matches.is_present("dry") {
            println!("{}", config.to_yaml());
        } else {
            config.write_yaml(matches.value_of("file").unwrap());
        }
    } else {
        let config = Config::from_args(&matches);
        backup(&config, matches.is_present("dry"));
    }
}
