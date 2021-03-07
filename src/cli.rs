use core::panic;
use std::{error::Error, path::PathBuf};

use indicatif::ProgressBar;
use regex::Regex;

use crate::{
    backup::{BackupReader, BackupWriter},
    config::Config,
    files::FileInfo,
};

/// Backup files
pub fn backup(config: Config, dry: bool) {
    let (mut bw, error) = BackupWriter::new(config);
    if error.is_some() {
        eprintln!(
            "Could not get time from previous backup: {}",
            error.unwrap()
        );
    }
    if bw.path.exists() && !bw.config.force {
        panic!(
            "Backup already exists at '{}' (use --force to overwrite)",
            bw.path.to_string_lossy()
        );
    }

    // Crawl for files
    let mut num_files = 0;
    if bw.config.verbose {
        if bw.config.time.is_some() {
            println!("Updated files to backup:");
        } else {
            println!("Files to backup:");
        }
        bw.get_files(
            false,
            Some(|res: Result<&mut FileInfo, Box<dyn Error>>| match res {
                Ok(fi) => {
                    num_files += 1;
                    println!("{}", &fi.get_string());
                }
                Err(e) => {
                    eprintln!("Could not access file: {}", e);
                }
            }),
        )
        .expect("Could not crawl for files");
    } else {
        println!("Crawling for files...");
        bw.get_files(
            false,
            Some(|res: Result<&mut FileInfo, Box<dyn Error>>| match res {
                Ok(_) => {
                    num_files += 1;
                }
                Err(e) => {
                    eprintln!("Could not access file: {}", e);
                }
            }),
        )
        .expect("Could not crawl for files");
    }

    if num_files == 0 {
        println!("Nothing to backup!");
        return;
    }

    // Perform the backup
    if !dry {
        if bw.config.verbose {
            println!("");
        }
        let bar = ProgressBar::new(num_files);
        bar.set_message("Backing up files");
        bar.tick();
        bw.write(|fi: &mut FileInfo, err| match err {
            Ok(_) => bar.inc(1),
            Err(e) => {
                bar.inc(1);
                bar.println(format!(
                    "Could not add '{}' to the backup: {}",
                    fi.get_string(),
                    e
                ));
            }
        })
        .expect("Could not create backup file");
        bar.finish();
    }
}

#[allow(unused_variables)]
pub fn restore<'a>(
    mut source: BackupReader,
    output: &str,
    include: Vec<&str>,
    regex: Vec<&str>,
    flatten: bool,
    force: bool,
    verbose: bool,
    dry: bool,
) {
    if include.is_empty() {
        let list = source
            .extract_list()
            .expect("Could not get list of files from backup");
        let include: Vec<&str> = if regex.is_empty() {
            list.split('\n').collect()
        } else {
            let regex = regex
                .into_iter()
                .map(Regex::new)
                .collect::<Result<Vec<Regex>, regex::Error>>()
                .expect("Could not parse regex");
            list.split('\n')
                .filter(|f| !regex.iter().any(|r| r.is_match(f)))
                .collect()
        };
        if include.is_empty() {
            eprintln!("No files in the backup: {}", source.path.to_string_lossy());
            return;
        }
        return restore(
            source,
            output,
            include,
            vec![],
            flatten,
            force,
            verbose,
            dry,
        );
    }
    let include = if regex.is_empty() {
        include
    } else {
        let regex = regex
            .into_iter()
            .map(Regex::new)
            .collect::<Result<Vec<Regex>, regex::Error>>()
            .expect("Could not parse regex");
        include
            .into_iter()
            .filter(|f| !regex.iter().any(|r| r.is_match(f)))
            .collect()
    };

    if verbose {
        println!("Files to restore:");
        for f in include.iter() {
            println!("{}", f);
        }
        println!("");
    }

    if !dry {
        let bar = ProgressBar::new(include.len() as u64);
        bar.set_message("Restoring files");
        bar.tick();

        let output = PathBuf::from(output);
        let callback = |res| match res {
            Ok(_) => bar.inc(1),
            Err(e) => {
                bar.inc(1);
                bar.println(format!("Could not restore from backup: {}", e));
            }
        };

        if flatten {
            source.restore_selected(
                include,
                |fi| FileInfo::from(output.join(fi.consume_path().file_name().unwrap())),
                callback,
                force,
            )
        } else {
            source.restore_selected(
                include,
                |mut fi| {
                    if fi.get_path().has_root() {
                        fi
                    } else {
                        FileInfo::from(output.join(fi.consume_path()))
                    }
                },
                callback,
                force,
            )
        }
        .expect("Could not restore from backup");

        bar.finish();
    }
}
