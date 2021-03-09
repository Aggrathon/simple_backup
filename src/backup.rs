use std::{
    error::Error,
    fmt::{Display, Formatter},
    fs::{create_dir_all, File},
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::NaiveDateTime;

use crate::{
    compression::{CompressionDecoder, CompressionEncoder},
    config::Config,
    files::{FileCrawler, FileInfo},
    parse_date::{self, system_to_naive},
};

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

pub struct BackupWriter {
    pub path: PathBuf,
    pub config: Config,
    pub prev_time: Option<NaiveDateTime>,
    pub list: Option<Vec<FileInfo>>,
    time: NaiveDateTime,
}

impl BackupWriter {
    pub fn new(config: Config) -> (Self, Option<Box<dyn Error>>) {
        let (prev_time, error) = if config.incremental {
            match config.time {
                Some(t) => (Some(t), None),
                None => match config.get_backups().get_latest() {
                    Some(path) => match BackupReader::read_config_only(path) {
                        Ok(c) => (c.time, None),
                        Err(e) => (None, Some(e)),
                    },
                    None => (None, None),
                },
            }
        } else {
            (None, None)
        };
        let path = config.get_output();
        (
            Self {
                config,
                path,
                prev_time,
                list: None,
                time: system_to_naive(SystemTime::now()),
            },
            error,
        )
    }

    pub fn get_files<F: FnMut(Result<&mut FileInfo, Box<dyn std::error::Error>>)>(
        &mut self,
        all: bool,
        callback: Option<F>,
    ) -> Result<&mut Vec<FileInfo>, Box<dyn std::error::Error>> {
        if self.list.is_some() {
            if callback.is_some() {
                let mut callback = callback.unwrap();
                if all || self.prev_time.is_none() {
                    self.list.as_mut().unwrap().iter_mut().for_each(|fi| {
                        callback(Ok(fi));
                    });
                } else {
                    let time = self.prev_time.unwrap();
                    self.list.as_mut().unwrap().iter_mut().for_each(|fi| {
                        if fi.time.unwrap() >= time {
                            callback(Ok(fi));
                        }
                    });
                }
            }
        } else {
            let fc = FileCrawler::new(
                &self.config.include,
                &self.config.exclude,
                &self.config.regex,
                self.config.local,
            )?;
            if callback.is_none() {
                self.list = Some(
                    fc.into_iter()
                        .filter_map(|fi| match fi {
                            Ok(fi) => Some(fi),
                            Err(_) => None,
                        })
                        .collect(),
                );
            } else if all || self.prev_time.is_none() {
                let mut callback = callback.unwrap();
                self.list = Some(
                    fc.into_iter()
                        .filter_map(|fi| match fi {
                            Ok(mut fi) => {
                                callback(Ok(&mut fi));
                                Some(fi)
                            }
                            Err(e) => {
                                callback(Err(e));
                                None
                            }
                        })
                        .collect(),
                );
            } else {
                let time = self.prev_time.unwrap();
                let mut callback = callback.unwrap();
                self.list = Some(
                    fc.into_iter()
                        .filter_map(|fi| match fi {
                            Ok(mut fi) => {
                                if fi.time.unwrap() >= time {
                                    callback(Ok(&mut fi));
                                }
                                Some(fi)
                            }
                            Err(e) => {
                                callback(Err(e));
                                None
                            }
                        })
                        .collect(),
                );
            }
        }
        Ok(self.list.as_mut().unwrap())
    }

    pub fn write(
        &mut self,
        mut callback: impl FnMut(&mut FileInfo, Result<(), std::io::Error>),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut encoder = CompressionEncoder::create(&self.path, self.config.quality)?;

        self.config.time = Some(self.time);
        let prev_time = self.prev_time.clone();
        encoder.append_data("config.yml", self.config.to_yaml()?)?;

        let list = self.get_files(
            true,
            None::<fn(Result<&mut FileInfo, Box<dyn std::error::Error>>)>,
        )?;
        let mut list_string = String::new();
        list_string.reserve(list.len() * 200);
        list.iter_mut().for_each(|fi| {
            #[cfg(target_os = "windows")]
            list_string.push_str(&fi.get_string().replace('\\', "/"));
            #[cfg(not(target_os = "windows"))]
            list_string.push_str(fi.get_string());
            list_string.push('\n');
        });
        list_string.pop();
        encoder.append_data("files.csv", list_string)?;

        match prev_time {
            Some(prev_time) => {
                for fi in list.iter_mut() {
                    match fi.get_path().metadata() {
                        Ok(md) => match md.modified() {
                            Ok(time) => {
                                if parse_date::system_to_naive(time) >= prev_time {
                                    let res = encoder.append_file(fi.get_path());
                                    callback(fi, res);
                                }
                            }
                            Err(e) => callback(fi, Err(e)),
                        },
                        Err(e) => callback(fi, Err(e)),
                    }
                }
            }
            None => {
                for fi in list.iter_mut() {
                    let res = encoder.append_file(fi.get_path());
                    callback(fi, res);
                }
            }
        }
        encoder.close()?;

        Ok(())
    }
}

pub struct BackupReader {
    decoder: CompressionDecoder,
    used: bool,
    pub path: PathBuf,
    pub config: Option<Config>,
    pub list: Option<String>,
}

impl BackupReader {
    pub fn read<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        Ok(BackupReader {
            path: path.as_ref().to_path_buf(),
            decoder: CompressionDecoder::read(path)?,
            used: false,
            list: None,
            config: None,
        })
    }

    pub fn from_config(config: Config) -> std::io::Result<Self> {
        let prev = config
            .get_backups()
            .get_latest()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Backup not found"))?;
        let decoder = CompressionDecoder::read(prev.as_path())?;
        Ok(BackupReader {
            path: prev,
            decoder,
            used: false,
            config: Some(config),
            list: None,
        })
    }

    fn use_decoder(&mut self) -> std::io::Result<()> {
        if self.used {
            self.decoder = CompressionDecoder::read(&self.path)?;
        } else {
            self.used = true;
        }
        Ok(())
    }

    pub fn read_config_only<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn Error>> {
        let mut br = BackupReader::read(path)?;
        br.read_config()?;
        Ok(br.config.unwrap())
    }

    pub fn read_config(&mut self) -> Result<&Config, Box<dyn Error>> {
        self.use_decoder()?;
        let entry = self.decoder.entries()?.next();
        if entry.is_none() {
            return Err(Box::new(BackupError::NoConfig(self.path.clone())));
        }
        let mut entry = entry.unwrap()?;
        if entry.0.get_string() != "config.yml" {
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
        self.use_decoder()?;
        let mut entries = self.decoder.entries()?.skip(1);
        let entry = entries.next();
        if entry.is_none() {
            return Err(Box::new(BackupError::NoList(self.path.clone())));
        }
        let mut entry = entry.unwrap()?;
        if entry.0.get_string() != "files.csv" {
            return Err(Box::new(BackupError::NoList(self.path.clone())));
        }
        let mut s = String::new();
        entry.1.read_to_string(&mut s)?;
        self.list = Some(s);
        Ok(self.list.as_ref().unwrap())
    }

    pub fn extract_list(&mut self) -> Result<String, Box<dyn Error>> {
        if self.list.is_none() {
            self.read_list()?;
        }
        Ok(std::mem::replace(&mut self.list, None).unwrap())
    }

    pub fn files(
        &mut self,
    ) -> std::io::Result<
        impl Iterator<Item = std::io::Result<(FileInfo, tar::Entry<brotli::Decompressor<File>>)>>,
    > {
        self.use_decoder()?;
        Ok(self.decoder.entries()?.skip(2))
    }

    #[allow(dead_code)]
    pub fn read_all(
        &mut self,
    ) -> Result<
        (
            &Config,
            &String,
            impl Iterator<Item = std::io::Result<(FileInfo, tar::Entry<brotli::Decompressor<File>>)>>,
        ),
        Box<dyn Error>,
    > {
        self.use_decoder()?;
        let mut entries = self.decoder.entries()?;
        // Read Config
        let entry = entries.next();
        if entry.is_none() {
            return Err(Box::new(BackupError::NoConfig(self.path.clone())));
        }
        let mut entry = entry.unwrap()?;
        if entry.0.get_string() != "config.yml" {
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
        if entry.0.get_string() != "files.csv" {
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

    pub fn get_previous(&mut self) -> Result<Option<Self>, Box<dyn Error>> {
        if self.config.is_none() {
            self.read_config()?;
        }
        match self
            .config
            .as_ref()
            .unwrap()
            .get_backups()
            .get_previous(&self.path)
        {
            None => Ok(None),
            Some(path) => Ok(Some(BackupReader::read(path)?)),
        }
    }

    #[allow(dead_code)]
    pub fn restore_all(
        &mut self,
        path_transform: impl FnMut(FileInfo) -> FileInfo,
        callback: impl FnMut(std::io::Result<FileInfo>),
        overwrite: bool,
    ) -> Result<(), Box<dyn Error>> {
        let list = self.extract_list()?;
        let files = list.split('\n').collect();
        let res = self.restore_selected(files, path_transform, callback, overwrite);
        self.list = Some(list);
        res
    }

    pub fn restore_selected(
        &mut self,
        selection: Vec<&str>,
        mut path_transform: impl FnMut(FileInfo) -> FileInfo,
        mut callback: impl FnMut(std::io::Result<FileInfo>),
        overwrite: bool,
    ) -> Result<(), Box<dyn Error>> {
        let mut not_found: Vec<&str> = vec![];
        let mut list = selection.into_iter();
        let mut current = match list.next() {
            Some(f) => f,
            None => return Ok(()),
        };
        for res in self.files()? {
            match res {
                Ok((mut fi, mut entry)) => {
                    while fi.get_string().as_str() > current {
                        not_found.push(current);
                        current = match list.next() {
                            Some(f) => f,
                            None => break,
                        };
                    }
                    if fi.get_string() == current {
                        current = match list.next() {
                            Some(s) => s,
                            None => current,
                        };
                        let mut path = path_transform(fi);
                        if !overwrite && path.get_path().exists() {
                            callback(Err(std::io::Error::new(
                                std::io::ErrorKind::AlreadyExists,
                                format!("File '{}' already exists", path.get_string()),
                            )));
                        } else {
                            if let Some(dir) = path.get_path().parent() {
                                callback(
                                    create_dir_all(dir)
                                        .and_then(|_| entry.unpack(path.get_path()).and(Ok(path))),
                                );
                            } else {
                                callback(entry.unpack(path.get_path()).and(Ok(path)));
                            }
                        }
                    }
                }
                Err(e) => callback(Err(e)),
            }
        }
        if not_found.len() > 0 {
            match self.get_previous()? {
                Some(mut bw) => {
                    return bw.restore_selected(not_found, path_transform, callback, overwrite)
                }
                None => {
                    for f in not_found.iter() {
                        callback(Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            format!("Could not find '{}' in backups", f),
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn restore_these(
        &mut self,
        mut path_transform: impl FnMut(FileInfo) -> FileInfo,
        mut callback: impl FnMut(std::io::Result<FileInfo>),
        overwrite: bool,
    ) -> Result<(), Box<dyn Error>> {
        for res in self.files()? {
            match res {
                Ok((fi, mut entry)) => {
                    let mut path = path_transform(fi);
                    if !overwrite && path.get_path().exists() {
                        callback(Err(std::io::Error::new(
                            std::io::ErrorKind::AlreadyExists,
                            format!("File '{}' already exists", path.get_string()),
                        )));
                    } else {
                        if let Some(dir) = path.get_path().parent() {
                            callback(
                                create_dir_all(dir)
                                    .and_then(|_| entry.unpack(path.get_path()).and(Ok(path))),
                            );
                        } else {
                            callback(entry.unpack(path.get_path()).and(Ok(path)));
                        }
                    }
                }
                Err(e) => callback(Err(e)),
            }
        }
        Ok(())
    }
}
