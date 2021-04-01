#[macro_use]
extern crate clap;

#[macro_use]
mod utils;
mod backup;
mod cli;
mod compression;
mod config;
mod files;
mod gui;
mod parse_date;

use std::path::PathBuf;

// use backup::get_backup_config;
use clap::{App, Arg, SubCommand, Values};
use config::Config;
use utils::{get_backup_from_path, get_config_from_path};

fn arg_include<'a>(restore: bool) -> Arg<'a, 'a> {
    let arg = Arg::with_name("include")
        .short("i")
        .long("include")
        .value_name("PATH")
        .takes_value(true)
        .multiple(true);
    if restore {
        arg.help("Files to restore (if given then only these are restored)")
    } else {
        arg.help("Paths (file or directory) to include in the backup")
            .min_values(1)
            .required(true)
    }
}

fn arg_exclude<'a>() -> Arg<'a, 'a> {
    Arg::with_name("exclude")
        .short("e")
        .long("exclude")
        .value_name("PATH")
        .help("Paths (file or directory) to exclude from the backup")
        .takes_value(true)
        .multiple(true)
}

fn arg_output<'a>(restore: bool) -> Arg<'a, 'a> {
    let arg = Arg::with_name("output")
        .short("o")
        .long("output")
        .value_name("PATH")
        .takes_value(true);
    if restore {
        arg.help("The root directory to restore to")
    } else {
        arg.help(
            "Where should the backup be stored (either a direcory or a file ending in `.tar.br`)",
        )
        .default_value(".")
    }
}

fn arg_force<'a>() -> Arg<'a, 'a> {
    Arg::with_name("force")
        .short("f")
        .long("force")
        .help("Overwrite existing files")
}

fn arg_verbose<'a>() -> Arg<'a, 'a> {
    Arg::with_name("verbose")
        .short("v")
        .long("verbose")
        .help("Increase verbosity")
}

fn arg_regex<'a>(restore: bool) -> Arg<'a, 'a> {
    let arg = Arg::with_name("regex")
        .short("r")
        .long("regex")
        .value_name("REGEX")
        .takes_value(true)
        .multiple(true);
    if restore {
        arg.help("Use regex to specify which files to restore")
    } else {
        arg.help("Use regex to specify exclusion filters")
    }
}

fn arg_dry<'a>() -> Arg<'a, 'a> {
    Arg::with_name("dry")
        .short("d")
        .long("dry")
        .help("Only display the output, don't write anything to disk")
}

fn arg_conf_file<'a>(new: bool) -> Arg<'a, 'a> {
    let arg = Arg::with_name("file")
        .value_name("CONFIG")
        .help("The path to the config file, previous backup, or directory with previous backups")
        .takes_value(true)
        .required(true);
    if new {
        arg.validator(|v| {
            if v.ends_with(".yml") {
                Ok(())
            } else {
                Err("The config file must end with .yml".to_string())
            }
        })
    } else {
        arg.validator(|v| {
            let path = PathBuf::from(&v);
            if path.exists() {
                if path.is_dir() {
                    Ok(())
                } else if path.is_file() {
                    if v.ends_with(".yml") || v.ends_with(".tar.br"){
                        Ok(())
                    } else {
                        Err("The file must be either a config file (ends with '.yml') or a previous backup (ends with `.tar.br`)".to_string())
                    }
                } else {
                    Err("The path to the config file is broken".to_string())
                }
            } else {
                Err("File does not exist".to_string())
            }
        })
    }
}

fn arg_source<'a>() -> Arg<'a, 'a> {
    Arg::with_name("source")
        .value_name("PATH")
        .help("Path to the backup, backup directory, or config file")
        .takes_value(true)
        .required(true)
}

fn arg_flatten<'a>() -> Arg<'a, 'a> {
    Arg::with_name("flatten")
        .short("F")
        .long("flatten")
        .help("Remove the paths and restore all files to the same directory")
        .requires("output")
}

fn arg_incremental<'a>() -> Arg<'a, 'a> {
    Arg::with_name("incremental")
        .short("I")
        .long("incremental")
        .help("Do an incremental backup (only backup files that have been modified)")
}

