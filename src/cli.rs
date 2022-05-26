/// This module contains the logic for running the program from a command line
use core::panic;
use std::path::PathBuf;

use indicatif::{ProgressBar, ProgressStyle};
use number_prefix::NumberPrefix;
use regex::RegexSet;

use crate::backup::{BackupReader, BackupWriter};
use crate::config::Config;
use crate::files::{FileAccessError, FileInfo};
use crate::utils::{sanitise_windows_paths, strip_absolute_from_path};

/// Backup files
pub fn backup(config: Config, verbose: bool, force: bool, dry: bool, quiet: bool) {
    let (mut bw, error) = BackupWriter::new(config);
    if error.is_some() {
        eprintln!(
            "Could not get time from previous backup: {}",
            error.unwrap()
        );
    }
    if bw.path.exists() && !force {
        panic!(
            "Backup already exists at '{}' (use --force to overwrite)",
            bw.path.to_string_lossy()
        );
    }

    // Crawl for files
    let mut num_files = 0;
    let mut total_size = 0;
    if verbose {
        if bw.config.time.is_some() {
            eprintln!("Updated files to backup:");
        } else {
            eprintln!("Files to backup:");
        }
        bw.foreach_file(false, |res: Result<&mut FileInfo, FileAccessError>| {
            match res {
                Ok(fi) => {
                    num_files += 1;
                    total_size += fi.size;
                    match NumberPrefix::binary(fi.size as f64) {
                        NumberPrefix::Standalone(number) => {
                            println!("{:>6.2} KiB  {}", number / 1024.0, &fi.get_string());
                        }
                        NumberPrefix::Prefixed(prefix, number) => {
                            println!("{:>6.2} {}B  {}", number, prefix, &fi.get_string());
                        }
                    }
                }
                Err(e) => eprintln!("{}", e),
            }
            Ok(())
        })
        .expect("Could not crawl for files");
    } else {
        if !quiet {
            println!("Crawling for files...");
        }
        bw.foreach_file(false, |res: Result<&mut FileInfo, FileAccessError>| {
            match res {
                Ok(fi) => {
                    num_files += 1;
                    total_size += fi.size;
                }
                Err(e) => eprintln!("{}", e),
            }
            Ok(())
        })
        .expect("Could not crawl for files");
    }

    if num_files == 0 {
        eprintln!("Nothing to backup!");
        return;
    }

    // Perform the backup
    if !dry {
        if verbose {
            eprintln!("");
        }
        if !quiet {
            eprintln!("Backing up files...");
        }
        let bar = if quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(total_size + num_files)
        };
        bar.set_style(ProgressStyle::default_bar().template(
            "{wide_msg} {bytes:>8} / {total_bytes:<8}\n{wide_bar} {elapsed_precise:>8} / {duration_precise:<8}",
        ));
        bar.set_message("Compressing file list");
        bar.tick();
        bar.enable_steady_tick(1000);
        bw.write(
            |fi: &mut FileInfo, err| {
                bar.set_message(fi.move_string());
                bar.inc(fi.size + 1);
                if let Err(e) = err {
                    bar.println(format!(
                        "Could not add '{}' to the backup: {}",
                        fi.get_string(),
                        e
                    ));
                }
                Ok(())
            },
            || bar.set_message("Waiting for the compression to complete..."),
        )
        .expect("Could not create backup file");
        bar.disable_steady_tick();
        bar.set_message("Backup completed!");
        bar.finish();
    }
}

/// Restore files from a backup
pub fn restore(
    mut source: BackupReader,
    output: Option<&str>,
    include: Vec<&str>,
    regex: Vec<&str>,
    flatten: bool,
    only_this: bool,
    force: bool,
    verbose: bool,
    dry: bool,
    quiet: bool,
) {
    let only_this = {
        let mut conf = source.get_config().expect("Could not read the backup");
        if conf.incremental {
            if only_this {
                conf.incremental = false;
            }
            only_this
        } else {
            true
        }
    };
    let list_str: String;
    let list: Vec<String>;
    let include: Vec<&str> = if include.is_empty() {
        list_str = source
            .extract_list()
            .expect("Could not get list of files from backup");
        if regex.is_empty() {
            list_str.split('\n').collect()
        } else {
            let regex = RegexSet::new(regex).expect("Could not parse regex");
            list_str
                .split('\n')
                .filter(|f| !regex.is_match(f))
                .collect()
        }
    } else {
        if regex.is_empty() {
            list = include.into_iter().map(sanitise_windows_paths).collect();
        } else {
            let regex = RegexSet::new(regex).expect("Could not parse regex");
            list = include
                .into_iter()
                .filter(|f| !regex.is_match(f))
                .map(sanitise_windows_paths)
                .collect();
        }
        list.iter().map(String::as_str).collect()
    };

    if include.is_empty() && !only_this {
        if !quiet {
            eprintln!("No files to backup");
        }
        return;
    }

    if verbose {
        eprintln!("Files to restore:");
        for f in include.iter() {
            println!("{}", f);
        }
        eprintln!("");
    }

    if !dry {
        let bar = if quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(include.len() as u64)
        };
        bar.set_style(
            ProgressStyle::default_bar().template(
                "{wide_msg} {pos:>8} / {len:<8}\n{wide_bar} {elapsed_precise:>8} / {duration_precise:<8}",
            ),
        );
        bar.set_message("Restoring files");
        bar.tick();
        bar.enable_steady_tick(1000);

        let callback = |res| match res {
            Ok(_) => bar.inc(1),
            Err(e) => {
                bar.inc(1);
                bar.println(format!("Could not restore from backup: {}", e));
            }
        };

        if flatten {
            let output = PathBuf::from(output.expect("Output directory required for flattening"));
            let path_transform = |mut fi: FileInfo| {
                bar.set_message(fi.move_string());
                FileInfo::from(output.join(fi.consume_path().file_name().unwrap()))
            };
            if only_this && include.is_empty() {
                source.restore_this(path_transform, callback, force)
            } else {
                source.restore_selected(include, path_transform, callback, force)
            }
        } else {
            let output = output.map(PathBuf::from);
            let path_transform = |mut fi: FileInfo| {
                if let Some(o) = &output {
                    let s = fi.move_string();
                    let path = strip_absolute_from_path(&s);
                    bar.set_message(s);
                    FileInfo::from(o.join(path))
                } else {
                    bar.set_message(fi.move_string());
                    fi
                }
            };
            if only_this && include.is_empty() {
                source.restore_this(path_transform, callback, force)
            } else {
                source.restore_selected(include, path_transform, callback, force)
            }
        }
        .expect("Could not restore from backup");

        bar.disable_steady_tick();
        bar.set_message("Restoration Completed!");
        bar.finish();
    }
}
