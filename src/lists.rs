use std::cmp::Ordering;

use chrono::NaiveDateTime;

use crate::backup::BackupError;
use crate::files::{FileAccessError, FileCrawler, FileInfo};

#[derive(Default)]
pub struct FileListVec(Vec<(bool, FileInfo)>);

impl FileListVec {
    pub fn push(&mut self, included: bool, file: FileInfo) {
        self.0.push((included, file))
    }

    pub fn crawl(crawler: FileCrawler, time: Option<NaiveDateTime>) -> Self {
        let mut list: Vec<(bool, FileInfo)> = match time {
            Some(prev) => crawler
                .into_iter()
                .filter_map(|fi| match fi {
                    Ok(fi) => Some((fi.time.unwrap() >= prev, fi)),
                    Err(_) => None,
                })
                .collect(),
            None => crawler
                .into_iter()
                .filter_map(|fi| match fi {
                    Ok(fi) => Some((true, fi)),
                    Err(_) => None,
                })
                .collect(),
        };
        list.sort_unstable_by(|a, b| a.1.cmp(&b.1));
        Self(list)
    }

    pub fn crawl_with_callback(
        crawler: FileCrawler,
        time: Option<NaiveDateTime>,
        all: bool,
        mut callback: impl FnMut(Result<&mut FileInfo, FileAccessError>) -> Result<(), BackupError>,
    ) -> Result<Self, BackupError> {
        let all = all || time.is_none();
        let mut list: Vec<(bool, FileInfo)> = vec![];
        for f in crawler {
            match f {
                Ok(mut fi) => {
                    let inc = match time {
                        Some(t) => fi.time.unwrap() >= t,
                        None => true,
                    };
                    if all || inc {
                        callback(Ok(&mut fi))?;
                    }
                    list.push((inc, fi));
                }
                Err(e) => callback(Err(e))?,
            }
        }
        list.sort_unstable_by(|a, b| a.1.cmp(&b.1));
        Ok(Self(list))
    }

    pub fn iter(&self) -> impl Iterator<Item = &(bool, FileInfo)> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut (bool, FileInfo)> {
        self.0.iter_mut()
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[allow(unused)]
    pub fn sort_unstable_by<F>(&mut self, mut f: F)
    where
        F: FnMut(&FileInfo, &FileInfo) -> Ordering,
    {
        self.0.sort_unstable_by(|a, b| f(&a.1, &b.1));
    }

    #[allow(unused)]
    pub fn sort_unstable(&mut self) {
        self.0.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    }
}

#[derive(Debug, Clone)]
pub struct FileListString {
    list: String,
    version: u8,
}

impl AsRef<[u8]> for FileListString {
    fn as_ref(&self) -> &[u8] {
        self.list.as_ref()
    }
}

impl FileListString {
    pub fn new<S: AsRef<str>>(filename: S, content: String) -> Result<Self, BackupError> {
        let version = match filename.as_ref() {
            "files.csv" => 1,
            "files_v2.csv" => 2,
            _ => return Err(BackupError::Unspecified),
        };
        Ok(Self {
            list: content,
            version,
        })
    }

    /// Convert a FileListVec to a FileListString
    pub fn from(files: &mut FileListVec) -> Self {
        let mut list = String::with_capacity(files.len() * 200);
        files.iter_mut().for_each(|(b, fi)| {
            list.push(if *b { '1' } else { '0' });
            list.push(',');
            #[cfg(target_os = "windows")]
            list.push_str(&fi.get_string().replace('\\', "/"));
            #[cfg(not(target_os = "windows"))]
            list.push_str(fi.get_string());
            list.push('\n');
        });
        list.pop();
        Self { list, version: 2 }
    }

    /// Get an iterator over all the files in the list with a flag
    pub fn iter(&'_ self) -> Box<dyn Iterator<Item = (bool, &str)> + '_> {
        match self.version {
            2 => Box::new(
                self.list
                    .split('\n')
                    .map(|s: &str| (s.starts_with('1'), &s[2..])),
            ),
            _ => Box::new(self.list.split('\n').map(|s| (true, s))),
        }
    }

    /// Get an iterator over all the files that are included
    pub fn iter_included(&'_ self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self.version {
            2 => Box::new(self.list.split('\n').filter_map(|s: &str| {
                if s.starts_with('1') {
                    Some(&s[2..])
                } else {
                    None
                }
            })),
            _ => Box::new(self.list.split('\n')),
        }
    }

    pub fn filename(&self) -> &'static str {
        match self.version {
            2 => "files_v2.csv",
            _ => "files.csv",
        }
    }
}
