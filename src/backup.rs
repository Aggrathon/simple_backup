/// This module contains the objects for reading and writing backups
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use number_prefix::NumberPrefix;

use crate::compression::{CompressionDecoder, CompressionDecoderEntry, CompressionEncoder};
use crate::config::Config;
use crate::files::{FileAccessError, FileCrawler, FileInfo};
use crate::parse_date::{self, naive_now};

#[derive(Debug)]
pub enum BackupError {
    NoConfig(PathBuf),
    NoList(PathBuf),
    NoBackup(PathBuf),
    ArchiveError(std::io::Error),
    FileError(std::io::Error),
    YamlError(serde_yaml::Error),
    InvalidPath(String),
    Cancel,
    FileAccessError(FileAccessError),
    GenericError(std::io::Error),
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
                write!(f, "Could not find a backup: {}", path.to_string_lossy())
            }
            BackupError::ArchiveError(e) => {
                write!(f, "Could not read the backup: {}", e)
            }
            BackupError::FileError(e) => {
                write!(f, "Could not access the file: {}", e)
            }
            BackupError::YamlError(e) => {
                write!(f, "Could not parse the config: {}", e)
            }
            BackupError::InvalidPath(path) => {
                write!(f, "The path must be either a config (.yml), a backup (.tar.zst), or a directory containing backups: {}", path)
            }
            BackupError::Cancel => {
                write!(f, "The operation has been cancelled")
            }
            BackupError::FileAccessError(e) => e.fmt(f),
            BackupError::GenericError(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for BackupError {}

impl From<serde_yaml::Error> for BackupError {
    fn from(e: serde_yaml::Error) -> Self {
        BackupError::YamlError(e)
    }
}

pub struct BackupWriter {
    pub path: PathBuf,
    pub config: Config,
    pub prev_time: Option<NaiveDateTime>,
    pub list: Option<Vec<FileInfo>>,
    time: NaiveDateTime,
}

impl BackupWriter {
    /// Create a new backup
    pub fn new(config: Config) -> (Self, Option<BackupError>) {
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
        let path = config.get_new_output();
        (
            Self {
                config,
                path,
                prev_time,
                list: None,
                time: naive_now(),
            },
            error,
        )
    }

    /// List all files that are added to the backup
    pub fn get_files(&mut self) -> Result<&mut Vec<FileInfo>, BackupError> {
        if self.list.is_none() {
            let fc = FileCrawler::new(
                &self.config.include,
                &self.config.exclude,
                &self.config.regex,
                self.config.local,
            )
            .map_err(BackupError::GenericError)?;
            let mut list: Vec<FileInfo> = fc
                .into_iter()
                .filter_map(|fi| match fi {
                    Ok(fi) => Some(fi),
                    Err(_) => None,
                })
                .collect();
            list.sort_unstable();
            self.list = Some(list);
        }
        Ok(self.list.as_mut().unwrap())
    }

    pub fn iter_files<'a>(
        &'a mut self,
    ) -> Result<impl std::iter::Iterator<Item = &mut FileInfo> + 'a, BackupError> {
        let time = self.prev_time.clone();
        Ok(self
            .get_files()?
            .iter_mut()
            .filter(move |fi| fi.time >= time))
    }

    /// Iterate through all files that are added to the backup
    pub fn foreach_file(
        &mut self,
        all: bool,
        callback: impl FnMut(Result<&mut FileInfo, FileAccessError>) -> Result<(), BackupError>,
    ) -> Result<(), BackupError> {
        let mut callback = callback;
        if self.list.is_some() {
            if all || self.prev_time.is_none() {
                for fi in self.list.as_mut().unwrap().iter_mut() {
                    callback(Ok(fi))?
                }
            } else {
                let time = self.prev_time.unwrap();
                for fi in self.list.as_mut().unwrap().iter_mut() {
                    if fi.time.unwrap() >= time {
                        callback(Ok(fi))?
                    }
                }
            }
        } else {
            let fc = FileCrawler::new(
                &self.config.include,
                &self.config.exclude,
                &self.config.regex,
                self.config.local,
            )
            .map_err(BackupError::GenericError)?;
            let mut list = Vec::<FileInfo>::with_capacity(500);
            if all || self.prev_time.is_none() {
                for res in fc.into_iter() {
                    match res {
                        Ok(mut fi) => {
                            callback(Ok(&mut fi))?;
                            list.push(fi);
                        }
                        Err(e) => callback(Err(e))?,
                    }
                }
            } else {
                let time = self.prev_time.unwrap();
                for res in fc.into_iter() {
                    match res {
                        Ok(mut fi) => {
                            if fi.time.unwrap() >= time {
                                callback(Ok(&mut fi))?
                            }
                            list.push(fi);
                        }
                        Err(e) => callback(Err(e))?,
                    }
                }
            }
            list.sort_unstable();
            self.list = Some(list);
        }
        Ok(())
    }

    /// Write (and compress) the backup to disk
    pub fn write(
        &mut self,
        on_added: impl FnMut(&mut FileInfo, Result<(), BackupError>) -> Result<(), BackupError>,
        on_final: impl FnOnce(),
    ) -> Result<(), BackupError> {
        match self.write_internal(on_added, on_final) {
            Ok(_) => Ok(()),
            #[allow(unused_must_use)]
            Err(e) => {
                // Clean up failed backup (allowed to fail without checking)
                std::fs::remove_file(&self.path);
                Err(e)
            }
        }
    }

    fn write_internal(
        &mut self,
        mut on_added: impl FnMut(&mut FileInfo, Result<(), BackupError>) -> Result<(), BackupError>,
        on_final: impl FnOnce(),
    ) -> Result<(), BackupError> {
        let mut list_string = String::new();
        {
            let list = self.get_files()?;
            list_string.reserve(list.len() * 200);
            list.iter_mut().for_each(|fi| {
                #[cfg(target_os = "windows")]
                list_string.push_str(&fi.get_string().replace('\\', "/"));
                #[cfg(not(target_os = "windows"))]
                list_string.push_str(fi.get_string());
                list_string.push('\n');
            });
            list_string.pop();
        }
        {
            let mut encoder =
                CompressionEncoder::create(&self.path, self.config.quality, self.config.threads)
                    .map_err(BackupError::GenericError)?;
            self.config.time = Some(self.time);
            encoder
                .append_data("config.yml", self.config.to_yaml()?)
                .map_err(BackupError::GenericError)?;
            encoder
                .append_data("files.csv", list_string)
                .map_err(BackupError::GenericError)?;

            let prev_time = self.prev_time.clone();
            let list = self.list.as_mut().unwrap();
            match prev_time {
                Some(prev_time) => {
                    for fi in list.iter_mut() {
                        match fi.get_path().metadata() {
                            Ok(md) => match md.modified() {
                                Ok(time) => {
                                    if parse_date::system_to_naive(time) >= prev_time {
                                        let res = encoder
                                            .append_file(fi.get_path())
                                            .map_err(BackupError::GenericError);
                                        on_added(fi, res)?;
                                    }
                                }
                                Err(e) => on_added(fi, Err(BackupError::GenericError(e)))?,
                            },
                            Err(e) => on_added(fi, Err(BackupError::GenericError(e)))?,
                        }
                    }
                }
                None => {
                    for fi in list.iter_mut() {
                        let res = encoder
                            .append_file(fi.get_path())
                            .map_err(BackupError::GenericError);
                        on_added(fi, res)?;
                    }
                }
            }
            on_final();
            encoder.close().map_err(BackupError::GenericError)?;
        }
        Ok(())
    }

    pub fn export_list<P: AsRef<Path>>(&mut self, path: P, all: bool) -> Result<(), BackupError> {
        let f = File::create(path).map_err(BackupError::GenericError)?;
        let mut f = BufWriter::new(f);
        write!(f, "{:19}, {:10}, {}", "Time", "Size", "Path").map_err(BackupError::GenericError)?;
        self.foreach_file(all, |res| {
            if let Ok(fi) = res {
                match NumberPrefix::binary(fi.size as f64) {
                    NumberPrefix::Standalone(number) => {
                        write!(
                            f,
                            "\n{}, {:>6.2} KiB, {}",
                            fi.time.unwrap().format("%Y-%m-%d %H:%M:%S"),
                            number / 1024.0,
                            &fi.get_string()
                        )
                    }
                    NumberPrefix::Prefixed(prefix, number) => {
                        write!(
                            f,
                            "\n{}, {:>6.2} {}B, {}",
                            fi.time.unwrap().format("%Y-%m-%d %H:%M:%S"),
                            number,
                            prefix,
                            &fi.get_string()
                        )
                    }
                }
                .map_err(BackupError::GenericError)?
            }
            Ok(())
        })?;
        Ok(())
    }
}

