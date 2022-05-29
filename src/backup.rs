use std::cmp::Ordering;
/// This module contains the objects for reading and writing backups
use std::fmt::{Display, Formatter};
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use number_prefix::NumberPrefix;

use crate::compression::{CompressionDecoder, CompressionDecoderEntry, CompressionEncoder};
use crate::config::Config;
use crate::files::{FileAccessError, FileCrawler, FileInfo};
use crate::parse_date::naive_now;

#[derive(Debug)]
#[allow(dead_code)]
pub enum BackupError {
    NoConfig(PathBuf),
    NoList(PathBuf),
    NoBackup(PathBuf),
    ArchiveError(std::io::Error),
    FileError(std::io::Error),
    WriteError(std::io::Error),
    YamlError(serde_yaml::Error),
    InvalidPath(String),
    Cancel,
    FileAccessError(FileAccessError),
    IOError(std::io::Error),
    GenericError(&'static str),
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
            BackupError::WriteError(e) => {
                write!(f, "Could not write the file: {}", e)
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
            BackupError::IOError(e) => e.fmt(f),
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

impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        BackupError::IOError(e)
    }
}

impl From<FileAccessError> for BackupError {
    fn from(e: FileAccessError) -> Self {
        BackupError::FileAccessError(e)
    }
}

pub struct FileListVec(Vec<(bool, FileInfo)>);

impl FileListVec {
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
        Self { 0: list }
    }

    pub fn crawl_with_callback(
        crawler: FileCrawler,
        time: Option<NaiveDateTime>,
        all: bool,
        mut callback: impl FnMut(Result<&mut FileInfo, FileAccessError>) -> Result<(), BackupError>,
    ) -> Result<Self, BackupError> {
        let all = all || time.is_none();
        let mut list: Vec<(bool, FileInfo)> = vec![];
        for f in crawler.into_iter() {
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
        Ok(Self { 0: list })
    }

    pub fn for_each(
        &mut self,
        all: bool,
        mut callback: impl FnMut(Result<&mut FileInfo, FileAccessError>) -> Result<(), BackupError>,
    ) -> Result<(), BackupError> {
        if all {
            for fi in self.iter_all_mut() {
                callback(Ok(fi))?;
            }
        } else {
            for fi in self.iter_inc_mut() {
                callback(Ok(fi))?;
            }
        }
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &(bool, FileInfo)> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut (bool, FileInfo)> {
        self.0.iter_mut()
    }

    pub fn iter_inc(&self) -> impl Iterator<Item = &FileInfo> {
        self.0
            .iter()
            .filter_map(|(b, f)| if *b { Some(f) } else { None })
    }

    pub fn iter_inc_mut(&mut self) -> impl Iterator<Item = &mut FileInfo> {
        self.0
            .iter_mut()
            .filter_map(|(b, f)| if *b { Some(f) } else { None })
    }

    pub fn iter_all(&self) -> impl Iterator<Item = &FileInfo> {
        self.0.iter().map(|(_, f)| f)
    }

    pub fn iter_all_mut(&mut self) -> impl Iterator<Item = &mut FileInfo> {
        self.0.iter_mut().map(|(_, f)| f)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn sort_unstable_by<F>(&mut self, mut f: F)
    where
        F: FnMut(&FileInfo, &FileInfo) -> Ordering,
    {
        self.0.sort_unstable_by(|a, b| f(&a.1, &b.1));
    }

    pub fn sort_unstable(&mut self) {
        self.0.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    }
}

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
            _ => return Err(BackupError::NoList(PathBuf::new())),
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
            list_string.push_str(fi.get_string());
            list.push('\n');
        });
        list.pop();
        Self { list, version: 2 }
    }

    /// Get an iterator over all the files in the list with a flag
    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (bool, &str)> + 'a> {
        match self.version {
            2 => Box::new(self.list.split('\n').filter_map(|s: &str| {
                if s.starts_with('1') {
                    Some((s.starts_with('1'), &s[2..]))
                } else {
                    None
                }
            })),
            _ => Box::new(self.list.split('\n').map(|s| (true, s))),
        }
    }

    /// Get an iterator over all the files that are included
    pub fn iter_included<'a>(&'a self) -> Box<dyn Iterator<Item = &str> + 'a> {
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

    /// Get an iterator over all the files
    pub fn iter_all(&self) -> impl Iterator<Item = &str> {
        let offset = match self.version {
            2 => 2,
            _ => 0,
        };
        self.list.split('\n').map(move |s| &s[offset..])
    }

    pub fn filename(&self) -> &'static str {
        match self.version {
            2 => "files_v2.csv",
            _ => "files.csv",
        }
    }
}