fn arg_time<'a>(req: bool) -> Arg<'a, 'a> {
    let arg = Arg::with_name("time")
        .long("time")
        .help("If doing an incremental backup, set the previous time to this")
        .validator(|v| parse_date::try_parse(&v).map_err(String::from).map(|_| ()));
    if req {
        arg.requires("incremental")
    } else {
        arg
    }
}

fn arg_quality<'a>() -> Arg<'a, 'a> {
    Arg::with_name("quality")
        .short("q")
        .long("quality")
        .value_name("quality")
        .help("Compression quality (1-11)")
        .takes_value(true)
        .default_value("11")
        .validator(|v: String| match v.parse::<u32>() {
            Ok(v) => {
                if v >= 1 && v <= 11 {
                    Ok(())
                } else {
                    Err(String::from("Must be a number between 1-11"))
                }
            }
            Err(_) => Err(String::from("Must be a number between 1-11")),
        })
}

fn arg_local<'a>() -> Arg<'a, 'a> {
    Arg::with_name("local")
        .short("l")
        .long("local")
        .help("Preserve relative (local) paths instead of converting to absolute paths")
}

fn main() {
    let matches = App::new(crate_name!())
        .setting(clap::AppSettings::SubcommandsNegateReqs)
        // .author(crate_authors!())
        .version(crate_version!())
        .about(crate_description!())
        .arg(arg_include(false))
        .arg(arg_exclude())
        .arg(arg_regex(false))
        .arg(arg_output(false))
        .arg(arg_incremental())
        .arg(arg_time(true))
        .arg(arg_local())
        .arg(arg_force())
        .arg(arg_verbose())
        .arg(arg_quality())
        .arg(arg_dry())
        .subcommand(
            SubCommand::with_name("backup")
                .version(crate_version!())
                .about("Backup using arguments from a config file")
                .arg(arg_conf_file(false))
                .arg(arg_time(false))
                .arg(arg_dry()),
        )
        .subcommand(
            SubCommand::with_name("restore")
                .version(crate_version!())
                .about("Restore from a backup.")
                .arg(arg_source())
                .arg(arg_output(true))
                .arg(arg_include(true))
                .arg(arg_regex(true))
                .arg(arg_flatten())
                .arg(arg_force())
                .arg(arg_verbose())
                .arg(arg_dry()),
        )
        .subcommand(SubCommand::with_name("gui").about("Start a graphical user interface"))
        .subcommand(
            SubCommand::with_name("config")
                .version(crate_version!())
                .about("Create a config file. The flags and options are added to the config file")
                .arg(arg_conf_file(true))
                .arg(arg_include(false))
                .arg(arg_exclude())
                .arg(arg_regex(false))
                .arg(arg_output(false))
                .arg(arg_incremental())
                .arg(arg_local())
                .arg(arg_force())
                .arg(arg_verbose())
                .arg(arg_quality())
                .arg(arg_dry()),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("restore") {
        // Restore backed up files
        cli::restore(
            get_backup_from_path(matches.value_of("source").unwrap())
                .expect("Could not find backup"),
            matches.value_of("output").unwrap_or("."),
            matches
                .values_of("include")
                .unwrap_or(Values::default())
                .collect(),
            matches
                .values_of("regex")
                .unwrap_or(Values::default())
                .collect(),
            matches.is_present("flatten"),
            matches.is_present("force"),
            matches.is_present("verbose"),
            matches.is_present("dry"),
        );
    } else if let Some(_) = matches.subcommand_matches("gui") {
        // Start a graphical user interface
        gui::gui();
    } else if let Some(matches) = matches.subcommand_matches("backup") {
        // Backup using an existing config
        let path = matches.value_of("file").unwrap();
        let config = get_config_from_path(path).expect("Could not load config");
        cli::backup(
            config,
            matches.is_present("force"),
            matches.is_present("dry"),
        );
    } else if let Some(matches) = matches.subcommand_matches("config") {
        // Create a config file
        let mut config = Config::from_args(&matches);
        if matches.is_present("dry") {
            println!("{}", config.to_yaml().expect("Could not serialise config"));
        } else {
            config
                .write_yaml(matches.value_of("file").unwrap())
                .expect("Could not serialise config");
        }
    } else {
        // Backup using arguments
        let config = Config::from_args(&matches);
        cli::backup(
            config,
            matches.is_present("force"),
            matches.is_present("dry"),
        );
    }
}