pub struct BackupReader<'a> {
    decoder: CompressionDecoder<'a>,
    used: bool,
    pub path: PathBuf,
    pub config: Option<Config>,
    pub list: Option<String>,
}

impl<'a> BackupReader<'a> {
    /// Read a backup
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self, BackupError> {
        let decoder = CompressionDecoder::read(&path).map_err(|e| BackupError::ArchiveError(e))?;
        Ok(BackupReader {
            path: path.as_ref().to_path_buf(),
            decoder,
            used: false,
            list: None,
            config: None,
        })
    }

    /// Read a backup from a config
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

    fn use_decoder(&mut self) -> Result<(), BackupError> {
        if self.used {
            self.decoder =
                CompressionDecoder::read(&self.path).map_err(|e| BackupError::ArchiveError(e))?;
        } else {
            self.used = true;
        }
        Ok(())
    }

    /// Read a backup, but only return the embedded config
    pub fn read_config_only<P: AsRef<Path>>(path: P) -> Result<Config, BackupError> {
        let mut br = BackupReader::read(path)?;
        br.read_config()?;
        Ok(br.config.unwrap())
    }

    /// Read the embedded config from the backup
    pub fn read_config(&mut self) -> Result<&mut Config, BackupError> {
        self.use_decoder()?;
        let entry = self
            .decoder
            .entries()
            .map_err(|e| BackupError::ArchiveError(e))?
            .next();
        let mut entry = match entry {
            Some(Ok(e)) => e,
            Some(Err(e)) => return Err(BackupError::ArchiveError(e)),
            None => return Err(BackupError::NoConfig(self.path.clone())),
        };
        if entry.0.get_string() != "config.yml" {
            return Err(BackupError::NoConfig(self.path.clone()));
        }
        let mut s = String::new();
        entry
            .1
            .read_to_string(&mut s)
            .map_err(|e| BackupError::ArchiveError(e))?;
        let mut conf: Config = Config::from_yaml(&s).map_err(|e| BackupError::YamlError(e))?;
        conf.origin = self.path.clone();
        self.config = Some(conf);
        Ok(self.config.as_mut().unwrap())
    }