pub struct BackupWriter {
    pub path: PathBuf,
    pub config: Config,
    pub prev_time: Option<NaiveDateTime>,
    pub list: Option<FileListVec>,
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
    fn get_files(&mut self) -> Result<&mut FileListVec, BackupError> {
        if self.list.is_none() {
            self.list = Some(FileListVec::crawl(
                FileCrawler::new(
                    &self.config.include,
                    &self.config.exclude,
                    &self.config.regex,
                    self.config.local,
                )?,
                self.prev_time,
            ));
        }
        Ok(self.list.as_mut().unwrap())
    }

    /// Iterate through all files that are added to the backup
    pub fn foreach_file(
        &mut self,
        all: bool,
        mut callback: impl FnMut(Result<&mut FileInfo, FileAccessError>) -> Result<(), BackupError>,
    ) -> Result<(), BackupError> {
        if self.list.is_some() {
            if all || self.prev_time.is_none() {
                for fi in self.list.as_mut().unwrap().iter_all_mut() {
                    callback(Ok(fi))?
                }
            } else {
                for fi in self.list.as_mut().unwrap().iter_inc_mut() {
                    callback(Ok(fi))?
                }
            }
        } else {
            self.list = Some(FileListVec::crawl_with_callback(
                FileCrawler::new(
                    &self.config.include,
                    &self.config.exclude,
                    &self.config.regex,
                    self.config.local,
                )?,
                self.prev_time,
                all,
                callback,
            )?);
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
        let list_string = FileListString::from(self.get_files()?);
        let mut encoder =
            CompressionEncoder::create(&self.path, self.config.quality, self.config.threads)?;
        self.config.time = Some(self.time);
        encoder.append_data("config.yml", self.config.to_yaml()?)?;
        encoder.append_data(list_string.filename(), list_string)?;

        let prev_time = self.prev_time.clone();
        let list = self.list.as_mut().unwrap();
        if prev_time.is_some() {
            for fi in list.iter_inc_mut() {
                let res = encoder.append_file(fi.get_path());
                on_added(fi, res.map_err(BackupError::IOError))?;
            }
        } else {
            for fi in list.iter_all_mut() {
                let res = encoder.append_file(fi.get_path());
                on_added(fi, res.map_err(BackupError::IOError))?;
            }
        }
        on_final();
        encoder.close()?;
        Ok(())
    }

    pub fn export_list<P: AsRef<Path>>(&mut self, path: P, all: bool) -> Result<(), BackupError> {
        let f = File::create(path).map_err(BackupError::FileError)?;
        let mut f = BufWriter::new(f);
        write!(f, "{:19}, {:10}, {}", "Time", "Size", "Path").map_err(BackupError::WriteError)?;
        let mut callback = |fi: &mut FileInfo| {
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
            .map_err(BackupError::WriteError)
        };
        if all || self.prev_time.is_none() {
            for fi in self.get_files()?.iter_all_mut() {
                callback(fi)?;
            }
        } else {
            for fi in self.get_files()?.iter_inc_mut() {
                callback(fi)?;
            }
        }
        Ok(())
    }
}

pub struct BackupReader {
    pub path: PathBuf,
    pub config: Option<Config>,
    list: Option<FileListString>,
}

impl BackupReader {
    /// Read a backup
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        BackupReader {
            path: path.as_ref().to_path_buf(),
            list: None,
            config: None,
        }
    }

