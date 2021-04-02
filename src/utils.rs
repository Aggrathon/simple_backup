use std::{
    ffi::OsStr,
    fs::ReadDir,
    path::{Path, PathBuf},
};

use crate::{
    backup::{BackupError, BackupReader},
    config::Config,
    parse_date::parse_backup_file_name,
};

macro_rules! try_some {
    ($value:expr) => {
        match $value {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        }
    };
}

macro_rules! try_option {
    ($value:expr) => {
        match $value {
            Some(v) => v,
            None => return None,
        }
    };
}

const PATTERN_LENGTH: usize = "_2020-20-20_20-20-20.tar.zst".len();

fn get_pattern(name: &OsStr) -> String {
    let f = name.to_string_lossy();
    if f.len() >= PATTERN_LENGTH {
        f[(f.len() - PATTERN_LENGTH)..].to_string()
    } else {
        f.to_string()
    }
}

enum BackupIteratorPattern {
    None,
    Timestamp,
}
pub struct BackupIterator {
    constant: Option<std::io::Result<PathBuf>>,
    dir: Option<ReadDir>,
    pattern: BackupIteratorPattern,
}

impl BackupIterator {
    pub fn timestamp<P: AsRef<Path>>(dir: P) -> Self {
        Self::new(dir, BackupIteratorPattern::Timestamp)
    }

    pub fn exact(path: PathBuf) -> Self {
        BackupIterator {
            constant: Some(path.metadata().map(|_| path)),
            dir: None,
            pattern: BackupIteratorPattern::None,
        }
    }

    fn new<P: AsRef<Path>>(dir: P, pattern: BackupIteratorPattern) -> Self {
        match dir.as_ref().read_dir() {
            Err(e) => BackupIterator {
                constant: Some(Err(e)),
                dir: None,
                pattern: BackupIteratorPattern::None,
            },
            Ok(d) => BackupIterator {
                constant: None,
                dir: Some(d),
                pattern,
            },
        }
    }

    pub fn get_latest(&mut self) -> Option<PathBuf> {
        // Select latest based on timestamps in the filename
        self.filter_map(|res| res.ok())
            .filter_map(|p| {
                let s = get_pattern(try_option!(&p.file_name()));
                Some((p, s))
            })
            .max_by(|(_, f1), (_, f2)| f1.cmp(&f2))
            .map(|(p, _)| p)
    }

    pub fn get_previous<P: AsRef<Path>>(&mut self, path: P) -> Option<PathBuf> {
        let limit = get_pattern(try_option!(path.as_ref().file_name()));
        self.filter_map(|res| res.ok())
            .filter_map(|p| {
                let s = get_pattern(try_option!(&p.file_name()));
                if s >= limit {
                    return None;
                }
                Some((p, s))
            })
            .max_by(|(_, f1), (_, f2)| f1.cmp(&f2))
            .map(|(p, _)| p)
    }
}

