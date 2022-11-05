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
use crate::lists::{FileListString, FileListVec};
use crate::parse_date::naive_now;
use crate::utils::extend_pathbuf;

pub(crate) const BACKUP_FILE_EXTENSION: &str = ".tar.zst";
pub(crate) const CONFIG_DEFAULT_NAME: &str = "config.yml";
pub(crate) const CONFIG_FILE_EXTENSION: &str = ".yml";

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
    DeleteError(std::io::Error),
    RenameError(String, String, std::io::Error),
    GenericError(&'static str),
    Unspecified,
    FileExists(PathBuf),
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
            BackupError::DeleteError(e) => {
                write!(f, "Could not delete the file: {}", e)
            }
            BackupError::RenameError(b, a, e) => {
                write!(f, "Could not rename '{}' to '{}': {}", b, a, e)
            }
            BackupError::YamlError(e) => {
                write!(f, "Could not parse the config: {}", e)
            }
            BackupError::InvalidPath(path) => {
                write!(f, "The path must be either a config ({}), a backup ({}), or a directory containing backups: {}", CONFIG_FILE_EXTENSION, BACKUP_FILE_EXTENSION, path)
            }
            BackupError::Cancel => {
                write!(f, "The operation has been cancelled")
            }
            BackupError::FileAccessError(e) => e.fmt(f),
            BackupError::IOError(e) => e.fmt(f),
            BackupError::GenericError(e) => e.fmt(f),
            BackupError::Unspecified => write!(f, "Unspecified error"),
            BackupError::FileExists(p) => write!(f, "Path already exists: {}", p.to_string_lossy()),
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
        let all = all || self.prev_time.is_none();
        if self.list.is_some() {
            for (b, fi) in self.list.as_mut().unwrap().iter_mut() {
                if all || *b {
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
        encoder.append_data(CONFIG_DEFAULT_NAME, self.config.as_yaml()?)?;
        encoder.append_data(list_string.filename(), list_string)?;

        let list = self.list.as_mut().unwrap();
        for (b, fi) in list.iter_mut() {
            if *b {
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
        write!(f, "{:19}, {:10}, Path", "Time", "Size").map_err(BackupError::WriteError)?;
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
        let all = all || self.prev_time.is_none();
        for (b, fi) in self.get_files()?.iter_mut() {
            if all || *b {
                callback(fi)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct BackupReader {
    pub path: FileInfo,
    pub config: Option<Config>,
    list: Option<FileListString>,
}

impl BackupReader {
    /// Read a backup
    pub fn new(path: PathBuf) -> Self {
        BackupReader {
            path: path.into(),
            list: None,
            config: None,
        }
    }

    /// Read a backup from a config
    pub fn from_config(config: Config) -> Result<Self, BackupError> {
        match config.get_backups().get_latest() {
            None => Err(BackupError::NoBackup(config.output)),
            Some(prev) => Ok(BackupReader {
                path: prev.into(),
                config: Some(config),
                list: None,
            }),
        }
    }

    pub fn get_decoder<'a>(&self) -> Result<CompressionDecoder<'a>, BackupError> {
        CompressionDecoder::read(self.path.copy_path().as_path()).map_err(BackupError::ArchiveError)
    }

    /// Read a backup, but only return the embedded config
    pub fn read_config_only(path: PathBuf) -> Result<Config, BackupError> {
        let mut br = BackupReader::new(path);
        br.read_config()?;
        Ok(br.config.unwrap())
    }

    /// Read the embedded config from the backup
    fn read_config(&mut self) -> Result<&mut Config, BackupError> {
        let mut decoder = self.get_decoder()?;
        let entry = decoder.entries().map_err(BackupError::ArchiveError)?.next();
        let entry = match entry {
            Some(Ok(e)) => e,
            Some(Err(e)) => return Err(BackupError::ArchiveError(e)),
            None => return Err(BackupError::NoConfig(self.path.clone_path())),
        };
        self.parse_config(entry)?;
        Ok(self.config.as_mut().unwrap())
    }

    fn parse_config(&mut self, mut entry: CompressionDecoderEntry) -> Result<(), BackupError> {
        if entry.0.get_string() != CONFIG_DEFAULT_NAME {
            return Err(BackupError::NoConfig(self.path.clone_path()));
        }
        let mut s = String::new();
        entry
            .1
            .read_to_string(&mut s)
            .map_err(BackupError::ArchiveError)?;
        let mut conf: Config = Config::from_yaml(&s).map_err(BackupError::YamlError)?;
        conf.origin = self.path.clone_path();
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
            .map_err(BackupError::ArchiveError)?
            .skip(1);
        match entries.next() {
            Some(entry) => self.parse_list(entry.map_err(BackupError::ArchiveError)?),
            None => Err(BackupError::NoList(self.path.clone_path())),
        }?;
        Ok(self.list.as_ref().unwrap())
    }

    fn parse_list(&mut self, mut entry: CompressionDecoderEntry) -> Result<(), BackupError> {
        let filename = entry.0.get_string();
        let mut content = String::new();
        entry
            .1
            .read_to_string(&mut content)
            .map_err(BackupError::ArchiveError)?;
        self.list = Some(
            FileListString::new(filename, content)
                .map_err(|_| BackupError::NoList(self.path.clone_path()))?,
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
        let mut entries = decoder.entries().map_err(BackupError::ArchiveError)?;
        // Read Config
        match entries.next() {
            Some(entry) => self.parse_config(entry.map_err(BackupError::ArchiveError)?),
            None => Err(BackupError::NoConfig(self.path.clone_path())),
        }?;
        // Read File List
        match entries.next() {
            Some(entry) => self.parse_list(entry.map_err(BackupError::ArchiveError)?),
            None => Err(BackupError::NoList(self.path.clone_path())),
        }?;
        // Rest
        Ok((self.config.as_ref().unwrap(), self.list.as_ref().unwrap()))
    }

    /// Get the embedded list of files
    pub fn get_meta(&mut self) -> Result<(&Config, &FileListString), BackupError> {
        if self.config.is_none() || self.list.is_none() {
            self.read_meta()
        } else {
            Ok((self.config.as_mut().unwrap(), self.list.as_ref().unwrap()))
        }
    }

    /// Is this an incemental backup
    pub fn check_incremental(&mut self) -> Result<bool, BackupError> {
        Ok(self.get_config()?.incremental)
    }

    /// Try to find the previous backup
    pub fn get_previous(&mut self) -> Result<Option<Self>, BackupError> {
        if !self.check_incremental()? {
            return Ok(None);
        }
        match self
            .config
            .as_ref()
            .unwrap()
            .get_backups()
            .get_previous(self.path.get_path())
        {
            None => Ok(None),
            Some(path) => Ok(Some(BackupReader::new(path))),
        }
    }

    pub fn export_list<P: AsRef<Path>>(&mut self, path: P) -> Result<(), BackupError> {
        let mut f = File::create(path).map_err(BackupError::FileError)?;
        f.write_all(self.get_list()?.as_ref())
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
        let selection = list.iter().map(|v| v.1).collect();
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
        let selection = list.iter().map(|v| v.1).collect();
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
        if selection.is_empty() {
            return Ok(());
        }
        let mut not_found: Vec<&str> = vec![];
        let mut decoder = self.get_decoder()?;
        let mut entries = decoder.entries().map_err(BackupError::ArchiveError)?;
        let unsorted = match entries.nth(1) {
            Some(r) => r?.0.get_string() == "files.csv",
            None => return Err(BackupError::NoList(self.path.clone_path())),
        };
        let mut list = selection.iter().map(|v| v.as_ref());
        let mut current = if unsorted {
            not_found.extend(selection.iter().map(|v| v.as_ref()));
            not_found.sort_unstable();
            ""
        } else {
            list.next().unwrap_or("")
        };
        'decoder: for res in entries {
            match res {
                Ok((mut fi, mut entry)) => {
                    let restore = if unsorted {
                        // Unsorted is needed to be able to extract files from some old
                        // simple_backup backups, where the files were not properly sorted.
                        let fis = fi.get_string().as_str();
                        if let Ok(i) = not_found.binary_search(&fis) {
                            not_found.remove(i);
                            true
                        } else {
                            false
                        }
                    } else {
                        // Otherwise assume that everything is sorted
                        let fis = fi.get_string().as_str();
                        while fis > current {
                            if recursive {
                                not_found.push(current);
                            } else {
                                callback(Err(std::io::Error::new(
                                    std::io::ErrorKind::NotFound,
                                    format!(
                                        "Could not find '{}' in backup '{}' ({}).",
                                        current,
                                        self.path.get_string(),
                                        &fis
                                    ),
                                )))?;
                            }
                            current = match list.next() {
                                Some(f) => f,
                                None => break 'decoder,
                            };
                        }
                        fi.get_string() == current
                    };
                    if restore {
                        let mut path = path_transform(fi);
                        if !overwrite && path.get_path().exists() {
                            callback(Err(std::io::Error::new(
                                std::io::ErrorKind::AlreadyExists,
                                format!("File '{}' already exists.", path.get_string()),
                            )))?;
                        } else if let Some(dir) = path.get_path().parent() {
                            callback(
                                create_dir_all(dir)
                                    .and_then(|_| entry.unpack(path.get_path()).and(Ok(path))),
                            )?;
                        } else {
                            callback(entry.unpack(path.get_path()).and(Ok(path)))?;
                        }
                        if unsorted {
                            if not_found.is_empty() {
                                break 'decoder;
                            }
                        } else {
                            current = match list.next() {
                                Some(s) => s,
                                None => break 'decoder,
                            };
                        }
                    }
                }
                Err(e) => callback(Err(e))?,
            }
        }
        if !not_found.is_empty() {
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
                        self.path.get_string()
                    ),
                )))?;
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for BackupReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackupReader")
            .field("path", &self.path)
            .finish()
    }
}

pub struct BackupMerger {
    pub path: PathBuf,
    tmp_path: PathBuf,
    readers: Vec<BackupReader>,
    pub files: FileListVec,
    delete: bool,
    overwrite: bool,
    quality: Option<i32>,
    threads: Option<u32>,
}

impl BackupMerger {
    /// Create a new backup merger.
    /// The merged backup can either contain only files mentioned in the latest backup, or all files from all backups.
    pub fn new(
        path: Option<PathBuf>,
        mut readers: Vec<BackupReader>,
        all: bool,
        delete: bool,
        overwrite: bool,
        quality: Option<i32>,
        threads: Option<u32>,
    ) -> Result<Self, BackupError> {
        if readers.len() < 2 {
            return Err(BackupError::GenericError(
                "At least two backups are needed for merging",
            ));
        }
        readers.sort_by(|a, b| a.path.cmp(&b.path));
        readers.dedup_by(|a, b| b.path == a.path);
        for r in readers.iter_mut() {
            r.get_meta()?;
        }
        readers.sort_by_cached_key(|r| {
            r.config
                .as_ref()
                .unwrap()
                .time
                .expect("A stored backup should always contain the backup time!")
        });
        readers.reverse();

        let path = match path {
            Some(path) => path,
            None => readers.first().unwrap().path.clone_path(),
        };

        let mut files = FileListVec::default();
        {
            let mut lists = readers
                .iter()
                .map(|r| Box::new(r.list.as_ref().unwrap().iter().peekable()))
                .collect::<Vec<_>>();
            loop {
                let s = if all {
                    lists
                        .iter_mut()
                        .filter_map(|p| p.peek())
                        .min()
                        .map(|(_, s)| String::from(*s))
                } else {
                    lists
                        .first_mut()
                        .unwrap()
                        .peek()
                        .map(|(_, s)| String::from(*s))
                };
                let mut inc = false;
                match s {
                    None => break,
                    Some(s) => {
                        for p in lists.iter_mut() {
                            if let Some((b, s2)) = p.peek() {
                                inc = inc || *b;
                                if s.as_str() >= *s2 {
                                    p.next();
                                }
                            }
                        }
                        files.push(inc, FileInfo::from(s));
                    }
                };
            }
        }
        Ok(Self {
            path,
            tmp_path: PathBuf::new(),
            readers,
            files,
            delete,
            overwrite,
            quality,
            threads,
        })
    }

    pub fn deconstruct(self) -> Vec<BackupReader> {
        self.readers
    }

    /// Write (and compress) the backup to disk
    pub fn write(
        &mut self,
        on_added: impl FnMut(&mut FileInfo, Result<(), BackupError>) -> Result<(), BackupError>,
        on_final: impl FnOnce(),
    ) -> Result<(), BackupError> {
        self.tmp_path = self.get_tmp_output();
        self.write_internal(on_added, on_final).map_err(|e| {
            // Clean up failed merge (allowed to fail without checking)
            #[allow(unused_must_use)]
            {
                std::fs::remove_file(&self.tmp_path);
                self.tmp_path.clear();
            }
            e
        })?;
        self.cleanup()
    }

    fn write_internal(
        &mut self,
        mut on_added: impl FnMut(&mut FileInfo, Result<(), BackupError>) -> Result<(), BackupError>,
        on_final: impl FnOnce(),
    ) -> Result<(), BackupError> {
        let config = self
            .readers
            .first_mut()
            .expect("The number of readers should always be more than one!")
            .config
            .as_mut()
            .expect("The config should already be read!");
        let quality = self.quality.unwrap_or(config.quality);
        let threads = self.threads.unwrap_or(config.threads);
        let config = config.as_yaml()?;

        let mut decoders = self
            .readers
            .iter_mut()
            .map(|r| r.get_decoder())
            .collect::<Result<Vec<_>, BackupError>>()?;
        let mut entries = decoders
            .iter_mut()
            .map(|d| {
                Ok(d.entries()
                    .map_err(BackupError::ArchiveError)?
                    .skip(2)
                    .peekable())
            })
            .collect::<Result<Vec<_>, BackupError>>()?;

        if let Some(p) = self.tmp_path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let list = FileListString::from(&mut self.files);
        let mut encoder = CompressionEncoder::create(&self.tmp_path, quality, threads)
            .map_err(BackupError::WriteError)?;
        encoder
            .append_data(CONFIG_DEFAULT_NAME, config)
            .map_err(BackupError::WriteError)?;
        encoder
            .append_data(list.filename(), list)
            .map_err(BackupError::WriteError)?;

        for (_, file) in self.files.iter_mut() {
            let file = file.get_string();
            'outer: for p in entries.iter_mut() {
                while let Some(e) = p.peek_mut() {
                    match e {
                        Err(_) => {
                            p.next().unwrap()?;
                        }
                        Ok((fi, _)) => match fi.get_string().cmp(file) {
                            std::cmp::Ordering::Less => {
                                p.next();
                            }
                            std::cmp::Ordering::Equal => {
                                let (mut fi, entry) = p.next().unwrap()?;
                                on_added(
                                    &mut fi,
                                    encoder.append_entry(entry).map_err(BackupError::WriteError),
                                )?;
                                break 'outer;
                            }
                            std::cmp::Ordering::Greater => break,
                        },
                    }
                }
            }
        }
        on_final();
        encoder.close()?;
        Ok(())
    }

    fn get_tmp_output(&self) -> PathBuf {
        let mut path = self.path.clone();
        while path.exists() {
            path = extend_pathbuf(path, ".tmp");
        }
        path
    }

    fn cleanup(&mut self) -> Result<(), BackupError> {
        if self.delete {
            for r in self.readers.iter_mut() {
                std::fs::remove_file(r.path.get_path()).map_err(BackupError::DeleteError)?;
            }
        } else {
            for r in self.readers.iter_mut() {
                let mut path = r.path.clone_path();
                path = extend_pathbuf(path, ".old");
                while path.exists() {
                    path = extend_pathbuf(path, ".old");
                }
                std::fs::rename(r.path.get_path(), &path).map_err(|e| {
                    BackupError::RenameError(
                        r.path.get_string().to_string(),
                        path.to_string_lossy().to_string(),
                        e,
                    )
                })?;
                r.path = path.into();
            }
        }
        if self.path != self.tmp_path {
            if self.path.exists() {
                if self.overwrite {
                    std::fs::remove_file(&self.path).map_err(BackupError::DeleteError)?;
                } else {
                    return Err(BackupError::FileExists(self.path.to_path_buf()));
                }
            }
            if let Some(p) = self.path.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::rename(&self.tmp_path, &self.path).map_err(|e| {
                BackupError::RenameError(
                    self.tmp_path.to_string_lossy().to_string(),
                    self.path.to_string_lossy().to_string(),
                    e,
                )
            })?;
        }
        self.tmp_path.clear();
        Ok(())
    }
}
