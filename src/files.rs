/// This module contains the FileInfo struct and a file crawler
use std::fmt::Display;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use path_absolutize::Absolutize;
use path_clean::PathClean;
use regex::RegexSet;

use crate::parse_date;

/// A struct that contains both the PathBuf and String versions of a path
#[derive(Debug, Eq, Ord)]
pub struct FileInfo {
    string: Option<String>,
    path: Option<PathBuf>,
    pub time: Option<NaiveDateTime>,
    pub size: u64,
}

impl From<PathBuf> for FileInfo {
    fn from(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            string: None,
            time: None,
            size: 0,
        }
    }
}
impl From<&Path> for FileInfo {
    fn from(path: &Path) -> Self {
        Self {
            path: Some(path.to_path_buf()),
            string: None,
            time: None,
            size: 0,
        }
    }
}

impl From<String> for FileInfo {
    fn from(path: String) -> Self {
        Self {
            path: None,
            string: Some(path),
            time: None,
            size: 0,
        }
    }
}

impl From<&str> for FileInfo {
    fn from(path: &str) -> Self {
        Self {
            path: None,
            string: Some(path.to_string()),
            time: None,
            size: 0,
        }
    }
}

impl PartialEq for FileInfo {
    fn eq(&self, other: &Self) -> bool {
        if let Some(s1) = self.string.as_ref() {
            if let Some(s2) = other.string.as_ref() {
                return s1 == s2;
            }
        }
        if let Some(p1) = self.path.as_ref() {
            if let Some(p2) = other.path.as_ref() {
                return p1 == p2;
            }
        }
        false
    }
}

impl PartialOrd for FileInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if let Some(s1) = self.string.as_ref() {
            if let Some(s2) = other.string.as_ref() {
                return Some(s1.cmp(s2));
            }
        }
        if let Some(p1) = self.path.as_ref() {
            if let Some(p2) = other.path.as_ref() {
                return Some(p1.cmp(p2));
            }
        }
        None
    }
}

impl FileInfo {
    /// Create a FileInfo from a Pathbuf and a String
    pub fn from_both(path: PathBuf, string: String) -> Self {
        Self {
            path: Some(path),
            string: Some(string),
            time: None,
            size: 0,
        }
    }

    /// Returns the String version (with lazy conversion)
    pub fn get_string(&mut self) -> &String {
        if self.string.is_none() {
            self.string = Some(self.path.as_ref().unwrap().to_string_lossy().to_string())
        }
        self.string.as_ref().unwrap()
    }

    /// Returns the PathBuf version (with lazy conversion)
    pub fn get_path(&mut self) -> &PathBuf {
        if self.path.is_none() {
            self.path = Some(PathBuf::from(self.string.as_ref().unwrap()))
        }
        self.path.as_ref().unwrap()
    }

    /// Convert the FileInfo into a PathBuf
    pub fn consume_path(self) -> PathBuf {
        match self.path {
            Some(path) => path,
            None => PathBuf::from(&self.string.unwrap()),
        }
    }

    /// Move the String version out (with minimal allocation)
    pub fn move_string(&mut self) -> String {
        if self.string.is_none() {
            self.path.as_ref().unwrap().to_string_lossy().to_string()
        } else if self.path.is_none() {
            self.string.as_ref().unwrap().to_string()
        } else {
            std::mem::replace(&mut self.string, None).unwrap()
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
    temp: Vec<(FileInfo, DirEntry)>,
    stack: Vec<FileInfo>,
    regex: RegexSet,
    local: bool,
}

impl FileCrawler {
    /// Create an iterator over files to be added to a backup
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
        let mut stack: Vec<FileInfo>;
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
                .collect::<std::io::Result<Vec<FileInfo>>>()?;
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
        stack.sort_unstable_by(|a, b| b.path.as_ref().unwrap().cmp(a.path.as_ref().unwrap()));

        let regex = RegexSet::new(
            filter
                .as_ref()
                .into_iter()
                .map(|s| s.as_ref())
                .chain(exc.iter().map(|s| s.as_str())),
        )?;

        Ok(Self {
            stack,
            regex,
            temp: vec![],
            local,
        })
    }
}

#[cfg(target_os = "windows")]
fn dir_read<P: AsRef<Path>>(
    dir: P,
) -> std::io::Result<impl Iterator<Item = std::io::Result<DirEntry>>> {
    if dir.as_ref().as_os_str() == "." {
        Path::new("").read_dir()
    } else {
        dir.as_ref().read_dir()
    }
}

#[cfg(not(target_os = "windows"))]
fn dir_read<P: AsRef<Path>>(
    dir: P,
) -> std::io::Result<impl Iterator<Item = std::io::Result<DirEntry>>> {
    dir.as_ref().read_dir()
}

#[cfg(target_os = "windows")]
fn dir_path(d: &DirEntry, _local: bool) -> PathBuf {
    d.path()
}

#[cfg(not(target_os = "windows"))]
fn dir_path(d: &DirEntry, local: bool) -> PathBuf {
    let path = d.path();
    if local && path.is_relative() {
        path.clean()
    } else {
        path
    }
}

impl Iterator for FileCrawler {
    type Item = Result<FileInfo, FileAccessError>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(mut item) = self.stack.pop() {
            let md = try_some!(item
                .get_path()
                .metadata()
                .map_err(|e| FileAccessError::new(e, item.move_string())));
            if md.is_file() {
                item.time = Some(parse_date::system_to_naive(try_some!(md
                    .modified()
                    .map_err(|e| FileAccessError::new(e, item.move_string())))));
                item.size = md.len();
                return Some(Ok(item));
            } else {
                let string = item.move_string();
                let path = item.consume_path();
                let dir =
                    try_some!(dir_read(path).map_err(|e| FileAccessError::new(e, string.clone())));
                for f in dir {
                    let entry = try_some!(f.map_err(|e| FileAccessError::new(e, string.clone())));
                    let path = dir_path(&entry, self.local);
                    let string = path.to_string_lossy();
                    if !self.regex.is_match(&string) {
                        let string = string.to_string();
                        let fi = FileInfo::from_both(path, string);
                        self.temp.push((fi, entry));
                    }
                }
                if self.temp.len() > 0 {
                    // Sort the added items to preserve lexicographic ordering
                    self.temp
                        .sort_unstable_by(|a, b| a.1.file_name().cmp(&b.1.file_name()));
                    // Check for items already on the stack
                    let mut count = self.stack.len();
                    let mut needs_sorting = false;
                    if count > 0 {
                        count -= 1;
                        for (fi1, _) in self.temp.iter() {
                            // SAFETY: count is guaranteed to be between zero and self.stack.len()
                            let fi2 = unsafe { self.stack.get_unchecked(count) };
                            match fi1.path.as_ref().unwrap().cmp(&fi2.path.as_ref().unwrap()) {
                                std::cmp::Ordering::Less => {}
                                std::cmp::Ordering::Equal => {
                                    self.stack.remove(count);
                                    if count == 0 {
                                        break;
                                    } else {
                                        count -= 1;
                                    }
                                }
                                std::cmp::Ordering::Greater => {
                                    needs_sorting = true;
                                    if count == 0 {
                                        break;
                                    } else {
                                        count -= 1;
                                    }
                                }
                            }
                        }
                    }
                    // Add new items to the stack
                    while let Some((fi, _)) = self.temp.pop() {
                        self.stack.push(fi);
                    }
                    // If the top of the stack is not sorted
                    if needs_sorting {
                        self.stack[count..].sort_unstable_by(|a, b| {
                            b.path.as_ref().unwrap().cmp(&a.path.as_ref().unwrap())
                        });
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
