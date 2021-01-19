use crate::{compression::CompressionDecoder, config::Config};
use crate::{compression::CompressionEncoder, utils};
use chrono::{offset::TimeZone, DateTime, Local, NaiveDateTime};
use path_absolutize::Absolutize;
use regex::Regex;
use std::{cmp::max, collections::VecDeque, io::Read, path::PathBuf, time::SystemTime};
use utils::ProgressBar;

/// Backup files
pub fn backup(config: &mut Config, dry: bool) {
    // Check for overwrite and collect timestamp
    let current_time = DateTime::<Local>::from(SystemTime::now()).naive_local();
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
    if config.verbose {
        println!("Files to backup:");
    } else {
        println!("Crawling for files...");
    }
    let mut files_all: Vec<PathBuf> = vec![];

    // Crawl for incremental files
    if config.incremental {
        if let Some(time_prev) = get_previous_time(&config) {
            let time_prev: SystemTime = Local.from_local_datetime(&time_prev).unwrap().into();
            files_all = FileCrawler::new(
                &config.include,
                &config.exclude,
                &config.regex,
                config.local,
            )
            .into_iter()
            .filter(|path| {
                path.metadata()
                    .and_then(|m| m.modified())
                    .and_then(|t| {
                        let fresh = t > time_prev;
                        if config.verbose || !dry {
                            let string = &path.to_string_lossy();
                            if !dry {
                                files_string.push(if fresh { '1' } else { '0' });
                                files_string.push(',');
                                files_string.push_str(&string);
                                files_string.push('\n');
                            }
                            if config.verbose && fresh {
                                println!("{}", &string);
                            }
                        }
                        Ok(fresh)
                    })
                    .unwrap_or(false)
            })
            .collect();
        }
    }
    // Crawl for all files
    if files_all.is_empty() {
        files_all = FileCrawler::new(
            &config.include,
            &config.exclude,
            &config.regex,
            config.local,
        )
        .map(|path| {
            if config.verbose || !dry {
                let string = &path.to_string_lossy();
                if !dry {
                    files_string.push('1');
                    files_string.push(',');
                    files_string.push_str(&string);
                    files_string.push('\n');
                }
                if config.verbose {
                    println!("{}", &string);
                }
            }
            path
        })
        .collect();
    }

    // Perform the backup
    if !dry {
        if config.verbose {
            println!("");
        }
        config.time = Some(current_time);
        let mut comp = CompressionEncoder::create(&output, config.quality);
        comp.append_data("config.yml", &config.to_yaml());
        comp.append_data("files.csv", &files_string);
        let mut bar = ProgressBar::start(files_all.len(), 80, "Backing up files");
        for path in files_all.iter() {
            comp.append_file(path);
            bar.progress();
        }
        comp.close();
    }
}

/// Check the config (arguments) or previous backups for a time limit in case of an incremental backup
pub fn get_previous_time(config: &Config) -> Option<NaiveDateTime> {
    if !config.incremental {
        None
    } else if let Some(t) = config.time {
        Some(t)
    } else {
        let mut time = None;
        for path in config.get_previous() {
            if path.is_err() {
                eprintln!("Could not find previous backup: {}", path.unwrap_err());
                continue;
            }
            let path = path.unwrap();
            let dec = CompressionDecoder::read(&path);
            if dec.is_err() {
                eprintln!(
                    "Could not open previous backup '{}': {}",
                    path.to_string_lossy(),
                    dec.err().unwrap()
                );
                continue;
            }
            let mut dec = dec.unwrap();
            let entries = dec.entries();
            if entries.is_err() {
                eprintln!(
                    "Could not read previous backup '{}': {}",
                    path.to_string_lossy(),
                    entries.err().unwrap()
                );
                continue;
            }
            let entry = entries.unwrap().next();
            if entry.is_none() {
                eprintln!("The previous backup is empty: {}", path.to_string_lossy());
                continue;
            }
            let entry = entry.unwrap();
            if entry.is_err() {
                eprintln!(
                    "Could not open the config for '{}': {}",
                    path.to_string_lossy(),
                    entry.err().unwrap()
                );
                continue;
            }
            let mut entry = entry.unwrap();
            if entry.0 != PathBuf::from("config.yml") {
                eprintln!(
                    "The first file in the previous backup is not a config: {}",
                    path.to_string_lossy(),
                );
                continue;
            }
            let mut s = String::new();
            let res = entry.1.read_to_string(&mut s);
            if res.is_err() {
                eprintln!(
                    "Could not read config in previous backup '{}': {}",
                    path.to_string_lossy(),
                    res.unwrap_err()
                );
                continue;
            }
            let conf = Config::from_yaml(&s);
            if conf.is_err() {
                eprintln!(
                    "Could not parse config in previous backup '{}': {}",
                    path.to_string_lossy(),
                    conf.unwrap_err()
                );
                continue;
            }
            let conf = conf.unwrap();
            if let Some(t1) = time {
                if let Some(t2) = conf.time {
                    time = Some(max(t1, t2))
                }
            } else if let Some(t) = conf.time {
                time = Some(t)
            }
        }
        time
    }
}

