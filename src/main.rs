#[macro_use]
extern crate clap;

mod backup;
mod compression;
mod config;
mod files;
mod gui;
mod parse_date;
mod restore;
mod utils;

use std::path::PathBuf;

use clap::{App, Arg, SubCommand, Values};
use config::Config;
use restore::get_config_from_backup;
use utils::BackupIterator;

fn arg_include<'a>() -> Arg<'a, 'a> {
    Arg::with_name("include")
        .short("i")
        .long("include")
        .value_name("PATH")
        .help("A path (file or directory) to include in the backup")
        .takes_value(true)
        .multiple(true)
}

fn arg_exclude<'a>() -> Arg<'a, 'a> {
    Arg::with_name("exclude")
        .short("e")
        .long("exclude")
        .value_name("PATH")
        .help("A path (file or directory) to exclude from the backup")
        .takes_value(true)
        .multiple(true)
}

fn arg_output<'a>(backup: bool) -> Arg<'a, 'a> {
    let arg = Arg::with_name("output")
        .short("o")
        .long("output")
        .value_name("PATH")
        .takes_value(true);
    if backup {
        arg.help(
            "Where should the backup be stored (either a direcory or a file ending in `.tar.br`)",
        )
        .default_value(".")
    } else {
        arg.help("The root directory to restore to")
    }
}

fn arg_name<'a>() -> Arg<'a, 'a> {
    Arg::with_name("name")
        .short("n")
        .long("name")
        .value_name("NAME")
        .help("Prefix to the backup filenames")
        .takes_value(true)
        .default_value("backup")
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

fn arg_regex<'a>(variant: u32) -> Arg<'a, 'a> {
    let arg = Arg::with_name("regex")
        .short("r")
        .long("regex")
        .value_name("REGEX")
        .takes_value(true)
        .multiple(true);
    if variant == 1 {
        arg.help("Use regex to specify which files to restore")
    } else if variant == 2 {
        arg.help("Use regex to specify which files to show")
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

fn arg_conffile<'a>() -> Arg<'a, 'a> {
    Arg::with_name("file")
        .value_name("PATH")
        .help("The path to the config file, previous backup, or directory with previous backups")
        .takes_value(true)
        .required(true)
        .validator(|v| {
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
        .help("Do an incremental backup (only backup files that have been changed)")
}

fn arg_time<'a>(req: bool) -> Arg<'a, 'a> {
    let arg = Arg::with_name("time")
        .long("time")
        .help("If doing an incremental backup, override the previous time")
        .validator(|v| parse_date::try_parse(&v).map_err(String::from).map(|_| ()));
    if req {
        arg.requires("incremental")
    } else {
        arg
    }
}

fn arg_all<'a>() -> Arg<'a, 'a> {
    Arg::with_name("all")
        .short("a")
        .long("all")
        .help("Restore all files (even ones deleted in the last backup)")
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
        // .author(crate_authors!())
        .version(crate_version!())
        .about(crate_description!())
        .arg(arg_include())
        .arg(arg_exclude())
        .arg(arg_regex(0))
        .arg(arg_output(true))
        .arg(arg_name())
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
                .arg(arg_conffile())
                .arg(arg_time(false))
                .arg(arg_dry()),
        )
        .subcommand(
            SubCommand::with_name("restore")
                .version(crate_version!())
                .about("Restore from a backup.")
                .arg(arg_source())
                .arg(arg_output(false))
                .arg(arg_regex(1))
                .arg(arg_all())
                .arg(arg_flatten())
                .arg(arg_force())
                .arg(arg_verbose())
                .arg(arg_dry()),
        )
        .subcommand(
            SubCommand::with_name("browse")
                .version(crate_version!())
                .about("List files in a backup.")
                .arg(arg_source())
                .arg(arg_regex(2)),
        )
        .subcommand(SubCommand::with_name("gui").about("Start a graphical user interface"))
        .subcommand(
            SubCommand::with_name("config")
                .version(crate_version!())
                .about("Create a config file. The flags and options are added to the config file")
                .arg(arg_conffile())
                .arg(arg_include())
                .arg(arg_exclude())
                .arg(arg_regex(0))
                .arg(arg_output(true))
                .arg(arg_name())
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
        restore::restore(
            matches.value_of("source").unwrap(),
            matches.value_of("output").unwrap_or(""),
            matches
                .values_of("regex")
                .unwrap_or(Values::default())
                .collect(),
            matches.is_present("all"),
            matches.is_present("force"),
            matches.is_present("verbose"),
            matches.is_present("flatten"),
            matches.is_present("dry"),
        );
    } else if let Some(matches) = matches.subcommand_matches("browse") {
        // List files from backups
        // TODO: Possibly load config from an earlier backup
        restore::browse(
            matches.value_of("source").unwrap(),
            matches
                .values_of("regex")
                .unwrap_or(Values::default())
                .collect(),
        );
    } else if let Some(_) = matches.subcommand_matches("gui") {
        // Start a graphical user interface
        gui::gui();
    } else if let Some(matches) = matches.subcommand_matches("backup") {
        // Backup using an existing config
        let path = matches.value_of("file").unwrap();
        let mut config = if path.ends_with(".yml") {
            Config::read_yaml(&path).expect("Could not get config from file")
        } else if path.ends_with(".tar.br") {
            get_config_from_backup(&path).expect("Could not get config from previous backup")
        } else {
            let mut config: Option<Config> = None;
            let mut selected = PathBuf::new();
            for path in BackupIterator::with_ending(path) {
                if let Err(e) = &path {
                    eprintln!("Could not find backups: {}", e);
                }
                let path = path.unwrap();
                let new = get_config_from_backup(&path);
                if let Err(e) = &new {
                    eprintln!("Could not get config from backup: {}", e);
                }
                let new = new.unwrap();
                if let Some(old) = config {
                    if old.time < new.time {
                        config = Some(new);
                        selected = path;
                    } else {
                        config = Some(old);
                    }
                } else {
                    selected = path;
                    config = Some(new);
                }
            }
            if config.is_none() {
                panic!("Could not find a config from an earlier backup");
            }
            println!("Using the config from '{}'", selected.to_string_lossy());
            config.unwrap()
        };
        backup::backup(&mut config, matches.is_present("dry"));
    } else if let Some(matches) = matches.subcommand_matches("config") {
        // Create a config file
        let mut config = Config::from_args(&matches);
        if matches.is_present("dry") {
            println!("{}", config.to_yaml());
        } else {
            config.write_yaml(matches.value_of("file").unwrap());
        }
    } else {
        // Backup using arguments
        let mut config = Config::from_args(&matches);
        backup::backup(&mut config, matches.is_present("dry"));
    }
}
