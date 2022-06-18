/// This module contains utility functions (such as getting backups and configs)
use std::cmp::{Ordering, PartialOrd};
use std::ffi::{OsStr, OsString};
use std::fs::ReadDir;
use std::path::{Path, PathBuf};

use number_prefix::NumberPrefix;

use crate::backup::{BackupError, BackupReader};
use crate::config::Config;
use crate::parse_date::parse_backup_file_name;

macro_rules! try_some {
    ($value:expr) => {
        match $value {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        }
    };
}

pub fn clamp<T>(value: T, min: T, max: T) -> T
where
    T: PartialOrd,
{
    if value > min {
        if value < max {
            value
        } else {
            max
        }
    } else {
        min
    }
}

pub fn format_size(size: u64) -> String {
    match NumberPrefix::binary(size as f64) {
        NumberPrefix::Standalone(number) => {
            format!("{:.2} KiB", number / 1024.0)
        }
        NumberPrefix::Prefixed(prefix, number) => {
            format!("{:.2} {}B", number, prefix)
        }
    }
}

const PATTERN_LENGTH: usize = "2020-20-20_20-20-20.tar.zst".len();

fn compare_backup_paths<P: AsRef<Path>>(p1: &P, p2: &P) -> Ordering {
    let f1 = match p1.as_ref().file_name() {
        None => return Ordering::Less,
        Some(f) => match f.to_str() {
            None => return Ordering::Less,
            Some(s) => s,
        },
    };
    let f2 = match p2.as_ref().file_name() {
        None => return Ordering::Greater,
        Some(f) => match f.to_str() {
            None => return Ordering::Greater,
            Some(s) => s,
        },
    };
    if f1.len() <= PATTERN_LENGTH || f2.len() <= PATTERN_LENGTH {
        return f1.cmp(f2);
    }
    f1[(f1.len() - PATTERN_LENGTH)..(f1.len() - 8)]
        .cmp(&f2[(f2.len() - PATTERN_LENGTH)..(f2.len() - 8)])
}

pub struct BackupIterator {
    constant: Option<std::io::Result<PathBuf>>,
    dir: Option<ReadDir>,
}

impl BackupIterator {
    /// Create an iterator over backups based on ONE specific backup
    pub fn exact(path: PathBuf) -> Self {
        BackupIterator {
            constant: Some(path.metadata().map(|_| path)),
            dir: None,
        }
    }

    /// Create an iterator over backups based on timestamps
    pub fn timestamp<P: AsRef<Path>>(dir: P) -> Self {
        match dir.as_ref().read_dir() {
            Err(e) => BackupIterator {
                constant: Some(Err(e)),
                dir: None,
            },
            Ok(d) => BackupIterator {
                constant: None,
                dir: Some(d),
            },
        }
    }

    /// Get the latest backup based on the timestamp in the file name
    pub fn get_latest(&mut self) -> Option<PathBuf> {
        self.filter_map(|res| res.ok()).max_by(compare_backup_paths)
    }

    /// Get the previous backup based on a file name
    pub fn get_previous(&mut self, path: &PathBuf) -> Option<PathBuf> {
        self.filter_map(|res| res.ok())
            .filter(|p| compare_backup_paths(path, p) == Ordering::Greater)
            .max_by(compare_backup_paths)
    }
}

impl Iterator for BackupIterator {
    type Item = std::io::Result<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.constant.is_some() {
            std::mem::take(&mut self.constant)
        } else if let Some(dir) = &mut self.dir {
            for entry in dir {
                let path = try_some!(entry.map(|e| e.path()));
                if !try_some!(path.metadata()).is_file() {
                    continue;
                }
                if let Some(p) = path.file_name() {
                    if let Some(s) = p.to_str() {
                        if parse_backup_file_name(s).is_ok() {
                            return Some(Ok(path));
                        }
                    }
                }
            }
            None
        } else {
            None
        }
    }
}

enum ConfigPathType<P: AsRef<Path>> {
    Dir(P),
    Backup(P),
    Config(P),
}

impl<P: AsRef<Path>> ConfigPathType<P> {
    /// Parse a path to get how the config should be extracted
    pub fn parse(path: P) -> Result<Self, BackupError> {
        let p = path.as_ref();
        let md = p.metadata().map_err(BackupError::FileError)?;
        if md.is_dir() {
            return Ok(Self::Dir(path));
        } else if md.is_file() {
            let s = p.to_string_lossy();
            if s.ends_with(".yml") {
                return Ok(Self::Config(path));
            } else if s.ends_with(".tar.zst") {
                return Ok(Self::Backup(path));
            }
        }
        Err(BackupError::InvalidPath(p.to_string_lossy().to_string()))
    }
}

