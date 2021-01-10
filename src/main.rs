#[macro_use]
extern crate clap;

use clap::{App, Arg, SubCommand};

fn main() {
    let matches = App::new(crate_name!())
        .author(crate_authors!())
        .version(crate_version!())
        .about(crate_description!())
        .arg(
            Arg::with_name("include")
                .short("i")
                .long("include")
                .value_name("PATH")
                .help("A path (file or directory) to include in the backup")
                .takes_value(true)
                .multiple(true)
                .required(true),
        )
        .arg(
            Arg::with_name("exclude")
                .short("e")
                .long("exclude")
                .value_name("PATH")
                .help("A path (file or directory) to exclude from the backup")
                .takes_value(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("PATH")
                .help("Where should the backup be stored (either a direcory or a file ending in `.tar.zstd`)")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .value_name("NAME")
                .help("Prefix to the backup filenames")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("force")
                .short("f")
                .long("force")
                .help("Overwrite existing files")
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Increase verbosity"),
        )
        .arg(
            Arg::with_name("regex")
                .short("r")
                .long("filter")
                .value_name("REGEX")
                .help("Use regex to specify exclusion filters")
                .takes_value(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("threads")
                .short("t")
                .long("threads")
                .value_name("NUM")
                .help("How many threads should be used for compression")
                .takes_value(true)
                .default_value("4")
                .validator(|v: String| match v.parse::<u32>() {
                    Ok(_) => Ok(()),
                    Err(_) => Err(String::from("The value must be a number"))
                }),
        )
        .arg(
            Arg::with_name("dry")
                .short("d")
                .long("dry")
                .help("Only display the output, don't create any backup")
        )
        .subcommand(
            Subcommand::with_name("backup")
                .about("Backup using a config file"),
        )
        .subcommand(
            Subcommand::with_name("restore")
                .about("Restore from a backup"),
        )
        .subcommand(
            Subcommand::with_name("browse")
                .about("List file in a backup"),
        )
        .subcommand(
            Subcommand::with_name("gui")
                .about("Start a graphical user interface"),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("restore") {
        panic!("Restoring is not yet implemented");
    } else if let Some(matches) = matches.subcommand_matches("browse") {
        panic!("Restoring is not yet implemented");
    } else if let Some(matches) = matches.subcommand_matches("gui") {
        panic!("GUI is not yet implemented");
    } else if let Some(matches) = matches.subcommand_matches("backup") {
        panic!("Config files are not yet implemented");
    } else {
    }
}
