use std::{
    collections::VecDeque,
    fmt::Write,
    path::{Path, PathBuf},
};

use chrono::NaiveDateTime;
use path_absolutize::Absolutize;
use regex::Regex;

use crate::parse_date;

macro_rules! some_box_try {
    ($value:expr) => {
        match $value {
            Ok(v) => v,
            Err(e) => return Some(Err(Box::new(e))),
        }
    };
}

pub enum PathString {
    Path(PathBuf),
    String(String),
    Both(PathBuf, String),
}

/// Struct for holding information about files from crawling or file lists
pub struct FileInfo {
    pub path: PathString,
    pub time: Option<NaiveDateTime>,
}

impl From<PathBuf> for FileInfo {
    fn from(path: PathBuf) -> Self {
        FileInfo {
            path: PathString::Path(path),
            time: None,
        }
    }
}
impl From<&Path> for FileInfo {
    fn from(path: &Path) -> Self {
        FileInfo {
            path: PathString::Path(path.to_path_buf()),
            time: None,
        }
    }
}

impl From<String> for FileInfo {
    fn from(path: String) -> Self {
        FileInfo {
            path: PathString::String(path),
            time: None,
        }
    }
}

impl From<&str> for FileInfo {
    fn from(path: &str) -> Self {
        FileInfo {
            path: PathString::String(path.to_string()),
            time: None,
        }
    }
}

impl FileInfo {
    pub fn new_str(path: &str, time: Option<NaiveDateTime>) -> Self {
        FileInfo {
            path: PathString::String(path.to_string()),
            time: time,
        }
    }

    pub fn from_file(path: PathBuf) -> std::io::Result<Self> {
        let time = parse_date::system_to_naive(path.metadata()?.modified()?);
        Ok(FileInfo {
            path: PathString::Path(path),
            time: Some(time),
        })
    }

    pub fn from_file2(path: PathBuf, string: String) -> std::io::Result<Self> {
        let time = parse_date::system_to_naive(path.metadata()?.modified()?);
        Ok(FileInfo {
            path: PathString::Both(path, string),
            time: Some(time),
        })
    }

    pub fn from_csv(csv: &str) -> Result<Self, &str> {
        let mut split = csv.splitn(2, ',');
        let time = split.next().ok_or("File info is missing")?;
        let string = split.next().ok_or("Could not split at ','")?;
        Ok(FileInfo {
            time: if time.len() == 0 {
                None
            } else {
                Some(
                    NaiveDateTime::parse_from_str(time, parse_date::FORMAT)
                        .map_err(|_| "Could not parse the time")?,
                )
            },
            path: PathString::String(String::from(string)),
        })
    }

    pub fn to_csv(&mut self) -> String {
        match self.time {
            None => ",".to_string() + self.get_string(),
            Some(t) => format!("{},{}", t.format(parse_date::FORMAT), self.get_string()),
        }
    }

    pub fn to_writer<W: Write>(&mut self, writer: &mut W) -> Result<(), std::fmt::Error> {
        if let Some(t) = self.time {
            write!(writer, "{}", t.format(parse_date::FORMAT))?;
        };
        writer.write_char(',')?;
        writer.write_str(self.get_string())?;
        writer.write_char('\n')
    }

    pub fn get_string(&mut self) -> &String {
        if let PathString::Path(_) = self.path {
            if let PathString::Path(p) =
                std::mem::replace(&mut self.path, PathString::String(String::new()))
            {
                let s = p.to_string_lossy().to_string();
                self.path = PathString::Both(p, s);
            }
        };
        match &self.path {
            PathString::Path(_) => panic!("This should not be possible (FileInfo::get_string)"),
            PathString::String(s) => &s,
            PathString::Both(_, s) => &s,
        }
    }

    pub fn get_path(&mut self) -> &PathBuf {
        if let PathString::String(_) = self.path {
            if let PathString::String(s) =
                std::mem::replace(&mut self.path, PathString::String(String::new()))
            {
                let p = PathBuf::from(&s);
                self.path = PathString::Both(p, s);
            }
        };
        match &self.path {
            PathString::Path(p) => &p,
            PathString::String(_) => panic!("This should not be possible (FileInfo::get_path)"),
            PathString::Both(p, _) => &p,
        }
    }

    pub fn consume_path(self) -> PathBuf {
        match self.path {
            PathString::Path(p) => p,
            PathString::String(_) => panic!("This should not be possible (FileInfo::get_path)"),
            PathString::Both(p, _) => p,
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
                let md = some_box_try!(item.get_path().metadata());
                if md.is_file() {
                    return Some(Ok(item));
                } else {
                    let mut count: usize = 0;
                    let dir = some_box_try!(item.get_path().read_dir());
                    for f in dir {
                        let entry = some_box_try!(f);
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
                                let fi = some_box_try!(FileInfo::from_file2(path, string));
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
        let mut fi1 = FileInfo::from_file(PathBuf::from("cargo.toml")).unwrap();
        let mut fi2 =
            FileInfo::from_file2(PathBuf::from("cargo.toml"), String::from("cargo.toml")).unwrap();
        let mut fi3 = FileInfo::from(PathBuf::from("cargo.toml"));
        let mut fi4 = FileInfo::from(String::from("cargo.toml"));
        let mut fi5 = FileInfo::from("cargo.toml");
        let mut fi6 = FileInfo::from(Path::new("cargo.toml"));
        let mut fi7 = FileInfo::from_csv(",cargo.toml").unwrap();
        let mut fi8 = FileInfo::from_csv("2012-12-12 12:12:12,cargo.toml").unwrap();

        assert_eq!(fi1.get_string(), fi2.get_string());
        assert_eq!(fi3.get_string(), fi2.get_string());
        assert_eq!(fi3.get_string(), fi4.get_string());
        assert_eq!(fi5.get_string(), fi4.get_string());
        assert_eq!(fi5.get_string(), fi6.get_string());
        assert_eq!(fi7.get_string(), fi6.get_string());
        assert_eq!(fi7.get_string(), fi8.get_string());

        assert_eq!(fi1.get_path(), fi2.get_path());
        assert_eq!(fi3.get_path(), fi2.get_path());
        assert_eq!(fi3.get_path(), fi4.get_path());
        assert_eq!(fi5.get_path(), fi4.get_path());
        assert_eq!(fi5.get_path(), fi6.get_path());
        assert_eq!(fi7.get_path(), fi6.get_path());
        assert_eq!(fi7.get_path(), fi8.get_path());

        assert_eq!(fi1.to_csv(), fi2.to_csv());
        assert_eq!(fi3.to_csv(), fi4.to_csv());
        assert_eq!(fi5.to_csv(), fi4.to_csv());
        assert_eq!(fi5.to_csv(), fi6.to_csv());
        assert_eq!(fi7.to_csv(), fi6.to_csv());
    }
}