/// Get a config based upon the path
pub fn get_config_from_path<P: AsRef<Path>>(path: P) -> Result<Config, BackupError> {
    match ConfigPathType::parse(path)? {
        ConfigPathType::Config(path) => Config::read_yaml(path).map_err(BackupError::FileError),
        ConfigPathType::Backup(path) => BackupReader::read_config_only(path),
        ConfigPathType::Dir(path) => match BackupIterator::timestamp(&path).get_latest() {
            None => Err(BackupError::NoBackup(path.as_ref().to_path_buf())),
            Some(path) => BackupReader::read_config_only(path),
        },
    }
}

/// Get a BackupReader based upon the path
pub fn get_backup_from_path<P: AsRef<Path>>(path: P) -> Result<BackupReader, BackupError> {
    match ConfigPathType::parse(path)? {
        ConfigPathType::Config(path) => Ok(BackupReader::from_config(Config::read_yaml(path)?)?),
        ConfigPathType::Backup(path) => Ok(BackupReader::new(path)),
        ConfigPathType::Dir(path) => match BackupIterator::timestamp(&path).get_latest() {
            None => Err(BackupError::NoBackup(path.as_ref().to_path_buf())),
            Some(path) => Ok(BackupReader::new(path)),
        },
    }
}

pub fn strip_absolute_from_path(path: &str) -> String {
    let path = path.trim_start_matches('.');
    let path = path.trim_start_matches('/');
    #[cfg(target_os = "windows")]
    {
        let path = path.trim_start_matches('\\');
        path.replace(':', "")
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.into()
    }
}

pub fn extend_pathbuf<S: AsRef<OsStr>>(mut path: PathBuf, extension: S) -> PathBuf {
    let mut p: OsString = path.into();
    p.push(extension);
    path = p.into();
    path
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{
        get_backup_from_path, get_config_from_path, strip_absolute_from_path, BackupIterator,
    };
    use crate::Config;

    #[test]
    fn try_macros() {
        let try_some_ok: fn() -> Option<Result<i32, i32>> = || Some(Ok(try_some!(Ok(1))));
        assert_eq!(Some(Ok(1)), try_some_ok());
        let try_some_err: fn() -> Option<Result<i32, i32>> = || Some(Ok(try_some!(Err(1))));
        assert_eq!(Some(Err(1)), try_some_err());
    }

    #[test]
    fn backup_iterator() -> std::io::Result<()> {
        let dir = tempdir()?;
        let f1 = dir.path().join("asd.tar.zst");
        let f2 = dir.path().join("backup_2020-02-20_20-20-20.tar.zst");
        let f3 = dir.path().join("backup_2020-04-24_21-20-20.tar.zst");
        let f4 = dir.path().join("backup_2020-04-24_22-20-20.tar.zst");
        File::create(&f1)?;
        File::create(&f2)?;
        File::create(&f3)?;
        File::create(&f4)?;
        let bi = BackupIterator::timestamp(dir.path());
        let bis = bi.collect::<std::io::Result<Vec<PathBuf>>>()?;
        assert_eq!(bis.len(), 3);
        assert!(bis.contains(&f2));
        assert!(bis.contains(&f3));
        assert!(bis.contains(&f4));
        let mut bi = BackupIterator::timestamp(dir.path());
        assert_eq!(bi.get_latest().unwrap(), f4);
        let mut bi = BackupIterator::timestamp(dir.path());
        assert_eq!(bi.get_previous(&f4.to_path_buf()).unwrap(), f3);
        let mut bi = BackupIterator::exact(f1.clone());
        assert_eq!(bi.next().unwrap()?, f1);
        assert!(bi.next().is_none());
        let mut bi = BackupIterator::exact(f1.clone());
        assert_eq!(bi.get_latest().unwrap(), f1);
        Ok(())
    }

    #[test]
    fn from_path() -> std::io::Result<()> {
        let dir = tempdir()?;
        let f1 = dir.path().join("asd.tar.zst");
        let f2 = dir.path().join("backup_2020-02-20_20-20-20.tar.zst");
        let f3 = dir.path().join("config.yml");
        File::create(&f1)?;
        File::create(&f2)?;
        let mut conf = Config::new();
        conf.output = PathBuf::from("test");
        conf.write_yaml(&f3)?;
        assert_eq!(get_config_from_path(f3).unwrap().output, conf.output);
        assert_eq!(get_backup_from_path(dir.path()).unwrap().path, f2);
        assert_eq!(get_backup_from_path(f1.as_path()).unwrap().path, f1);
        Ok(())
    }

    #[test]
    fn strip_abs() {
        assert_eq!("server/path", strip_absolute_from_path("/server/path"));
        assert_eq!("path", strip_absolute_from_path("../path"));
        #[cfg(target_os = "windows")]
        {
            assert_eq!("server\\path", strip_absolute_from_path("\\\\server\\path"));
            assert_eq!("E\\path", strip_absolute_from_path("E:\\path"));
        }
    }
}
