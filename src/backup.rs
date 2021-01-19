use crate::{compression::CompressionDecoder, config::Config};
use crate::{compression::CompressionEncoder, utils};
use chrono::{offset::TimeZone, DateTime, Local, NaiveDateTime};
use core::panic;
use path_absolutize::Absolutize;
use regex::Regex;
use std::{cmp::max, collections::VecDeque, io::Read, path::PathBuf, time::SystemTime};
use utils::ProgressBar;

pub fn backup(config: &mut Config, dry: bool) {
    let mut files_all: Vec<PathBuf> = FileCrawler::new(
        &config.include,
        &config.exclude,
        &config.regex,
        config.local,
    )
    .collect();
    let files_list: String = files_all
        .iter()
        .map(|f| f.to_string_lossy() + "\n")
        .collect();

    if config.incremental {
        if let Some(time_prev) = get_previous_time(&config) {
            let time_prev: SystemTime = Local.from_local_datetime(&time_prev).unwrap().into();
            files_all = files_all
                .into_iter()
                .filter(|path| {
                    path.metadata()
                        .and_then(|m| m.modified())
                        .and_then(|t| Ok(t > time_prev))
                        .unwrap_or(false)
                })
                .collect();
        }
    }

    if config.verbose {
        println!("Files to backup:");
        files_all
            .iter()
            .for_each(|f| println! {"{}", f.to_string_lossy()});
    }

    if !dry {
        config.time = Some(DateTime::<Local>::from(SystemTime::now()).naive_local());
        let output = config.get_output();
        if output.exists() && !config.force {
            panic!("Backup already exists at '{}'", output.to_string_lossy());
        }
        let mut comp = CompressionEncoder::create(&output, config.quality);
        comp.append_data("config.yml", &config.to_yaml());
        comp.append_data("files.txt", &files_list);
        let mut bar = ProgressBar::start(files_all.len(), 80, "Backing up files");
        for path in files_all.iter() {
            comp.append_file(path);
            bar.progress();
        }
        comp.close();
    }
}

#[allow(unreachable_code)]
pub fn get_previous_time(config: &Config) -> Option<NaiveDateTime> {
    if !config.incremental {
        None
    } else if let Some(t) = config.time {
        Some(t)
    } else {
        todo!("Incremental backup is not implemented");
        todo!("Make these warnings instead of panics");
        todo!("Iterate through all possible earlier backups");
        let mut time = config.time;
        let mut dec = CompressionDecoder::read(&PathBuf::from("")).expect("Could not open backup");
        let mut entries = dec.entries().expect("Could not read backup");
        let mut entry = entries
            .next()
            .expect("The backup is empty")
            .expect("Could not open config");
        if entry.0 != PathBuf::from("config.yml") {
            panic!("The first file is not a config");
        }
        let mut s = String::new();
        entry
            .1
            .read_to_string(&mut s)
            .expect("Could not read config");
        let conf = Config::from_yaml(&s);
        if let Some(t1) = time {
            if let Some(t2) = conf.time {
                time = Some(max(t1, t2))
            }
        } else if let Some(t) = conf.time {
            time = Some(t)
        }
        time
    }
}

struct FileCrawler {
    include: Vec<PathBuf>,
    exclude: Vec<PathBuf>,
    regex: Vec<Regex>,
    stack: VecDeque<PathBuf>,
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
                                if !filtered
                                    && !self
                                        .regex
                                        .iter()
                                        .any(|r| r.is_match(&path.to_string_lossy()))
                                {
                                    self.stack.push_front(path);
                                    count += 1;
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
