use std::{
    cmp::max,
    error::Error,
    fmt::{Display, Formatter},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use chrono::NaiveDateTime;

use crate::{compression::CompressionDecoder, config::Config, files::FileInfo, parse_date};

#[derive(Debug)]
pub enum BackupError {
    NoConfig(PathBuf),
    NoList(PathBuf),
    NoBackup(PathBuf),
}

impl Display for BackupError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            BackupError::NoConfig(path) => {
                write!(
                    f,
                    "Could not find the config file in the backup: {}",
                    path.to_string_lossy()
                )
            }
            BackupError::NoList(path) => {
                write!(
                    f,
                    "Could not find the file list in the backup: {}",
                    path.to_string_lossy()
                )
            }
            BackupError::NoBackup(path) => {
                write!(f, "Could not the backup: {}", path.to_string_lossy())
            }
        }
    }
}

impl std::error::Error for BackupError {}

/// Check the config (arguments) or previous backups for a time limit in case of an incremental backup
pub fn get_previous_time(config: &Config) -> Option<NaiveDateTime> {
    if !config.incremental {
        None
    } else if let Some(t) = config.time {
        Some(t)
    } else {
        let mut time = None;
        for path in config.get_previous() {
            if let Err(e) = path {
                eprintln!("Could not find backup: {}", e);
                continue;
            }
            let path = path.unwrap();
            let bp = BackupReader::read(&path);
            if let Err(e) = bp {
                eprintln!("Could not open backup: {}", e);
                continue;
            }
            match bp.unwrap().read_config() {
                Err(e) => {
                    eprintln!(
                        "Could not get time from '{}': {}",
                        path.to_string_lossy(),
                        e
                    );
                }
                Ok(conf) => {
                    if let Some(t1) = time {
                        if let Some(t2) = conf.time {
                            time = Some(max(t1, t2))
                        }
                    } else if let Some(t) = conf.time {
                        time = Some(t)
                    }
                }
            }
        }
        time
    }
}

// pub fn find_backups<P: AsRef<Path>, S: AsRef<str>>(dir: P, name: Option<S>) -> Option<PathBuf> {
//     let dir = dir.as_ref();
//     if dir.is_dir() {
//         dir.read_dir()?.filter_map(|de| {
//             de?.file_name().as_os_str().e
//         })
//     } else if dir.is_file() {
//         if name.is_none() {
//             Some(dir.to_path_buf())
//         } else {
//             None
//         }
//     } else {
//         None
//     }
// }

// pub fn get_path_config<S: AsRef<str>>(path: S) -> Result<Config, Box<dyn Error>> {
//     let path = path.as_ref();
//     let mut config = if path.ends_with(".yml") {
//         Config::read_yaml(path)?
//     } else if path.ends_with(".tar.br") {
//         Backup::read(&path)?.get_config()?
//     } else {
//         let mut config: Option<Config> = None;
//         let mut selected = PathBuf::new();
//         for path in BackupIterator::with_ending(path) {
//             if let Err(e) = &path {
//                 eprintln!("Could not find backups: {}", e);
//             }
//             let path = path.unwrap();
//             let new = get_backup_config(&path);
//             if let Err(e) = &new {
//                 eprintln!("Could not get config from backup: {}", e);
//             }
//             let new = new.unwrap();
//             if let Some(old) = config {
//                 if old.time < new.time {
//                     config = Some(new);
//                     selected = path;
//                 } else {
//                     config = Some(old);
//                 }
//             } else {
//                 selected = path;
//                 config = Some(new);
//             }
//         }
//         if config.is_none() {
//             panic!("Could not find a config from an earlier backup");
//         }
//         println!("Using the config from '{}'", selected.to_string_lossy());
//         config.unwrap()
//     };
//     Ok(config)
// }

pub struct BackupReader {
    decoder: CompressionDecoder,
    pub path: PathBuf,
    pub config: Option<Config>,
    pub list: Option<String>,
}