    pub fn get_config(&mut self) -> Result<&mut Config, BackupError> {
        if self.config.is_none() {
            self.read_config()
        } else {
            Ok(self.config.as_mut().unwrap())
        }
    }

    /// Read the embedded list of files from the backup
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

    /// move the list of files out of the backup
    pub fn extract_list(&mut self) -> Result<String, Box<dyn Error>> {
        if self.list.is_none() {
            self.read_list()?;
        }
        Ok(std::mem::take(&mut self.list).unwrap())
    }

    /// Iterator over the files in the backup
    pub fn files(
        &mut self,
    ) -> Result<impl Iterator<Item = std::io::Result<CompressionDecoderEntry<'_, 'a>>>, BackupError>
    {
        self.use_decoder()?;
        Ok(self
            .decoder
            .entries()
            .map_err(|e| BackupError::ArchiveError(e))?
            .skip(2))
    }

    /// Read the embedded config and file list, and return the iterator over the files
    #[allow(dead_code)]
    pub fn read_all(
        &mut self,
    ) -> Result<
        (
            &Config,
            &String,
            impl Iterator<Item = std::io::Result<CompressionDecoderEntry<'_, 'a>>>,
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
        conf.origin = self.path.clone();
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

    /// Is this an incemental backup
    pub fn is_incremental(&mut self) -> Result<bool, Box<dyn Error>> {
        Ok(self.get_config()?.incremental)
    }

    /// Try to find the previous backup
    pub fn get_previous(&mut self) -> Result<Option<Self>, Box<dyn Error>> {
        if !self.is_incremental()? {
            return Ok(None);
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

    /// Restore all files from (only) this backup
    #[allow(dead_code)]
    pub fn restore_this(
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

    /// Restore all files
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

    /// Restore specific files
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
                    {
                        let fis = fi.get_string().as_str();
                        while fis > current {
                            not_found.push(current);
                            current = match list.next() {
                                Some(f) => f,
                                None => break,
                            };
                        }
                    }
                    if fi.get_string() == current {
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
                        current = match list.next() {
                            Some(s) => s,
                            None => break,
                        };
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
                            format!(
                                "Could not find '{}' in backup '{}'",
                                f,
                                self.path.to_string_lossy()
                            ),
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}