/// Iterator for crawling through files to backup
struct FileCrawler {
    include: Vec<PathBuf>,
    exclude: Vec<PathBuf>,
    regex: Vec<Regex>,
    stack: VecDeque<PathBuf>,
}

impl FileCrawler {
    fn new(include: &Vec<String>, exclude: &Vec<String>, regex: &Vec<String>, local: bool) -> Self {
        let mut inc: Vec<PathBuf>;
        let mut exc: Vec<PathBuf>;
        if local {
            inc = include.iter().map(|s| PathBuf::from(s)).collect();
            exc = exclude.iter().map(|s| PathBuf::from(s)).collect();
        } else {
            inc = include
                .iter()
                .filter_map(|s| match PathBuf::from(s).absolutize() {
                    Ok(p) => Some(p.to_path_buf()),
                    Err(e) => {
                        eprintln!("Could not convert to absolute path: {}", e);
                        None
                    }
                })
                .collect();
            exc = exclude
                .iter()
                .filter_map(|s| match PathBuf::from(s).absolutize() {
                    Ok(p) => Some(p.to_path_buf()),
                    Err(e) => {
                        eprintln!("Could not convert to absolute path: {}", e);
                        None
                    }
                })
                .collect();
        }
        inc.sort_unstable_by(|a, b| b.cmp(a));
        exc.sort_unstable_by(|a, b| b.cmp(a));
        let regex = regex
            .iter()
            .map(|s| Regex::new(s).expect("Could not parse regex"))
            .collect();

        Self {
            include: inc,
            exclude: exc,
            regex,
            stack: VecDeque::new(),
        }
    }
}

impl Iterator for FileCrawler {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while let Some(item) = self.stack.pop_front() {
                if item.is_dir() {
                    let mut count: usize = 0;
                    match item.read_dir() {
                        Ok(dir) => dir.for_each(|f| match f {
                            Ok(entry) => {
                                let path = entry.path();
                                let mut filtered = false;
                                while let Some(p) = self.include.last() {
                                    if *p <= path {
                                        self.include.pop().unwrap();
                                    } else {
                                        break;
                                    }
                                }
                                while let Some(p) = self.exclude.last() {
                                    if *p == path {
                                        self.exclude.pop().unwrap();
                                        filtered = true;
                                    } else if *p < path {
                                        self.exclude.pop().unwrap();
                                    } else {
                                        break;
                                    }
                                }
                                if !filtered {
                                    let string = path.to_string_lossy();
                                    if !self.regex.iter().any(|r| r.is_match(&string)) {
                                        self.stack.push_front(path);
                                        count += 1;
                                    }
                                }
                            }
                            Err(e) => {
                                eprint!("Could not read file: {}", e)
                            }
                        }),
                        Err(e) => eprint!("Could not read directory: {}", e),
                    };
                    // Reverse the order of the added items to preserve lexicographic ordering
                    if count > 1 {
                        for i in 0..(count / 2) {
                            self.stack.swap(i, count - i - 1);
                        }
                    }
                } else if item.is_file() {
                    return Some(item);
                }
            }
            if self.include.len() > 0 {
                self.stack.push_back(self.include.pop().unwrap());
            } else {
                break;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::FileCrawler;
    use path_absolutize::Absolutize;
    use std::path::PathBuf;

    #[test]
    fn file_crawler_abs() {
        let files: Vec<PathBuf> = FileCrawler::new(
            &vec!["src".to_string()],
            &vec!["src/main.rs".to_string()],
            &vec!["config.*".to_string()],
            false,
        )
        .collect();
        files
            .iter()
            .take(files.len() - 1)
            .zip(files.iter().skip(1))
            .for_each(|(a, b)| assert!(a < b));
        files
            .iter()
            .for_each(|f| assert_ne!(*f, PathBuf::from("src/main.rs").absolutize().unwrap()));
        files
            .iter()
            .for_each(|f| assert_ne!(*f, PathBuf::from("src/config.rs").absolutize().unwrap()));
        assert_eq!(
            files
                .iter()
                .filter(|f| **f == PathBuf::from("src/backup.rs").absolutize().unwrap())
                .count(),
            1
        );
    }
    #[test]
    fn file_crawler_rel() {
        let files: Vec<PathBuf> = FileCrawler::new(
            &vec!["src".to_string()],
            &vec!["src/main.rs".to_string()],
            &vec!["config.*".to_string()],
            true,
        )
        .collect();
        files
            .iter()
            .take(files.len() - 1)
            .zip(files.iter().skip(1))
            .for_each(|(a, b)| assert!(a < b));
        files
            .iter()
            .for_each(|f| assert_ne!(*f, PathBuf::from("src/main.rs")));
        files
            .iter()
            .for_each(|f| assert_ne!(*f, PathBuf::from("src/config.rs")));
        assert_eq!(
            files
                .iter()
                .filter(|f| **f == PathBuf::from("src/backup.rs"))
                .count(),
            1
        );
    }
}
