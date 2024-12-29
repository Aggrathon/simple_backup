/// This module contains utility functions (such as getting backups and configs)
use std::cmp::PartialOrd;
use std::ffi::{OsStr, OsString};
use std::fs::ReadDir;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use number_prefix::NumberPrefix;

use crate::backup::{BackupError, BackupReader, BACKUP_FILE_EXTENSION, CONFIG_FILE_EXTENSION};
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

#[allow(unused)]
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

fn get_probable_time<P: AsRef<Path>>(path: P) -> Option<NaiveDateTime> {
    let path = path.as_ref();
    let s = path.file_name()?;
    if let Ok(ndt) = parse_backup_file_name(&s.to_string_lossy()) {
        return Some(ndt);
    }
    let br = BackupReader::read_config_only(path.to_path_buf()).ok()?;
    br.time
}

pub struct BackupIterator {
    constant: Option<std::io::Result<PathBuf>>,
    dir: Option<ReadDir>,
}

impl BackupIterator {
    /// Create an iterator over backups based on ONE specific backup
    pub fn file(path: PathBuf) -> Self {
        BackupIterator {
            constant: Some(path.metadata().map(|_| path)),
            dir: None,
        }
    }

    /// Create an iterator over backups based on timestamps
    pub fn dir<P: AsRef<Path>>(dir: P) -> Self {
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

    /// Construct a BackupIterator from a path.
    /// This involves parsing a config if necessary.
    /// Files are treated as BackupIterator::file and directories as BackupIterator::dir
    pub fn path(path: PathBuf) -> Result<Self, BackupError> {
        let iter = match ConfigPathType::parse(path)? {
            ConfigPathType::Dir(path) => BackupIterator::dir(path),
            ConfigPathType::Backup(path) => BackupIterator::file(path),
            ConfigPathType::Config(path) => BackupIterator::path(Config::read_yaml(path)?.output)?,
        };
        if let Some(Err(e)) = iter.constant {
            Err(BackupError::IOError(e))
        } else {
            Ok(iter)
        }
    }

    /// Get the latest backup based on the timestamp in the file name
    pub fn get_latest(&mut self) -> Option<PathBuf> {
        self.filter_map(|res| res.ok())
            .max_by_key(|p| get_probable_time(p))
    }

    /// Get the previous backup based on a file name
    pub fn get_previous(&mut self, path: &PathBuf) -> Option<PathBuf> {
        let time = get_probable_time(path);
        self.filter_map(|res| res.ok())
            .filter_map(|p| {
                let t2 = get_probable_time(&p);
                if t2 < time {
                    Some((p, t2))
                } else {
                    None
                }
            })
            .max_by_key(|(_, t)| *t)
            .map(|(p, _)| p)
    }

    /// Get a vec of backups in chronological order
    #[allow(unused)]
    pub fn get_all(&mut self) -> std::io::Result<Vec<PathBuf>> {
        let mut vec = self.collect::<std::io::Result<Vec<PathBuf>>>()?;
        vec.sort_by_key(|p| get_probable_time(p));
        Ok(vec)
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
                    let s = p.to_string_lossy();
                    if s.ends_with(BACKUP_FILE_EXTENSION) {
                        return Some(Ok(path));
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
            if s.ends_with(CONFIG_FILE_EXTENSION) {
                return Ok(Self::Config(path));
            } else if s.ends_with(BACKUP_FILE_EXTENSION) {
                return Ok(Self::Backup(path));
            }
        }
        Err(BackupError::InvalidPath(p.to_string_lossy().to_string()))
    }
}

/// Get a config based upon the path
pub fn get_config_from_path(path: PathBuf) -> Result<Config, BackupError> {
    match ConfigPathType::parse(path)? {
        ConfigPathType::Config(path) => Config::read_yaml(path).map_err(BackupError::FileError),
        ConfigPathType::Backup(path) => BackupReader::read_config_only(path),
        ConfigPathType::Dir(path) => match BackupIterator::dir(&path).get_latest() {
            None => Err(BackupError::NoBackup(path)),
            Some(path) => BackupReader::read_config_only(path),
        },
    }
}

/// Get a BackupReader based upon the path
pub fn get_backup_from_path(path: PathBuf) -> Result<BackupReader, BackupError> {
    match ConfigPathType::parse(path)? {
        ConfigPathType::Config(path) => Ok(BackupReader::from_config(Config::read_yaml(path)?)?),
        ConfigPathType::Backup(path) => Ok(BackupReader::new(path)),
        ConfigPathType::Dir(path) => match BackupIterator::dir(&path).get_latest() {
            None => Err(BackupError::NoBackup(path)),
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

#[cfg(feature = "dirs")]
pub fn default_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
}

#[cfg(feature = "dirs")]
#[allow(unused)]
pub fn home_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

#[cfg(not(feature = "dirs"))]
pub fn default_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(feature = "dirs")]
#[allow(unused)]
pub fn default_dir_opt() -> Option<PathBuf> {
    std::env::current_dir()
        .map(Some)
        .unwrap_or_else(|_| dirs::home_dir())
}

#[cfg(not(feature = "dirs"))]
#[allow(unused)]
pub fn default_dir_opt() -> Option<PathBuf> {
    std::env::current_dir().map(Some).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{
        get_backup_from_path, get_config_from_path, strip_absolute_from_path, BackupIterator,
    };
    use crate::backup::BackupError;
    use crate::Config;

    #[test]
    fn try_macros() {
        let try_some_ok: fn() -> Option<Result<i32, i32>> = || Some(Ok(try_some!(Ok(1))));
        assert_eq!(Some(Ok(1)), try_some_ok());
        let try_some_err: fn() -> Option<Result<i32, i32>> = || Some(Ok(try_some!(Err(1))));
        assert_eq!(Some(Err(1)), try_some_err());
    }

    #[test]
    fn backup_iterator() -> Result<(), BackupError> {
        let dir = tempdir()?;
        let dir2 = tempdir()?;
        let f2 = dir.path().join("backup_2020-02-20_20-20-20.tar.zst");
        let f3 = dir.path().join("backup_2020-04-24_21-20-20.tar.zst");
        let f4 = dir.path().join("backup_2020-04-24_22-20-20.tar.zst");
        let f5 = dir2.path().join("a.tar.zst");
        let f6 = dir2.path().join("b.tar.zst");
        File::create(&f2)?;
        File::create(&f3)?;
        File::create(&f4)?;
        File::create(&f5)?;
        let bis = BackupIterator::dir(dir.path()).get_all()?;
        assert_eq!(bis, vec![f2.clone(), f3.clone(), f4.clone()]);
        let mut bi = BackupIterator::dir(dir.path());
        assert_eq!(bi.get_latest().unwrap(), f4);
        let mut bi = BackupIterator::dir(dir.path());
        assert_eq!(bi.get_previous(&f4).unwrap(), f3);
        let mut bi = BackupIterator::file(f2.clone());
        assert_eq!(bi.next().unwrap()?, f2);
        assert!(bi.next().is_none());
        let mut bi = BackupIterator::file(f2.clone());
        assert_eq!(bi.get_latest().unwrap(), f2);
        std::thread::sleep(std::time::Duration::from_millis(10));
        File::create(&f6)?;
        let bis = BackupIterator::path(dir2.path().to_path_buf())?.get_all()?;
        assert_eq!(bis, vec![f5, f6.clone()]);
        let mut bi = BackupIterator::dir(dir2.path());
        assert_eq!(bi.get_latest().unwrap(), f6);
        Ok(())
    }

    #[test]
    fn from_path() -> std::io::Result<()> {
        let dir = tempdir()?;
        let f1 = dir.path().join("backup_2020-02-20_20-20-20.tar.zst");
        let f2 = dir.path().join("backup_2020-02-20_20-20-22.tar.zst");
        let f3 = dir.path().join("config.yml");
        File::create(&f1)?;
        File::create(&f2)?;
        let mut conf = Config::new();
        conf.output = PathBuf::from("test");
        conf.write_yaml(&f3, true)?;
        assert_eq!(get_config_from_path(f3).unwrap().output, conf.output);
        assert_eq!(
            get_backup_from_path(dir.path().to_path_buf()).unwrap().path,
            f2.into()
        );
        assert_eq!(get_backup_from_path(f1.clone()).unwrap().path, f1.into());
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
