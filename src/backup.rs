use core::panic;
use std::{collections::VecDeque, path::PathBuf};

use chrono::NaiveDateTime;

use crate::config::Config;

#[allow(unused_variables)]
pub fn backup(config: &Config, dry: bool, time: NaiveDateTime) {
    if config.verbose {
        FileCrawler::new(&config.include, &config.exclude, &config.regex)
            .for_each(|f| println! {"{}", f.to_string_lossy()});
    }
    panic!("Backupping is not yet implemented");
}

pub fn get_previous_time<'a>(config: &Config, time: &str) -> NaiveDateTime {
    if !config.incremental {
        NaiveDateTime::from_timestamp(0, 0)
    } else if time == "" {
        panic!("Incremental backup is not implemented");
    } else {
        panic!("Incremental backup is not implemented");
        // TODO: NaiveDateTime::parse_from_str
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
    threads: u32,
    dry: bool,
) {
    panic!("Restoring is not implemented");
}

#[allow(unused_variables)]
pub fn browse(source: &str, regex: Vec<&str>) {
    panic!("Browsing is not implemented");
}

struct FileCrawler {
    include: Vec<PathBuf>,
    exclude: Vec<PathBuf>,
    regex: Vec<String>,
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
                                // self.regex.iter().for_each(|r| {
                                //     // TODO: REGEX
                                // });
                                if !filtered {
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
    fn new(include: &Vec<String>, exclude: &Vec<String>, regex: &Vec<String>) -> Self {
        let mut include: Vec<PathBuf> = include.iter().map(|s| PathBuf::from(s)).collect();
        include.sort_unstable_by(|a, b| b.cmp(a));
        let mut exclude: Vec<PathBuf> = exclude.iter().map(|s| PathBuf::from(s)).collect();
        exclude.sort_unstable_by(|a, b| b.cmp(a));
        let regex = regex.iter().map(|s| String::from(s)).collect();

        Self {
            include,
            exclude,
            regex,
            stack: VecDeque::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileCrawler;
    use std::path::PathBuf;

    #[test]
    fn file_crawler() {
        let files: Vec<PathBuf> = FileCrawler::new(
            &vec!["src".to_string()],
            &vec!["src/main.rs".to_string()],
            &vec![],
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
        assert_eq!(
            files
                .iter()
                .filter(|f| **f == PathBuf::from("src/backup.rs"))
                .count(),
            1
        );
    }
}
