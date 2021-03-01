use std::time::SystemTime;

use crate::{
    backup::get_previous_time,
    compression::CompressionEncoder,
    config::Config,
    files::{FileCrawler, FileInfo},
    parse_date::system_to_naive,
    utils::ProgressBar,
};

/// Backup files
pub fn backup(config: &mut Config, dry: bool) {
    // Check for overwrite and collect timestamp
    let current_time = system_to_naive(SystemTime::now());
    let output = config.get_output();
    if output.exists() && !config.force {
        eprintln!(
            "Backup already exists at '{}' (use --force to overwrite)",
            output.to_string_lossy()
        );
        return;
    }

    // Prepare lists of files
    let mut files_string = String::new();
    files_string.reserve(10_000);
    let mut files_all: Vec<FileInfo> = vec![];
    let mut listed = false;

    // Crawl for incremental files
    if config.incremental {
        if let Some(time_prev) = get_previous_time(&config) {
            if config.verbose {
                println!("Updated files to backup:");
            } else {
                println!("Crawling for updated files...");
            }
            files_all = FileCrawler::new(
                &config.include,
                &config.exclude,
                &config.regex,
                config.local,
            )
            .expect("Could not start crawling for files")
            .into_iter()
            .filter_map(|fi| match fi {
                Ok(fi) => Some(fi),
                Err(e) => {
                    eprintln!("Could not access file: {}", e);
                    None
                }
            })
            .filter_map(|mut fi| {
                fi.time
                    .and_then(|t| {
                        let fresh = t > time_prev;
                        if !dry {
                            fi.to_writer(&mut files_string)
                                .expect("Could not create list of files");
                            // TODO: mark incremental
                        }
                        if config.verbose && fresh {
                            println!("{}", fi.get_string());
                        }
                        Some(Some(fi))
                    })
                    .unwrap_or(None)
            })
            .collect();
            listed = true;
        }
    }
    // Crawl for all files
    if !listed {
        if config.verbose {
            println!("Files to backup:");
        } else {
            println!("Crawling for files...");
        }
        files_all = FileCrawler::new(
            &config.include,
            &config.exclude,
            &config.regex,
            config.local,
        )
        .expect("Could not start crawling for files")
        .filter_map(|fi| match fi {
            Ok(fi) => Some(fi),
            Err(e) => {
                eprintln!("Could not access file: {}", e);
                None
            }
        })
        .map(|mut fi| {
            if !dry {
                fi.to_writer(&mut files_string)
                    .expect("Could not create list of files");
            }
            if config.verbose {
                println!("{}", fi.get_string());
            }
            fi
        })
        .collect();
    }

    if files_all.len() == 0 {
        println!("Nothing to backup!");
        return;
    }

    // Perform the backup
    if !dry {
        if config.verbose {
            println!("");
        }
        config.time = Some(current_time);
        let mut comp = CompressionEncoder::create(&output, config.quality)
            .expect("Could not create backup file");
        comp.append_data("config.yml", &config.to_yaml())
            .expect("Could not write to the backup");
        comp.append_data("files.csv", &files_string)
            .expect("Could not write to the backup");
        let mut bar = ProgressBar::start(files_all.len(), 80, "Backing up files");
        for fi in files_all.iter_mut() {
            comp.append_file(fi.get_path()).unwrap_or_else(|e| {
                eprintln!("Could not add '{}' to the backup: {}", fi.get_string(), e)
            });
            bar.progress();
        }
        comp.close().expect("Could not store the backup");
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
    panic!("Restoring is not implemented");
}
