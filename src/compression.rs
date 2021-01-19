use brotli::{CompressorWriter, Decompressor};
use path_clean::PathClean;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use tar::{Archive, Builder, Entries, Entry, Header};

pub struct CompressionEncoder {
    archive: Builder<CompressorWriter<File>>,
}

impl CompressionEncoder {
    pub fn create(path: &PathBuf, quality: u32) -> Self {
        let file = File::create(path).expect("Could not create file");
        let encoder = CompressorWriter::new(file, 16384, quality, 23);
        let archive = Builder::new(encoder);
        CompressionEncoder { archive }
    }

    pub fn close(self) {
        self.archive
            .into_inner()
            .expect("Could not create the archive")
            .into_inner()
            .sync_all()
            .expect("Could not close the backup file");
    }

    pub fn append_file(&mut self, file: &PathBuf) {
        let name = path_to_archive(&file);
        if let Err(e) = self.archive.append_path_with_name(&file, name) {
            eprintln!(
                "Could not add '{}' to archive: {}",
                file.to_string_lossy(),
                e
            );
        }
    }

    pub fn append_data(&mut self, name: &str, content: &str) {
        let mut header = Header::new_gnu();
        header.set_size(content.len() as u64);
        if let Err(e) = self
            .archive
            .append_data(&mut header, name, content.as_bytes())
        {
            eprintln!("Could not add '{}' to archive: {}", name, e);
        }
    }
}

pub struct CompressionDecoder {
    archive: Archive<Decompressor<File>>,
}

impl CompressionDecoder {
    pub fn read(path: &PathBuf) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let decoder = brotli::Decompressor::new(file, 16384);
        let mut archive = Archive::new(decoder);
        archive.set_unpack_xattrs(true);
        Ok(Self { archive })
    }

    pub fn entries(&mut self) -> Result<CompressionEntries, std::io::Error> {
        Ok(CompressionEntries {
            entries: self.archive.entries()?,
        })
    }
}

pub struct CompressionEntries<'a> {
    entries: Entries<'a, Decompressor<File>>,
}

impl<'a> Iterator for CompressionEntries<'a> {
    type Item = Result<(PathBuf, Entry<'a, Decompressor<File>>), std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.entries.next()?;
        if let Err(e) = entry {
            return Some(Err(e));
        }
        let entry = entry.unwrap();
        let path = entry.header().path();
        if let Err(e) = path {
            return Some(Err(e));
        }
        Some(Ok((path_from_archive(&path.unwrap()), entry)))
    }
}

fn path_to_archive(path: &PathBuf) -> String {
    if path.is_relative() {
        "rel/".to_string() + &path.clean().to_string_lossy()
    } else {
        "abs/".to_string() + &path.to_string_lossy()
    }
}

fn path_from_archive(path: &Path) -> PathBuf {
    let path = path.to_string_lossy();
    if path.starts_with("rel/") {
        PathBuf::from(&path[4..])
    } else if path.starts_with("abs/") {
        PathBuf::from(&path[4..])
    } else {
        PathBuf::from(&path[0..])
    }
}

#[cfg(test)]
mod tests {
    use path_absolutize::Absolutize;
    use std::path::PathBuf;

    use super::{path_from_archive, path_to_archive};

    #[test]
    fn paths() {
        let dir = PathBuf::from(".").absolutize().unwrap().to_path_buf();
        let pia = path_to_archive(&dir);
        let out = path_from_archive(&PathBuf::from(pia));
        assert_eq!(dir, out);
    }
}