    /// Read a backup from a config
    pub fn from_config(config: Config) -> Result<Self, BackupError> {
        match config.get_backups().get_latest() {
            None => Err(BackupError::NoBackup(config.output)),
            Some(prev) => Ok(BackupReader {
                path: prev,
                config: Some(config),
                list: None,
            }),
        }
    }

    fn get_decoder<'a>(&self) -> Result<CompressionDecoder<'a>, BackupError> {
        CompressionDecoder::read(&self.path.as_path()).map_err(|e| BackupError::ArchiveError(e))
    }

    /// Read a backup, but only return the embedded config
    pub fn read_config_only<P: AsRef<Path>>(path: P) -> Result<Config, BackupError> {
        let mut br = BackupReader::new(path);
        br.read_config()?;
        Ok(br.config.unwrap())
    }

    /// Read the embedded config from the backup
    fn read_config(&mut self) -> Result<&mut Config, BackupError> {
        let mut decoder = self.get_decoder()?;
        let entry = decoder
            .entries()
            .map_err(|e| BackupError::ArchiveError(e))?
            .next();
        let entry = match entry {
            Some(Ok(e)) => e,
            Some(Err(e)) => return Err(BackupError::ArchiveError(e)),
            None => return Err(BackupError::NoConfig(self.path.to_path_buf())),
        };
        self.parse_config(entry)?;
        Ok(self.config.as_mut().unwrap())
    }

    fn parse_config(&mut self, mut entry: CompressionDecoderEntry) -> Result<(), BackupError> {
        if entry.0.get_string() != "config.yml" {
            return Err(BackupError::NoConfig(self.path.to_path_buf()));
        }
        let mut s = String::new();
        entry
            .1
            .read_to_string(&mut s)
            .map_err(|e| BackupError::ArchiveError(e))?;
        let mut conf: Config = Config::from_yaml(&s).map_err(|e| BackupError::YamlError(e))?;
        conf.origin = self.path.to_path_buf();
        self.config = Some(conf);
        Ok(())
    }

    /// Get the config
    pub fn get_config(&mut self) -> Result<&mut Config, BackupError> {
        if self.config.is_none() {
            self.read_config()
        } else {
            Ok(self.config.as_mut().unwrap())
        }
    }

    /// Read the embedded list of files from the backup
    fn read_list(&mut self) -> Result<&FileListString, BackupError> {
        let mut decoder = self.get_decoder()?;
        let mut entries = decoder
            .entries()
            .map_err(|e| BackupError::ArchiveError(e))?
            .skip(1);
        match entries.next() {
            Some(entry) => self.parse_list(entry.map_err(|e| BackupError::ArchiveError(e))?),
            None => Err(BackupError::NoList(self.path.to_path_buf())),
        }?;
        Ok(self.list.as_ref().unwrap())
    }

    fn parse_list(&mut self, mut entry: CompressionDecoderEntry) -> Result<(), BackupError> {
        let filename = entry.0.get_string();
        let mut content = String::new();
        entry
            .1
            .read_to_string(&mut content)
            .map_err(|e| BackupError::ArchiveError(e))?;
        self.list = Some(
            FileListString::new(filename, content)
                .map_err(|_| BackupError::NoList(self.path.to_path_buf()))?,
        );
        Ok(())
    }

    /// Get the embedded list of files
    pub fn get_list(&mut self) -> Result<&FileListString, BackupError> {
        if self.list.is_none() {
            self.read_list()
        } else {
            Ok(self.list.as_ref().unwrap())
        }
    }

    /// move the list of files out of the backup
    pub fn move_list(&mut self) -> Result<FileListString, BackupError> {
        if self.list.is_none() {
            self.read_list()?;
        }
        Ok(std::mem::take(&mut self.list).unwrap())
    }

    /// Read the embedded config and file list
    pub fn read_meta(&mut self) -> Result<(&Config, &FileListString), BackupError> {
        let mut decoder = self.get_decoder()?;
        let mut entries = decoder
            .entries()
            .map_err(|e| BackupError::ArchiveError(e))?;
        // Read Config
        match entries.next() {
            Some(entry) => self.parse_config(entry.map_err(|e| BackupError::ArchiveError(e))?),
            None => Err(BackupError::NoConfig(self.path.to_path_buf())),
        }?;
        // Read File List
        match entries.next() {
            Some(entry) => self.parse_list(entry.map_err(|e| BackupError::ArchiveError(e))?),
            None => Err(BackupError::NoList(self.path.to_path_buf())),
        }?;
        // Rest
        Ok((self.config.as_ref().unwrap(), self.list.as_ref().unwrap()))
    }