impl BackupReader {
    pub fn read<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        Ok(BackupReader {
            path: path.as_ref().to_path_buf(),
            decoder: CompressionDecoder::read(path)?,
            list: None,
            config: None,
        })
    }

    pub fn from_config(config: Config) -> std::io::Result<Self> {
        let prev = config
            .get_previous()
            .get_latest(true)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Backup not found"))?;
        let decoder = CompressionDecoder::read(prev.as_path())?;
        Ok(BackupReader {
            path: prev,
            decoder,
            config: Some(config),
            list: None,
        })
    }

    pub fn read_config_only<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn Error>> {
        let mut br = BackupReader::read(path)?;
        br.read_config()?;
        Ok(br.config.unwrap())
    }

    pub fn read_config(&mut self) -> Result<&Config, Box<dyn Error>> {
        let entry = self.decoder.entries()?.next();
        if entry.is_none() {
            return Err(Box::new(BackupError::NoConfig(self.path.clone())));
        }
        let mut entry = entry.unwrap()?;
        if entry.0.as_os_str() != "config.yml" {
            return Err(Box::new(BackupError::NoConfig(self.path.clone())));
        }
        let mut s = String::new();
        entry.1.read_to_string(&mut s)?;
        let mut conf: Config = Config::from_yaml(&s)?;
        conf.origin = Some(self.path.to_string_lossy().to_string());
        self.config = Some(conf);
        Ok(self.config.as_ref().unwrap())
    }

    pub fn read_list(&mut self) -> Result<&String, Box<dyn Error>> {
        let entry = self.decoder.entries()?.skip(1).next();
        if entry.is_none() {
            return Err(Box::new(BackupError::NoList(self.path.clone())));
        }
        let mut entry = entry.unwrap()?;
        if entry.0.as_os_str() != "files.csv" {
            return Err(Box::new(BackupError::NoList(self.path.clone())));
        }
        let mut s = String::new();
        entry.1.read_to_string(&mut s)?;
        self.list = Some(s);
        Ok(self.list.as_ref().unwrap())
    }

    pub fn files(
        &mut self,
    ) -> std::io::Result<
        impl Iterator<Item = std::io::Result<(PathBuf, tar::Entry<brotli::Decompressor<File>>)>>,
    > {
        Ok(self.decoder.entries()?.skip(2))
    }

    pub fn read_all(
        &mut self,
    ) -> Result<
        (
            &Config,
            &String,
            impl Iterator<Item = std::io::Result<(PathBuf, tar::Entry<brotli::Decompressor<File>>)>>,
        ),
        Box<dyn Error>,
    > {
        let mut entries = self.decoder.entries()?;
        // Read Config
        let entry = entries.next();
        if entry.is_none() {
            return Err(Box::new(BackupError::NoConfig(self.path.clone())));
        }
        let mut entry = entry.unwrap()?;
        if entry.0.as_os_str() != "config.yml" {
            return Err(Box::new(BackupError::NoConfig(self.path.clone())));
        }
        let mut s = String::new();
        entry.1.read_to_string(&mut s)?;
        let mut conf: Config = Config::from_yaml(&s)?;
        conf.origin = Some(self.path.to_string_lossy().to_string());
        self.config = Some(conf);
        // Read File List
        let entry = entries.next();
        if entry.is_none() {
            return Err(Box::new(BackupError::NoList(self.path.clone())));
        }
        let mut entry = entry.unwrap()?;
        if entry.0.as_os_str() != "files.csv" {
            return Err(Box::new(BackupError::NoList(self.path.clone())));
        }
        s.truncate(0);
        entry.1.read_to_string(&mut s)?;
        self.list = Some(s);
        // Rest
        Ok((
            self.config.as_ref().unwrap(),
            self.list.as_ref().unwrap(),
            entries,
        ))
    }
}

pub fn parse_file_list(
    list: &str,
) -> std::iter::Map<std::str::Lines<'_>, fn(&str) -> Result<FileInfo, &str>> {
    list.lines().map(|l| {
        let mut split = l.splitn(2, ',');
        let time = split.next().ok_or("File info is missing")?;
        let string = split.next().ok_or("Could not split at ','")?;
        Ok(FileInfo::new_str(string, parse_date::try_parse(time)?))
    })
}
