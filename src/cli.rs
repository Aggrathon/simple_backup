use core::panic;
use std::error::Error;

use indicatif::ProgressBar;

use crate::{backup::BackupWriter, config::Config, files::FileInfo, utils::get_backup_from_path};

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
        bw.write(Some(|fi: &mut FileInfo, err| match err {
            Ok(_) => bar.inc(1),
            Err(e) => bar.println(format!(
                "Could not add '{}' to the backup: {}",
                fi.get_string(),
                e
            )),
        }))
        .expect("Could not create backup file");
        bar.finish();
    }
}

#[allow(unused_variables)]
pub fn restore(
    source: &str,
    output: &str,
    regex: Vec<&str>,
    all: bool,
    force: bool,
    verbose: bool,
    flatten: bool,
    dry: bool,
) {
    let mut br = get_backup_from_path(source).expect("Could not find backup");
    // TODO handle output

    panic!("Restoring is not implemented");
}
