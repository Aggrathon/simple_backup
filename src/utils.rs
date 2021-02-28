use std::{
    cmp::{max, min},
    fs::ReadDir,
    io::{Error, ErrorKind, Write},
    path::{Path, PathBuf},
};

use chrono::NaiveDateTime;
use regex::Regex;

use crate::{
    backup::{BackupError, BackupReader},
    config::Config,
};

const PATTERN_LENGTH: usize = "_2020-20-20_20-20-20.tar.br".len();

macro_rules! try_some {
    ($value:expr) => {
        match $value {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        }
    };
}

macro_rules! try_some_box {
    ($value:expr) => {
        match $value {
            Ok(v) => v,
            Err(e) => return Some(Err(Box::new(e))),
        }
    };
}

enum ConfigPathType<P: AsRef<Path>> {
    Dir(P),
    Backup(P),
    Config(P),
}

impl<P: AsRef<Path>> ConfigPathType<P> {
    pub fn parse(path: P) -> std::io::Result<Self> {
        let p = path.as_ref();
        let md = p.metadata()?;
        if md.is_dir() {
            return Ok(Self::Dir(path));
        } else if md.is_file() {
            if p.ends_with(".yml") {
                return Ok(Self::Config(path));
            } else if p.ends_with(".tar.br") {
                return Ok(Self::Backup(path));
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "The path must be either a config (.yml), a backup (.tar.br), or a directory containing backups",
        ))
    }
}

enum BackupIteratorPattern {
    None,
    Fullstamp(String),
    Endstamp,
    Regex(Regex),
}

pub struct BackupIterator {
    constant: Option<std::io::Result<PathBuf>>,
    dir: Option<ReadDir>,
    pattern: BackupIteratorPattern,
}

impl BackupIterator {
    #[allow(dead_code)]
    pub fn with_timestamp<P: AsRef<Path>>(dir: P) -> Self {
        Self::new(dir, BackupIteratorPattern::Endstamp)
    }

    pub fn with_name<P: AsRef<Path>>(dir: P, name: String) -> Self {
        Self::new(dir, BackupIteratorPattern::Fullstamp(name))
    }

    pub fn with_ending<P: AsRef<Path>>(dir: P) -> Self {
        match Regex::new(".*.tar.br") {
            Err(e) => BackupIterator {
                constant: Some(Err(Error::new(ErrorKind::Other, e))),
                dir: None,
                pattern: BackupIteratorPattern::None,
            },
            Ok(r) => Self::new(dir, BackupIteratorPattern::Regex(r)),
        }
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

    pub fn get_latest(&mut self, pattern: bool) -> Option<PathBuf> {
        if pattern {
            // Select latest based on timestamps in the filename
            self.filter_map(|res| res.ok())
                .filter(|p| p.file_name().is_some())
                .map(|p| {
                    let f = p.file_name().unwrap().to_string_lossy();
                    let s = if f.len() >= PATTERN_LENGTH {
                        f[(f.len() - PATTERN_LENGTH)..].to_string()
                    } else {
                        f.to_string()
                    };
                    (p, s)
                })
                .max_by(|(_, f1), (_, f2)| f1.cmp(&f2))
                .map(|(p, _)| p)
        } else {
            // Select latest based on the modified metadata
            self.filter_map(|res| {
                res.ok().and_then(|p| {
                    p.metadata()
                        .ok()
                        .and_then(|md| md.modified().ok().and_then(|t| Some((p, t))))
                })
            })
            .max_by(|(_, t1), (_, t2)| t1.cmp(t2))
            .and_then(|(p, _)| Some(p))
        }
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
                    BackupIteratorPattern::Fullstamp(name) => {
                        if string.starts_with(name)
                            && NaiveDateTime::parse_from_str(
                                &string[name.len()..],
                                "_%Y-%m-%d_%H-%M-%S.tar.br",
                            )
                            .is_ok()
                        {
                            return Some(Ok(path));
                        }
                    }
                    BackupIteratorPattern::Endstamp => {
                        let start = string.len() - min(string.len(), PATTERN_LENGTH);
                        if NaiveDateTime::parse_from_str(
                            &string[start..],
                            "_%Y-%m-%d_%H-%M-%S.tar.br",
                        )
                        .is_ok()
                        {
                            return Some(Ok(path));
                        }
                    }
                    BackupIteratorPattern::Regex(regex) => {
                        if regex.is_match(&string) {
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

pub fn get_config_from_path<S: AsRef<str>>(path: S) -> Result<Config, Box<dyn std::error::Error>> {
    match ConfigPathType::parse(Path::new(path.as_ref()))? {
        ConfigPathType::Dir(path) => Ok(Config::read_yaml(path)?),
        ConfigPathType::Backup(path) => BackupReader::read_config_only(path),
        ConfigPathType::Config(path) => {
            match BackupIterator::with_timestamp(&path).get_latest(true) {
                None => Err(Box::new(BackupError::NoBackup(path.to_path_buf()))),
                Some(path) => BackupReader::read_config_only(path),
            }
        }
    }
}

pub fn get_backup_from_path<S: AsRef<str>>(
    path: S,
) -> Result<BackupReader, Box<dyn std::error::Error>> {
    match ConfigPathType::parse(Path::new(path.as_ref()))? {
        ConfigPathType::Dir(path) => Ok(BackupReader::from_config(Config::read_yaml(path)?)?),
        ConfigPathType::Backup(path) => Ok(BackupReader::read(path)?),
        ConfigPathType::Config(path) => {
            match BackupIterator::with_timestamp(&path).get_latest(true) {
                None => Err(Box::new(BackupError::NoBackup(path.to_path_buf()))),
                Some(path) => Ok(BackupReader::read(path)?),
            }
        }
    }
}

pub struct ProgressBar {
    max: usize,
    current: usize,
    steps: usize,
    progress: usize,
}

impl ProgressBar {
    pub fn start(size: usize, steps: usize, title: &str) -> Self {
        let length = title.chars().count();
        let steps = max(length + 4, steps);
        let pad = steps - length;
        for _ in 0..(pad / 2) {
            print!("_");
        }
        print!("{}", title);
        for _ in 0..((pad - 1) / 2 + 1) {
            print!("_");
        }
        print!("\n#");
        std::io::stdout().flush().unwrap();
        Self {
            max: size,
            current: 0,
            steps: steps,
            progress: 1,
        }
    }

    pub fn progress(&mut self) {
        if self.current < self.max {
            self.current += 1;
            let blocks = self.current * self.steps / self.max;
            if blocks > self.progress {
                while blocks > self.progress {
                    print!("#");
                    self.progress += 1;
                }
                if self.current == self.max {
                    println!("");
                } else {
                    std::io::stdout().flush().unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn progress_bar() {
        for n in [333, 500, 100].iter() {
            for s in [20, 27, 63].iter() {
                let mut bar = super::ProgressBar::start(*n, *s, "Backing up files");
                let mut count = 1;
                for _ in 0..*n {
                    bar.progress();
                    if bar.current < bar.max && bar.current * bar.steps % bar.max < bar.steps {
                        count += 1
                    }
                }
                assert_eq!(*s, count);
            }
        }
    }
}
