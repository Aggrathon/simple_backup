/// This module contains the logic for running the program from a command line
use core::panic;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
            eprintln!(
                "Updated files to backup (since {}):",
                bw.config.time.unwrap()
            );
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
        ).expect("The progressbar template is wrong!"));
        bar.set_message("Compressing file list");
        bar.tick();
        bar.enable_steady_tick(Duration::from_secs(1));
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
#[allow(clippy::too_many_arguments)]
pub fn restore<P: AsRef<Path>>(
    mut source: BackupReader,
    output: Option<P>,
    #[allow(unused_mut)] mut include: Vec<String>,
    regex: Vec<String>,
    flatten: bool,
    only_this: bool,
    force: bool,
    verbose: bool,
    dry: bool,
    quiet: bool,
) {
    source.get_meta().expect("Could not read the backup");
    let only_this = {
        let conf = source.get_config().expect("Could not read the backup");
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
    let mut list: Vec<&str> = if !regex.is_empty() {
        let regex = RegexSet::new(regex).expect("Could not parse regex");
        tmp1 = source
            .move_list()
            .expect("Could not get list of files from backup");
        if only_this {
            tmp1.iter_included().filter(|f| regex.is_match(f)).collect()
        } else {
            tmp1.iter()
                .map(|v| v.1)
                .filter(|f| regex.is_match(f))
                .collect()
        }
    } else if include.is_empty() {
        tmp1 = source
            .move_list()
            .expect("Could not get list of files from backup");
        if only_this {
            tmp1.iter_included().collect()
        } else {
            tmp1.iter().map(|v| v.1).collect()
        }
    } else {
        vec![]
    };
    if !include.is_empty() {
        list.reserve(include.len());
        #[cfg(target_os = "windows")]
        include.iter_mut().for_each(|s| *s = s.replace('\\', "/"));
        list.extend(include.iter().map(|s| s.as_str()));
        list.sort_unstable();
    }

    if list.is_empty() {
        if !quiet {
            eprintln!("No files to backup");
        }
        return;
    }
    if verbose {
        eprintln!("Files to restore:");
        for f in list.iter() {
            println!("{}", f);
        }
        eprintln!();
    }

    if !dry {
        let bar = if quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(list.len() as u64)
        };
        bar.set_style(ProgressStyle::default_bar().template(
            "{wide_msg} {pos:>8} / {len:<8}\n{wide_bar} {elapsed_precise:>8} / {duration_precise:<8}"
        ).expect("The progressbar template is wrong!"));
        bar.set_message("Restoring files");
        bar.tick();
        bar.enable_steady_tick(Duration::from_secs(1));

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
            source.restore(list, path_transform, callback, force, !only_this)
        } else if let Some(o) = &output {
            let path_transform = |mut fi: FileInfo| {
                let s = fi.move_string();
                let path = strip_absolute_from_path(&s);
                bar.set_message(s);
                FileInfo::from(o.as_ref().join(path))
            };
            source.restore(list, path_transform, callback, force, !only_this)
        } else {
            let path_transform = |mut fi: FileInfo| {
                bar.set_message(fi.move_string());
                fi
            };
            source.restore(list, path_transform, callback, force, !only_this)
        }
        .expect("Could not restore from backup");

        bar.disable_steady_tick();
        bar.set_message("Restoration Completed!");
        bar.finish();
    }
}

/// Inspect backup metadata
pub fn inspect(mut source: BackupReader, config: bool, list: bool, quiet: bool) {
    let backup = source.path.move_string();
    let mut decoder = source.get_decoder().expect("Could not open the backup");
    let mut entries = decoder.entries().expect("Could not read the backup");
    if config {
        let (mut fi, mut entry) = entries
            .next()
            .expect("No config found")
            .expect("Could not read the backup");
        if !quiet {
            eprintln!("{} > {}:", backup, fi.move_string());
        }
        let mut conf = String::new();
        entry
            .read_to_string(&mut conf)
            .expect("Could not read the backup");
        if !quiet {
            print!("{}", conf);
        }
    } else {
        entries.next();
    }
    if list {
        let (mut fi, mut entry) = entries
            .next()
            .expect("No file list found")
            .expect("Could not read the backup");
        if config && !quiet {
            eprint!("{} > {}:", backup, fi.move_string());
            println!();
        } else if !quiet {
            eprintln!("{} > {}:", backup, fi.move_string());
        }
        let mut conf = String::new();
        entry
            .read_to_string(&mut conf)
            .expect("Could not read the backup");
        if !quiet {
            println!("{}", conf);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn merge(
    backups: Vec<PathBuf>,
    path: Option<PathBuf>,
    all: bool,
    delete: bool,
    quality: Option<i32>,
    threads: Option<u32>,
    verbose: bool,
    force: bool,
    dry: bool,
    quiet: bool,
) {
    let backups = backups
        .into_iter()
        .flat_map(|p| BackupIterator::path(p).expect("Could not find backup"))
        .map(|r| r.map(BackupReader::new))
        .collect::<std::io::Result<Vec<BackupReader>>>()
        .expect("Could not find backup");
    let mut merger = BackupMerger::new(path, backups, all, delete, force, quality, threads)
        .map_err(|(_, e)| e)
        .expect("Could not read the backups");
    let count;
    if verbose {
        eprintln!("Files in the merged backup:");
        count = merger
            .files
            .iter()
            .filter(|(b, f)| {
                println!("{}", f.copy_string());
                *b
            })
            .count();
        eprintln!();
        eprintln!(
            "Storing the merged backup in: {}",
            merger.path.to_string_lossy()
        );
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
    ).expect("The progressbar template is wrong!"));
    bar.set_message("Merging backups...");
    bar.tick();
    bar.enable_steady_tick(Duration::from_secs(1));

    merger
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
        .expect("Could not merge the backups");
    bar.disable_steady_tick();
    bar.set_message("Merge complete!");
    bar.finish();
}
