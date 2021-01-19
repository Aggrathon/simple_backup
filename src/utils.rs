use std::{
    cmp::{max, min},
    fs::ReadDir,
    io::{Error, ErrorKind, Write},
    path::{Path, PathBuf},
};

use chrono::NaiveDateTime;
use regex::Regex;

pub mod parse_date {
    use chrono::NaiveDateTime;
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    const FORMAT: &'static str = "%Y-%m-%d %H:%M:%S";
    const FORMATS: [&'static str; 19] = [
        "%Y-%m-%d_%H-%M-%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
        "%y-%m-%d %H:%M:%S",
        "%y-%m-%d %H:%M",
        "%y-%m-%d",
        "%Y.%m.%d %H:%M:%S",
        "%Y.%m.%d %H:%M",
        "%Y.%m.%d",
        "%y.%m.%d %H:%M:%S",
        "%y.%m.%d %H:%M",
        "%y.%m.%d",
        "%Y%m%d%H%M%S",
        "%Y%m%d%H%M",
        "%Y%m%d",
        "%y%m%d%H%M%S",
        "%y%m%d%H%M",
        "%y%m%d",
    ];

    pub fn serialize<S>(date: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match date {
            None => serializer.serialize_str(""),
            Some(date) => serializer.serialize_str(&format!("{}", date.format(FORMAT))),
        }
    }

    pub fn deserialize<'a, D>(deserializer: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: Deserializer<'a>,
    {
        let date = &String::deserialize(deserializer)?;
        if date == "" {
            Ok(None)
        } else {
            NaiveDateTime::parse_from_str(&date, FORMAT)
                .map_err(Error::custom)
                .map(|v| Some(v))
        }
    }

    pub fn try_parse(input: &str) -> Result<Option<NaiveDateTime>, &str> {
        if input == "" {
            return Ok(None);
        }
        for f in FORMATS.iter() {
            if let Ok(t) = NaiveDateTime::parse_from_str(input, f) {
                return Ok(Some(t));
            }
        }
        Err("Unknown time format, try, e.g., `YYMMDD`")
    }
}

enum BackupIteratorPattern {
    None,
    Fullstamp(String),
    Endstamp(String),
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
        Self::new(
            dir,
            BackupIteratorPattern::Endstamp("_%Y-%m-%d_%H-%M-%S.tar.br".to_string()),
        )
    }

    pub fn with_name<P: AsRef<Path>, S: AsRef<str>>(dir: P, name: S) -> Self {
        Self::new(
            dir,
            BackupIteratorPattern::Fullstamp(format!("{}_%Y-%m-%d_%H-%M-%S.tar.br", name.as_ref())),
        )
    }

    #[allow(dead_code)]
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
}

impl Iterator for BackupIterator {
    type Item = std::io::Result<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.constant.is_some() {
            std::mem::replace(&mut self.constant, None)
        } else if let Some(dir) = &mut self.dir {
            for entry in dir {
                if entry.is_err() {
                    return Some(entry.map(|e| e.path()));
                }
                let entry = entry.unwrap();
                let filename = entry.file_name();
                let string = filename.to_string_lossy();
                match &self.pattern {
                    BackupIteratorPattern::Fullstamp(pattern) => {
                        if NaiveDateTime::parse_from_str(&string, &pattern).is_ok() {
                            return Some(Ok(entry.path()));
                        }
                    }
                    BackupIteratorPattern::Endstamp(pattern) => {
                        let start = string.len() - min(string.len(), pattern.len());
                        if NaiveDateTime::parse_from_str(&string[start..], &pattern).is_ok() {
                            return Some(Ok(entry.path()));
                        }
                    }
                    BackupIteratorPattern::Regex(regex) => {
                        if regex.is_match(&string) {
                            return Some(Ok(entry.path()));
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