impl Iterator for BackupIterator {
    type Item = std::io::Result<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.constant.is_some() {
            std::mem::replace(&mut self.constant, None)
        } else if let Some(dir) = &mut self.dir {
            for entry in dir {
                let path = try_some!(entry.map(|e| e.path()));
                if !try_some!(path.metadata()).is_file() {
                    continue;
                }
                let string = path.file_name().unwrap().to_string_lossy();
                match &self.pattern {
                    BackupIteratorPattern::Timestamp => {
                        if parse_backup_file_name(&string).is_ok() {
                            return Some(Ok(path));
                        }
                    }
                    BackupIteratorPattern::None => {}
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
    pub fn parse<S: AsRef<str>>(path: P, string: S) -> std::io::Result<Self> {
        let p = path.as_ref();
        let md = p.metadata()?;
        if md.is_dir() {
            return Ok(Self::Dir(path));
        } else if md.is_file() {
            if string.as_ref().ends_with(".yml") {
                return Ok(Self::Config(path));
            } else if string.as_ref().ends_with(".tar.zst") {
                return Ok(Self::Backup(path));
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "The path must be either a config (.yml), a backup (.tar.zst), or a directory containing backups",
        ))
    }
}

pub fn get_config_from_path<S: AsRef<str>>(path: S) -> Result<Config, Box<dyn std::error::Error>> {
    match ConfigPathType::parse(Path::new(path.as_ref()), &path)? {
        ConfigPathType::Config(path) => Ok(Config::read_yaml(path)?),
        ConfigPathType::Backup(path) => BackupReader::read_config_only(path),
        ConfigPathType::Dir(path) => match BackupIterator::timestamp(&path).get_latest() {
            None => Err(Box::new(BackupError::NoBackup(path.to_path_buf()))),
            Some(path) => BackupReader::read_config_only(path),
        },
    }
}

pub fn get_backup_from_path<'a, S: AsRef<str>>(
    path: S,
) -> Result<BackupReader<'a>, Box<dyn std::error::Error>> {
    match ConfigPathType::parse(Path::new(path.as_ref()), &path)? {
        ConfigPathType::Config(path) => Ok(BackupReader::from_config(Config::read_yaml(path)?)?),
        ConfigPathType::Backup(path) => Ok(BackupReader::read(path)?),
        ConfigPathType::Dir(path) => match BackupIterator::timestamp(&path).get_latest() {
            None => Err(Box::new(BackupError::NoBackup(path.to_path_buf()))),
            Some(path) => Ok(BackupReader::read(path)?),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tempfile::tempdir;

    use super::{get_backup_from_path, get_config_from_path, BackupIterator};
    use crate::Config;

    #[test]
    fn try_macros() {
        let try_some_ok: fn() -> Option<Result<i32, i32>> = || Some(Ok(try_some!(Ok(1))));
        assert_eq!(Some(Ok(1)), try_some_ok());
        let try_some_err: fn() -> Option<Result<i32, i32>> = || Some(Ok(try_some!(Err(1))));
        assert_eq!(Some(Err(1)), try_some_err());

        let option_some: fn() -> Option<i32> = || Some(try_option!(Some(1)));
        assert_eq!(Some(1), option_some());
        let option_none: fn() -> Option<i32> = || Some(try_option!(None));
        assert_eq!(None, option_none());
    }

    #[test]
    fn backup_iterator() -> std::io::Result<()> {
        let dir = tempdir()?;
        let f1 = dir.path().join("asd.tar.zst");
        let f1b = f1.clone();
        let f2 = dir.path().join("backup_2020-02-20_20-20-20.tar.zst");
        let f3 = dir.path().join("backup_2020-04-24_21-20-20.tar.zst");
        let f4 = dir.path().join("backup_2020-04-24_22-20-20.tar.zst");
        File::create(&f1)?;
        File::create(&f2)?;
        File::create(&f3)?;
        File::create(&f4)?;
        let mut bi = BackupIterator::timestamp(dir.path());
        assert_eq!(bi.next().unwrap().unwrap(), f2);
        assert_eq!(bi.next().unwrap().unwrap(), f3);
        assert_eq!(bi.next().unwrap().unwrap(), f4);
        assert!(bi.next().is_none());
        let mut bi = BackupIterator::timestamp(dir.path());
        assert_eq!(bi.get_latest().unwrap(), f4);
        let mut bi = BackupIterator::timestamp(dir.path());
        assert_eq!(bi.get_previous(f4).unwrap(), f3);
        let mut bi = BackupIterator::exact(f1);
        assert_eq!(bi.next().unwrap().unwrap(), f1b);
        assert!(bi.next().is_none());
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
        conf.output = "test".to_string();
        conf.write_yaml(&f3)?;
        assert_eq!(
            get_config_from_path(f3.to_string_lossy()).unwrap().output,
            conf.output
        );
        assert_eq!(
            get_backup_from_path(dir.path().to_string_lossy())
                .unwrap()
                .path,
            f2
        );
        assert_eq!(get_backup_from_path(f1.to_string_lossy()).unwrap().path, f1);
        Ok(())
    }
}
