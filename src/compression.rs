use std::{
    fmt::Debug,
    fs::{create_dir_all, remove_file, File},
    io::BufReader,
    path::{Path, PathBuf},
};

use path_clean::PathClean;
use tar::{Archive, Builder, Entry, Header};
use zstd::{Decoder, Encoder};

use crate::files::FileInfo;

pub struct CompressionEncoder<'a>(Builder<Encoder<'a, File>>);

impl<'a> CompressionEncoder<'a> {
    /// Create a compressed archive
    pub fn create<P: AsRef<Path>>(path: P, quality: i32, threads: u32) -> std::io::Result<Self> {
        if let Some(p) = path.as_ref().parent() {
            create_dir_all(p)?;
        }
        let file = File::create(&path)?;
        let cleanup = |err| {
            remove_file(&path).unwrap_or_default();
            err
        };
        let mut encoder = Encoder::new(file, quality).map_err(cleanup)?;
        encoder.multithread(threads).map_err(cleanup)?;
        let archive = Builder::new(encoder);
        Ok(CompressionEncoder { 0: archive })
    }

    /// Finnish compressing the archive and close the file
    pub fn close(self) -> std::io::Result<()> {
        self.0.into_inner()?.finish()?.sync_all()?;
        Ok(())
    }

    /// Add a file to the compressed archive
    pub fn append_file(&mut self, file: &PathBuf) -> std::io::Result<()> {
        let name = path_to_archive(&file);
        self.0.append_path_with_name(&file, name)
    }

    /// Add raw data as a file to the compressed archive
    pub fn append_data<P: AsRef<Path>, B: AsRef<[u8]>>(
        &mut self,
        name: P,
        content: B,
    ) -> std::io::Result<()> {
        let content = content.as_ref();
        let mut header = Header::new_gnu();
        header.set_size(content.len() as u64);
        self.0.append_data(&mut header, &name, content)
    }
}

pub type CompressionDecoderEntry<'dummy, 'a> =
    (FileInfo, Entry<'dummy, Decoder<'a, BufReader<File>>>);
pub struct CompressionDecoder<'a>(Archive<Decoder<'a, BufReader<File>>>);

impl<'a> Debug for CompressionDecoder<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompressionDecoder").finish()
    }
}

impl<'a> CompressionDecoder<'a> {
    /// Read a compressed archive
    pub fn read<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = File::open(&path)?;
        let decoder = Decoder::new(file)?;
        let mut archive = Archive::new(decoder);
        archive.set_unpack_xattrs(true);
        archive.set_preserve_permissions(true);
        archive.set_overwrite(true);
        Ok(Self { 0: archive })
    }

    /// Iterate over the files in the compressed archive
    pub fn entries(
        &mut self,
    ) -> std::io::Result<impl Iterator<Item = std::io::Result<CompressionDecoderEntry<'_, 'a>>>>
    {
        Ok(self.0.entries()?.map(|entry| {
            let entry = entry?;
            let path = entry.header().path()?;
            Ok((path_from_archive(&path), entry))
        }))
    }
}

/// Encode a path for adding to a tar archive
#[cfg(target_os = "windows")]
fn path_to_archive(path: &PathBuf) -> String {
    if path.has_root() {
        "abs".to_string() + &path.to_string_lossy().replace('\\', "/")
    } else {
        "rel/".to_string() + &path.clean().to_string_lossy().replace('\\', "/")
    }
}

/// Encode a path for adding to a tar archive
#[cfg(not(target_os = "windows"))]
fn path_to_archive(path: &PathBuf) -> String {
    if path.has_root() {
        "abs".to_string() + &path.to_string_lossy()
    } else {
        "rel/".to_string() + &path.clean().to_string_lossy()
    }
}

/// Decode a path from a tar archive
fn path_from_archive<P: AsRef<Path>>(path: P) -> FileInfo {
    let path = path.as_ref();
    let string = path.to_string_lossy();
    if string.starts_with("rel/") {
        FileInfo::from(string[4..].to_string())
    } else if string.starts_with("abs") {
        FileInfo::from(string[3..].to_string())
    } else if string == "rel" {
        FileInfo::from(".")
    } else {
        FileInfo::from(path)
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, path::PathBuf};

    use path_absolutize::Absolutize;
    use tar::{Archive, Builder, Header};

    use super::{path_from_archive, path_to_archive};

    #[test]
    fn paths_abs() {
        let dir = PathBuf::from(".").absolutize().unwrap().to_path_buf();
        let pta = path_to_archive(&dir);
        let out = path_from_archive(&PathBuf::from(&pta)).consume_path();
        assert_eq!(dir, out);

        let tmp: Vec<u8> = vec![];
        let mut tar = Builder::new(tmp);
        let mut header = Header::new_gnu();
        header.set_size(2);
        tar.append_data(&mut header, pta, "ab".as_bytes()).unwrap();
        let tmp = tar.into_inner().unwrap();
        let mut tar = Archive::new(Cursor::new(tmp));
        let entry = tar.entries().unwrap().next().unwrap().unwrap();
        let pia = entry.header().path().unwrap();
        let out = path_from_archive(&pia).consume_path();
        assert_eq!(dir, out);
    }

    #[test]
    fn paths_rel() {
        let dir = PathBuf::from(".");
        let pta = path_to_archive(&dir);
        let out = path_from_archive(&PathBuf::from(&pta)).consume_path();
        assert_eq!(dir, out);

        let tmp: Vec<u8> = vec![];
        let mut tar = Builder::new(tmp);
        let mut header = Header::new_gnu();
        header.set_size(2);
        tar.append_data(&mut header, pta, "ab".as_bytes()).unwrap();
        let tmp = tar.into_inner().unwrap();
        let mut tar = Archive::new(Cursor::new(tmp));
        let entry = tar.entries().unwrap().next().unwrap().unwrap();
        let pia = entry.header().path().unwrap();
        let out = path_from_archive(&pia).consume_path();
        assert_eq!(dir, out);
    }
}