    /// Get the embedded list of files
    pub fn get_meta(&mut self) -> Result<(&Config, &FileListString), BackupError> {
        if self.config.is_none() || self.list.is_none() {
            return self.read_meta();
        } else {
            Ok((self.config.as_mut().unwrap(), self.list.as_ref().unwrap()))
        }
    }

    /// Is this an incemental backup
    pub fn is_incremental(&mut self) -> Result<bool, BackupError> {
        Ok(self.get_config()?.incremental)
    }

    /// Try to find the previous backup
    pub fn get_previous(&mut self) -> Result<Option<Self>, BackupError> {
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
            Some(path) => Ok(Some(BackupReader::new(path))),
        }
    }

    pub fn export_list<P: AsRef<Path>>(&mut self, path: P) -> Result<(), BackupError> {
        let mut f = File::create(path).map_err(BackupError::FileError)?;
        f.write_all(&self.get_list()?.list.as_bytes())
            .map_err(BackupError::WriteError)?;
        Ok(())
    }

    #[allow(unused)]
    pub fn restore_this(
        &mut self,
        path_transform: impl FnMut(FileInfo) -> FileInfo,
        callback: impl FnMut(std::io::Result<FileInfo>) -> Result<(), BackupError>,
        overwrite: bool,
    ) -> Result<(), BackupError> {
        let list = self.move_list()?;
        let selection = list.iter_included().collect();
        let res = self.restore(selection, path_transform, callback, overwrite, false);
        self.list = Some(list);
        res
    }

    #[allow(unused)]
    pub fn restore_all(
        &mut self,
        path_transform: impl FnMut(FileInfo) -> FileInfo,
        callback: impl FnMut(std::io::Result<FileInfo>) -> Result<(), BackupError>,
        overwrite: bool,
    ) -> Result<(), BackupError> {
        let list = self.move_list()?;
        let selection = list.iter_all().collect();
        let res = self.restore(selection, path_transform, callback, overwrite, true);
        self.list = Some(list);
        res
    }

    /// Restore specific files
    pub fn restore<S: AsRef<str>>(
        &mut self,
        selection: Vec<S>,
        mut path_transform: impl FnMut(FileInfo) -> FileInfo,
        mut callback: impl FnMut(std::io::Result<FileInfo>) -> Result<(), BackupError>,
        overwrite: bool,
        recursive: bool,
    ) -> Result<(), BackupError> {
        let mut not_found: Vec<&str> = vec![];
        let mut list = selection.iter().map(|v| v.as_ref());
        let mut current = match list.next() {
            Some(f) => f,
            None => return Ok(()),
        };
        for res in self
            .get_decoder()?
            .entries()
            .map_err(|e| BackupError::ArchiveError(e))?
            .skip(2)
        {
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
                                format!("File '{}' already exists.", path.get_string()),
                            )))?;
                        } else {
                            if let Some(dir) = path.get_path().parent() {
                                callback(
                                    create_dir_all(dir)
                                        .and_then(|_| entry.unpack(path.get_path()).and(Ok(path))),
                                )?;
                            } else {
                                callback(entry.unpack(path.get_path()).and(Ok(path)))?;
                            }
                        }
                        current = match list.next() {
                            Some(s) => s,
                            None => break,
                        };
                    }
                }
                Err(e) => callback(Err(e))?,
            }
        }
        if not_found.len() > 0 {
            if recursive {
                if let Some(mut bw) = self.get_previous()? {
                    return bw.restore(not_found, path_transform, callback, overwrite, recursive);
                }
            }
            for f in not_found.iter() {
                callback(Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "Could not find '{}' in backup '{}'.",
                        f,
                        self.path.to_string_lossy()
                    ),
                )))?;
            }
        }
        Ok(())
    }
}
