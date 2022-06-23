/// This is the main module that handles parsing the commandline arguments

#[macro_use]
mod utils;
mod backup;
mod cli;
mod compression;
mod config;
mod files;
#[cfg(feature = "gui")]
mod gui;
mod lists;
mod parse_date;

use std::path::PathBuf;

use backup::CONFIG_FILE_EXTENSION;
use chrono::NaiveDateTime;
use clap::{Args, Parser, Subcommand};
use config::Config;
use utils::{get_backup_from_path, get_config_from_path};

#[derive(Parser)]
#[clap(version, about, long_about = None, propagate_version = true, term_width = 0)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a config file (the options are added to the config file)
    Config {
        /// The path for the new config file
        #[clap(value_parser = parse_config_path, value_name = "CONFIG")]
        path: PathBuf,
        #[clap(flatten)]
        config: ArgConfig,
        /// Only display the output, don't write anything to disk
        #[clap(short, long)]
        dry: bool,
    },
    /// Backup using an existing config file
    Backup {
        /// The path to the config file, previous backup, or directory with previous backups
        #[clap(value_parser = parse_config, value_name = "PATH")]
        config: Config,
        /// If doing an incremental backup, set the previous time to this
        #[clap(short, long, value_parser = parse_time, value_name = "TIME")]
        time: Option<NaiveDateTime>,
        /// Increase verbosity
        #[clap(short, long)]
        verbose: bool,
        /// Overwrite existing files
        #[clap(short, long)]
        force: bool,
        /// Only display the output, don't write anything to disk
        #[clap(short, long)]
        dry: bool,
    },
    /// Restore from a backup
    Restore {
        /// Path to the backup, backup directory, or config file
        #[clap(value_parser, value_name = "PATH")]
        source: PathBuf,
        /// The directory to restore to (if not original)
        #[clap(short, long, value_parser, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Files to restore (if given then only these are restored)
        #[clap(short, long, value_parser, value_name = "PATH")]
        include: Vec<String>,
        /// Use regex to specify which files to restore
        #[clap(short, long, value_parser, value_name = "REGEX")]
        regex: Vec<String>,
        /// Remove the paths and restore all files to the same directory (if an output path is given)
        #[clap(short = 'F', long, value_parser, requires = "output")]
        flatten: bool,
        /// Only restore from the selected / latest backup even if it is incremental
        #[clap(short, long)]
        this: bool,
        /// Increase verbosity
        #[clap(short, long)]
        verbose: bool,
        /// Overwrite existing files
        #[clap(short, long)]
        force: bool,
        /// Only display the output, don't write anything to disk
        #[clap(short, long)]
        dry: bool,
    },
    /// Backup using command line arguments directly
    Direct {
        #[clap(flatten)]
        config: ArgConfig,
        /// If doing an incremental backup, use this as the previous time
        #[clap(short, long, value_parser = parse_time, value_name = "TIME", requires = "incremental")]
        time: Option<NaiveDateTime>,
        /// Increase verbosity
        #[clap(short, long)]
        verbose: bool,
        /// Overwrite existing files
        #[clap(short, long)]
        force: bool,
        /// Only display the output, don't write anything to disk
        #[clap(short, long)]
        dry: bool,
    },
    /// Merge two backup archives
    Merge {
        /// Backups to merge (as paths to the backups or a directory containing backups)
        #[clap(value_parser, value_name = "BACKUPS", required = true)]
        backups: Vec<PathBuf>,
        /// The path to write the merged backup to (otherwise replace the newer backup)
        #[clap(short, long, value_parser, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Keep all files (not just those mentioned in the newest backup)
        #[clap(short, long)]
        all: bool,
        /// Delete the old backups after the merge (instead of renaming them)
        #[clap(short = 'r', long)]
        delete: bool,
        /// Increase verbosity
        #[clap(short, long)]
        verbose: bool,
        /// Overwrite existing files
        #[clap(short, long)]
        force: bool,
        /// Only display the output, don't write anything to disk
        #[clap(short, long)]
        dry: bool,
    },
    #[cfg(feature = "gui")]
    /// Start a graphical user interface
    Gui,
}

