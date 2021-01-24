use std::{
    fs::File,
    path::{Path, PathBuf},
};

use brotli::{CompressorWriter, Decompressor};
use path_clean::PathClean;
use tar::{Archive, Builder, Entry, Header};

pub struct CompressionEncoder {
    archive: Builder<CompressorWriter<File>>,
}

impl CompressionEncoder {
    pub fn create<P: AsRef<Path>>(path: P, quality: u32) -> std::io::Result<Self> {
        let file = File::create(path)?;
        let encoder = CompressorWriter::new(file, 16384, quality, 23);
        let archive = Builder::new(encoder);
        Ok(CompressionEncoder { archive })
    }

    pub fn close(self) -> std::io::Result<()> {
        self.archive.into_inner()?.into_inner().sync_all()?;
        Ok(())
    }

    pub fn append_file(&mut self, file: &PathBuf) -> std::io::Result<()> {
        let name = path_to_archive(&file);
        self.archive.append_path_with_name(&file, name)?;
        Ok(())
    }

    pub fn append_data<P: AsRef<Path>, B: AsRef<[u8]>>(
        &mut self,
        name: P,
        content: B,
    ) -> std::io::Result<()> {
        let content = content.as_ref();
        let mut header = Header::new_gnu();
        header.set_size(content.len() as u64);
        self.archive.append_data(&mut header, &name, content)?;
        Ok(())
    }
}

pub struct CompressionDecoder {
    pub archive: Archive<Decompressor<File>>,
    pub path: PathBuf,
}

impl CompressionDecoder {
    pub fn read<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = File::open(&path)?;
        let decoder = brotli::Decompressor::new(file, 16384);
        let mut archive = Archive::new(decoder);
        archive.set_unpack_xattrs(true);
        Ok(Self {
            archive,
            path: path.as_ref().to_path_buf(),
        })
    }

    pub fn entries(
        &mut self,
    ) -> std::io::Result<impl Iterator<Item = std::io::Result<(PathBuf, Entry<Decompressor<File>>)>>>
    {
        Ok(self.archive.entries()?.map(|entry| {
            let entry = entry?;
            let path = entry.header().path()?;
            Ok((path_from_archive(&path), entry))
        }))
    }
}

fn path_to_archive(path: &PathBuf) -> String {
    if path.has_root() {
        "abs".to_string() + &path.to_string_lossy()
    } else {
        "rel/".to_string() + &path.clean().to_string_lossy()
    }
}

fn path_from_archive<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    let string = path.to_string_lossy();
    if string.starts_with("rel/") {
        PathBuf::from(&string[4..])
    } else if string.starts_with("abs") {
        PathBuf::from(&string[3..])
    } else if string == "rel" {
        PathBuf::from(".")
    } else {
        PathBuf::from(path)
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
        let out = path_from_archive(&PathBuf::from(&pta));
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
        let out = path_from_archive(&pia);
        assert_eq!(dir, out);
    }

    #[test]
    fn paths_rel() {
        let dir = PathBuf::from(".");
        let pta = path_to_archive(&dir);
        let out = path_from_archive(&PathBuf::from(&pta));
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
        let out = path_from_archive(&pia);
        assert_eq!(dir, out);
    }
}
