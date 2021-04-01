use std::{
    collections::VecDeque,
    fmt::Display,
    path::{Path, PathBuf},
};

use chrono::NaiveDateTime;
use path_absolutize::Absolutize;
use path_clean::PathClean;
use regex::RegexSet;

use crate::parse_date;

#[derive(Debug)]
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

    pub fn move_string(&mut self) -> String {
        if self.string.is_none() {
            self.path.as_ref().unwrap().to_string_lossy().to_string()
        } else {
            let str = std::mem::replace(&mut self.string, None).unwrap();
            if self.path.is_none() {
                self.path = Some(PathBuf::from(&str));
            }
            str
        }
    }
}

impl Display for FileInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.string {
            Some(s) => write!(f, "FileInfo({})", s),
            None => write!(
                f,
                "FileInfo({})",
                self.path.as_ref().unwrap().to_string_lossy()
            ),
        }
    }
}

#[derive(Debug)]
pub struct FileAccessError {
    error: std::io::Error,
    path: String,
}

impl std::fmt::Display for FileAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not access '{}': {}", self.path, self.error)
    }
}

impl std::error::Error for FileAccessError {}

impl FileAccessError {
    fn new(error: std::io::Error, path: String) -> Self {
        Self { error, path }
    }
}

/// Iterator for crawling through files to backup
pub struct FileCrawler {
    stack: VecDeque<FileInfo>,
    regex: RegexSet,
}

impl FileCrawler {
    pub fn new<
        S1: AsRef<str>,
        S2: AsRef<str>,
        S3: AsRef<str>,
        VS1: AsRef<[S1]>,
        VS2: AsRef<[S2]>,
        VS3: AsRef<[S3]>,
    >(
        include: VS1,
        exclude: VS2,
        filter: VS3,
        local: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut stack: VecDeque<FileInfo>;
        let exc: Vec<String>;
        if local {
            stack = include
                .as_ref()
                .iter()
                .map(|s| FileInfo::from(PathBuf::from(s.as_ref()).clean()))
                .collect();
            exc = exclude
                .as_ref()
                .iter()
                .map(|s| {
                    format!(
                        "^{}$",
                        regex::escape(&PathBuf::from(s.as_ref()).clean().to_string_lossy())
                    )
                })
                .collect::<Vec<String>>();
        } else {
            stack = include
                .as_ref()
                .iter()
                .map(|s| {
                    PathBuf::from(s.as_ref())
                        .absolutize()
                        .map(|p| FileInfo::from(p.to_path_buf()))
                })
                .collect::<std::io::Result<VecDeque<FileInfo>>>()?;
            exc = exclude
                .as_ref()
                .iter()
                .map(|s| {
                    PathBuf::from(s.as_ref())
                        .absolutize()
                        .map(|p| format!("^{}$", regex::escape(&p.to_string_lossy())))
                })
                .collect::<std::io::Result<Vec<String>>>()?;
        }
        stack
            .make_contiguous()
            .sort_unstable_by(|a, b| a.path.as_ref().unwrap().cmp(b.path.as_ref().unwrap()));

        let regex = RegexSet::new(
            filter
                .as_ref()
                .into_iter()
                .map(|s| s.as_ref())
                .chain(exc.iter().map(|s| s.as_str())),
        )?;

        Ok(Self { stack, regex })
    }
}

impl Iterator for FileCrawler {
    type Item = Result<FileInfo, FileAccessError>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(mut item) = self.stack.pop_front() {
            let md = try_some!(item
                .get_path()
                .metadata()
                .map_err(|e| FileAccessError::new(e, item.move_string())));
            if md.is_file() {
                item.time = Some(parse_date::system_to_naive(try_some!(md
                    .modified()
                    .map_err(|e| FileAccessError::new(e, item.move_string())))));
                return Some(Ok(item));
            } else {
                let mut count: usize = 0;
                let dir = try_some!(if item.get_path().as_os_str() == "." {
                    PathBuf::from("")
                        .read_dir()
                        .map_err(|e| FileAccessError::new(e, item.move_string()))
                } else {
                    item.get_path()
                        .read_dir()
                        .map_err(|e| FileAccessError::new(e, item.move_string()))
                });
                for f in dir {
                    let entry =
                        try_some!(f.map_err(|e| FileAccessError::new(e, item.move_string())));
                    let path = entry.path();
                    let string = path.to_string_lossy();
                    if !self.regex.is_match(&string) {
                        let string = string.to_string();
                        let fi = FileInfo::from_both(path, string);
                        self.stack.push_front(fi);
                        count += 1;
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
        let main_path = Path::new("src").join("main.rs");
        let files: Vec<PathBuf> = FileCrawler::new(
            &vec!["src".to_string()],
            &vec![main_path.to_string_lossy()],
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
        files.iter().for_each(|f| assert_ne!(*f, main_path));
        files
            .iter()
            .for_each(|f| assert_ne!(*f, Path::new("src").join("config.rs")));
        assert_eq!(
            files
                .iter()
                .filter(|f| **f == Path::new("src").join("backup.rs"))
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