#[derive(Args)]
struct ArgConfig {
    /// Paths (file or directory) to include in the backup
    #[clap(short, long, value_parser, value_name = "PATH", required = true)]
    include: Vec<String>,
    /// Paths (file or directory) to exclude from the backup
    #[clap(short, long, value_parser, value_name = "PATH")]
    exclude: Vec<String>,
    /// Use regex to specify exclusion filters
    #[clap(short, long, value_parser, value_name = "REGEX")]
    regex: Vec<String>,
    /// Where should the backup be stored (either a direcory or a file ending in `.tar.zst`)
    #[clap(short, long, value_parser, value_name = "PATH", default_value = ".")]
    output: PathBuf,
    /// Do an incremental backup (only backup files that have been modified)
    #[clap(short = 'I', long)]
    incremental: bool,
    /// Preserve relative (local) paths instead of converting to absolute paths
    #[clap(short, long)]
    local: bool,
    /// Number of worker threads (using threads requires more memory)
    #[clap(short='n', long, value_parser = parse_cpu, default_value_t = 1, value_name = "NUM")]
    threads: u32,
    /// Compression quality (1-22)
    #[clap(short, long, value_parser = parse_quality, default_value_t = 20, value_name = "NUM")]
    quality: i32,
}

impl ArgConfig {
    fn into_config(self, time: Option<NaiveDateTime>) -> Config {
        Config {
            include: self.include,
            exclude: self.exclude,
            regex: self.regex,
            output: self.output,
            incremental: self.incremental,
            quality: self.quality,
            local: self.local,
            threads: self.threads,
            time,
            origin: PathBuf::new(),
        }
    }
}

fn parse_cpu(s: &str) -> Result<u32, String> {
    let cpus = num_cpus::get() as u32;
    if let Ok(i) = s.parse::<u32>() {
        if (1..=cpus).contains(&i) {
            return Ok(i);
        }
    }
    Err(format!("Must be a number between 1-{}!", cpus))
}

fn parse_quality(s: &str) -> Result<i32, &'static str> {
    if let Ok(i) = s.parse::<i32>() {
        if (1..=22).contains(&i) {
            return Ok(i);
        }
    }
    Err("Must be a number between 1-22!")
}

fn parse_time(s: &str) -> Result<NaiveDateTime, &'static str> {
    parse_date::try_parse(s)?.ok_or("Missing time")
}

fn parse_config(s: &str) -> Result<Config, String> {
    get_config_from_path(PathBuf::from(s)).map_err(|e| e.to_string())
}

fn parse_config_path(s: &str) -> Result<PathBuf, String> {
    if s.ends_with(CONFIG_FILE_EXTENSION) {
        Ok(PathBuf::from(s))
    } else {
        Err("The config file must end with".to_string() + CONFIG_FILE_EXTENSION)
    }
}

fn main() {
    let cli = Cli::parse();

    if cli.command.is_none() {
        #[cfg(feature = "gui")]
        gui::gui();
        #[cfg(not(feature = "gui"))]
        Cli::command().print_help().unwrap();
        return;
    }

    match cli.command.unwrap() {
        Commands::Backup {
            mut config,
            time,
            verbose,
            force,
            dry,
        } => {
            if time.is_some() {
                config.time = time;
            }
            cli::backup(config, verbose, force, dry, false);
        }
        #[cfg(feature = "gui")]
        Commands::Gui => {
            gui::gui();
        }
        Commands::Restore {
            source,
            output,
            include,
            regex,
            flatten,
            this,
            verbose,
            force,
            dry,
        } => {
            cli::restore(
                get_backup_from_path(source).expect("Could not find backup"),
                output,
                include,
                regex,
                flatten,
                this,
                force,
                verbose,
                dry,
                false,
            );
        }
        Commands::Config { path, config, dry } => {
            let mut config = config.into_config(None);
            if dry {
                println!("{}", config.as_yaml().expect("Could not serialise config"));
            } else {
                config.write_yaml(path).expect("Could not serialise config");
            }
        }
        Commands::Direct {
            config,
            time,
            verbose,
            force,
            dry,
        } => {
            let config = config.into_config(time);
            cli::backup(config, verbose, force, dry, false);
        }
        Commands::Merge {
            output,
            verbose,
            force,
            dry,
            backups,
            all,
            delete,
        } => cli::merge(backups, output, all, delete, verbose, force, dry, false),
    }
}
