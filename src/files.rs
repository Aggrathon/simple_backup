use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use chrono::NaiveDateTime;
use path_absolutize::Absolutize;
use regex::Regex;

use crate::parse_date;

pub struct FileInfo {
    path: Option<PathBuf>,
    string: Option<String>,
    pub time: Option<NaiveDateTime>,
}

impl From<PathBuf> for FileInfo {
    fn from(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            string: None,
            time: None,
        }
    }
}
impl From<&Path> for FileInfo {
    fn from(path: &Path) -> Self {
        Self {
            path: Some(path.to_path_buf()),
            string: None,
            time: None,
        }
    }
}

impl From<String> for FileInfo {
    fn from(path: String) -> Self {
        Self {
            path: None,
            string: Some(path),
            time: None,
        }
    }
}

impl From<&str> for FileInfo {
    fn from(path: &str) -> Self {
        Self {
            path: None,
            string: Some(path.to_string()),
            time: None,
        }
    }
}

impl FileInfo {
    pub fn from_both(path: PathBuf, string: String) -> Self {
        Self {
            path: Some(path),
            string: Some(string),
            time: None,
        }
    }

    pub fn get_string(&mut self) -> &String {
        if self.string.is_none() {
            self.string = Some(self.path.as_ref().unwrap().to_string_lossy().to_string())
        }
        self.string.as_ref().unwrap()
    }

    pub fn get_path(&mut self) -> &PathBuf {
        if self.path.is_none() {
            self.path = Some(PathBuf::from(self.string.as_ref().unwrap()))
        }
        self.path.as_ref().unwrap()
    }

    pub fn consume_path(self) -> PathBuf {
        match self.path {
            Some(path) => path,
            None => PathBuf::from(&self.string.unwrap()),
        }
    }
}

/// Iterator for crawling through files to backup
pub struct FileCrawler {
    include: Vec<PathBuf>,
    exclude: Vec<PathBuf>,
    regex: Vec<Regex>,
    stack: VecDeque<FileInfo>,
}

impl FileCrawler {
    pub fn new<S: AsRef<str>, VS: AsRef<[S]>>(
        include: VS,
        exclude: VS,
        regex: VS,
        local: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut inc: Vec<PathBuf>;
        let mut exc: Vec<PathBuf>;
        if local {
            inc = include
                .as_ref()
                .iter()
                .map(|s| PathBuf::from(s.as_ref()))
                .collect();
            exc = exclude
                .as_ref()
                .iter()
                .map(|s| PathBuf::from(s.as_ref()))
                .collect();
        } else {
            inc = include
                .as_ref()
                .iter()
                .map(|s| {
                    PathBuf::from(s.as_ref())
                        .absolutize()
                        .map(|p| p.to_path_buf())
                })
                .collect::<std::io::Result<Vec<PathBuf>>>()?;
            exc = exclude
                .as_ref()
                .iter()
                .map(|s| {
                    PathBuf::from(s.as_ref())
                        .absolutize()
                        .map(|p| p.to_path_buf())
                })
                .collect::<std::io::Result<Vec<PathBuf>>>()?;
        }
        inc.sort_unstable_by(|a, b| b.cmp(a));
        exc.sort_unstable_by(|a, b| b.cmp(a));
        let regex = regex
            .as_ref()
            .iter()
            .map(|s| Regex::new(s.as_ref()))
            .collect::<Result<Vec<Regex>, regex::Error>>()?;

        Ok(Self {
            include: inc,
            exclude: exc,
            regex,
            stack: VecDeque::new(),
        })
    }
}

impl Iterator for FileCrawler {
    type Item = Result<FileInfo, Box<dyn std::error::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while let Some(mut item) = self.stack.pop_front() {
                let md = try_some_box!(item.get_path().metadata());
                if md.is_file() {
                    item.time = Some(parse_date::system_to_naive(try_some_box!(md.modified())));
                    return Some(Ok(item));
                } else {
                    let mut count: usize = 0;
                    let dir = try_some_box!(item.get_path().read_dir());
                    for f in dir {
                        let entry = try_some_box!(f);
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
                                let string = string.to_string();
                                let fi = FileInfo::from_both(path, string);
                                self.stack.push_front(fi);
                                count += 1;
                            }
                        }
                    }
                    // Reverse the order of the added items to preserve lexicographic ordering
                    if count > 1 {
                        for i in 0..(count / 2) {
                            self.stack.swap(i, count - i - 1);
                        }
                    }
                }
            }
            if self.include.len() > 0 {
                self.stack
                    .push_back(FileInfo::from(self.include.pop().unwrap()));
            } else {
                break;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use path_absolutize::Absolutize;

    use super::{FileCrawler, FileInfo};

    #[test]
    fn file_crawler_abs() {
        let files: Vec<PathBuf> = FileCrawler::new(
            &vec!["src".to_string()],
            &vec!["src/main.rs".to_string()],
            &vec!["config.*".to_string()],
            false,
        )
        .unwrap()
        .map(|fi| fi.unwrap().consume_path())
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
        .unwrap()
        .map(|fi| fi.unwrap().consume_path())
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

    #[test]
    fn fileinfo_from() {
        let mut fi1 = FileInfo::from(PathBuf::from("cargo.toml"));
        let mut fi2 = FileInfo::from_both(PathBuf::from("cargo.toml"), String::from("cargo.toml"));
        let mut fi3 = FileInfo::from(PathBuf::from("cargo.toml"));
        let mut fi4 = FileInfo::from(String::from("cargo.toml"));
        let mut fi5 = FileInfo::from("cargo.toml");
        let mut fi6 = FileInfo::from(Path::new("cargo.toml"));

        assert_eq!(fi1.get_string(), fi2.get_string());
        assert_eq!(fi3.get_string(), fi2.get_string());
        assert_eq!(fi3.get_string(), fi4.get_string());
        assert_eq!(fi5.get_string(), fi4.get_string());
        assert_eq!(fi5.get_string(), fi6.get_string());

        assert_eq!(fi1.get_path(), fi2.get_path());
        assert_eq!(fi3.get_path(), fi2.get_path());
        assert_eq!(fi3.get_path(), fi4.get_path());
        assert_eq!(fi5.get_path(), fi4.get_path());
        assert_eq!(fi5.get_path(), fi6.get_path());
    }
}
