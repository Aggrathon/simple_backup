use core::panic;
use std::path::PathBuf;

use indicatif::{ProgressBar, ProgressStyle};
use number_prefix::NumberPrefix;
use regex::RegexSet;

use crate::backup::{BackupReader, BackupWriter};
use crate::config::Config;
use crate::files::{FileAccessError, FileInfo};
use crate::utils::sanitise_windows_paths;

/// Backup files
pub fn backup(config: Config, verbose: bool, force: bool, dry: bool) {
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
        bw.get_files(
            false,
            Some(|res: Result<&mut FileInfo, FileAccessError>| match res {
                Ok(fi) => {
                    num_files += 1;
                    total_size += fi.size;
                    match NumberPrefix::binary(fi.size as f64) {
                        NumberPrefix::Standalone(number) => {
                            println!("{:>6.0} B    {}", number, &fi.get_string());
                        }
                        NumberPrefix::Prefixed(prefix, number) => {
                            println!("{:>6.2} {}B  {}", number, prefix, &fi.get_string());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                }
            }),
        )
        .expect("Could not crawl for files");
    } else {
        println!("Crawling for files...");
        bw.get_files(
            false,
            Some(|res: Result<&mut FileInfo, FileAccessError>| match res {
                Ok(fi) => {
                    num_files += 1;
                    total_size += fi.size;
                }
                Err(e) => {
                    eprintln!("{}", e);
                }
            }),
        )
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
        eprintln!("Backing up files...");
        let bar = ProgressBar::new(total_size + num_files);
        bar.set_style(ProgressStyle::default_bar().template(
            "{wide_msg} {bytes:>8} / {total_bytes:<8}\n{wide_bar} {elapsed_precise:>8} / {duration_precise:<8}",
        ));
        bar.set_message("Compressing file list");
        bar.tick();
        bar.enable_steady_tick(1000);
        bw.write(
            |fi| bar.set_message(fi.move_string()),
            |fi: &mut FileInfo, err| match err {
                Ok(_) => bar.inc(fi.size + 1),
                Err(e) => {
                    bar.inc(fi.size + 1);
                    bar.println(format!(
                        "Could not add '{}' to the backup: {}",
                        fi.get_string(),
                        e
                    ));
                }
            },
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
    output: &str,
    include: Vec<&str>,
    regex: Vec<&str>,
    flatten: bool,
    non_incremental: bool,
    force: bool,
    verbose: bool,
    dry: bool,
) {
    let non_incremental = {
        let mut conf = source.get_config().expect("Could not read the backup");
        if conf.incremental {
            if non_incremental {
                // non_incremental => !config.incremental
                conf.incremental = false;
            }
            non_incremental
        } else {
            // !config.incremental => non_incremental
            true
        }
    };
    let list_str: String;
    let list: Vec<String>;
    let include: Vec<&str> = if include.is_empty() {
        if non_incremental && regex.is_empty() {
            vec![]
        } else {
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

    if include.is_empty() && !non_incremental {
        eprintln!("No files to backup");
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
        let bar = ProgressBar::new(include.len() as u64);
        bar.set_style(
            ProgressStyle::default_bar().template(
                "{wide_msg} {pos:>8} / {len:<8}\n{wide_bar} {elapsed_precise:>8} / {duration_precise:<8}",
            ),
        );
        bar.set_message("Restoring files");
        bar.tick();
        bar.enable_steady_tick(1000);

        let output = PathBuf::from(output);
        let callback = |res| match res {
            Ok(_) => bar.inc(1),
            Err(e) => {
                bar.inc(1);
                bar.println(format!("Could not restore from backup: {}", e));
            }
        };

        if flatten {
            let path_transform = |mut fi: FileInfo| {
                bar.set_message(fi.move_string());
                FileInfo::from(output.join(fi.consume_path().file_name().unwrap()))
            };
            if non_incremental && include.is_empty() {
                source.restore_this(path_transform, callback, force)
            } else {
                source.restore_selected(include, path_transform, callback, force)
            }
        } else {
            let path_transform = |mut fi: FileInfo| {
                bar.set_message(fi.move_string());
                if fi.get_path().has_root() {
                    fi
                } else {
                    FileInfo::from(output.join(fi.consume_path()))
                }
            };
            if non_incremental && include.is_empty() {
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
