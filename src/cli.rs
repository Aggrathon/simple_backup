/// This module contains the logic for running the program from a command line
use core::panic;
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};
use number_prefix::NumberPrefix;
use regex::RegexSet;

use crate::backup::{BackupMerger, BackupReader, BackupWriter};
use crate::config::Config;
use crate::files::{FileAccessError, FileInfo};
use crate::lists::FileListString;
use crate::utils::{strip_absolute_from_path, BackupIterator};

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
        .expect("Could not crawl for files:");
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
        .expect("Could not crawl for files:");
    }

    if num_files == 0 {
        eprintln!("Nothing to backup!");
        return;
    }

    // Perform the backup
    if !dry {
        if verbose {
            eprintln!();
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
        .expect("Could not create backup file:");
        bar.disable_steady_tick();
        bar.set_message("Backup completed!");
        bar.finish();
    }
}

/// Restore files from a backup
#[allow(clippy::too_many_arguments)]
pub fn restore<P: AsRef<Path>>(
    mut source: BackupReader,
    output: Option<P>,
    mut include: Vec<String>,
    regex: Vec<String>,
    flatten: bool,
    only_this: bool,
    force: bool,
    verbose: bool,
    dry: bool,
    quiet: bool,
) {
    source.get_meta().expect("Could not read the backup:");
    let only_this = {
        let mut conf = source.get_config().expect("Could not read the backup:");
        if conf.incremental {
            if only_this {
                conf.incremental = false;
            }
            only_this
        } else {
            true
        }
    };
    let tmp1: FileListString;
    let inc_iter = if include.is_empty() {
        tmp1 = source
            .move_list()
            .expect("Could not get list of files from backup:");
        if only_this {
            tmp1.iter_included()
        } else {
            Box::new(tmp1.iter().map(|v| v.1))
        }
    } else {
        #[cfg(target_os = "windows")]
        include.iter_mut().for_each(|s| *s = s.replace('\\', "/"));
        Box::new(include.iter().map(|s| s.as_str()))
    };
    let include: Vec<&str> = if regex.is_empty() {
        inc_iter.collect()
    } else {
        let regex = RegexSet::new(regex).expect("Could not parse regex:");
        inc_iter.filter(|f| regex.is_match(f)).collect()
    };

    if include.is_empty() {
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
        eprintln!();
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

        let callback = |res| {
            match res {
                Ok(_) => bar.inc(1),
                Err(e) => {
                    bar.inc(1);
                    bar.println(format!("Could not restore from backup: {}", e));
                }
            }
            Ok(())
        };

        if flatten {
            let output = output.expect("Output directory required for flattening!");
            let output = output.as_ref();
            let path_transform = |mut fi: FileInfo| {
                bar.set_message(fi.move_string());
                FileInfo::from(output.join(fi.consume_path().file_name().unwrap()))
            };
            source.restore(include, path_transform, callback, force, !only_this)
        } else {
            let path_transform = |mut fi: FileInfo| {
                if let Some(o) = &output {
                    let s = fi.move_string();
                    let path = strip_absolute_from_path(&s);
                    bar.set_message(s);
                    FileInfo::from(o.as_ref().join(path))
                } else {
                    bar.set_message(fi.move_string());
                    fi
                }
            };
            source.restore(include, path_transform, callback, force, !only_this)
        }
        .expect("Could not restore from backup:");

        bar.disable_steady_tick();
        bar.set_message("Restoration Completed!");
        bar.finish();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn merge(
    backups: Vec<PathBuf>,
    path: Option<PathBuf>,
    all: bool,
    delete: bool,
    verbose: bool,
    force: bool,
    dry: bool,
    quiet: bool,
) {
    let backups = backups
        .into_iter()
        .flat_map(|p| BackupIterator::path(p).expect("Could not find backup:"))
        .map(|r| r.map(BackupReader::new))
        .collect::<std::io::Result<Vec<BackupReader>>>()
        .expect("Could not find backup:");
    let mut merger = BackupMerger::new(path, backups, all).expect("Could not read the backups:");
    let count;
    if verbose {
        eprintln!("Files in the merged backup:");
        count = merger
            .files
            .iter()
            .filter(|(b, f)| {
                println!("{}", f);
                *b
            })
            .count();
        eprintln!();
    } else {
        count = merger.files.iter().filter(|(b, _)| *b).count();
    }
    if dry {
        return;
    }

    let bar = if quiet {
        ProgressBar::hidden()
    } else {
        ProgressBar::new(count as u64 + 1)
    };
    bar.set_style(ProgressStyle::default_bar().template(
        "{wide_msg} {pos:>8} / {len:<8}\n{wide_bar} {elapsed_precise:>8} / {duration_precise:<8}",
    ));
    bar.set_message("Merging backups...");
    bar.tick();
    bar.enable_steady_tick(1000);

    let path = merger
        .write(
            |fi: &mut FileInfo, err| {
                bar.set_message(fi.move_string());
                bar.inc(1);
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
        .expect("Could not merge the backups:");
    bar.disable_steady_tick();
    bar.set_message("Merge complete!");
    bar.finish();

    merger
        .cleanup(Some(path), delete, force)
        .expect("Could not cleanup backup files:");
}
