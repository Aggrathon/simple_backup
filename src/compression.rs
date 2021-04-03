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

    pub fn close(self) -> std::io::Result<()> {
        self.0.into_inner()?.finish()?.sync_all()?;
        Ok(())
    }

    pub fn append_file(&mut self, file: &PathBuf) -> std::io::Result<()> {
        let name = path_to_archive(&file);
        self.0.append_path_with_name(&file, name)
    }

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

pub type CompressionDecoderEntry<'dummy, 'a> = Entry<'dummy, Decoder<'a, BufReader<File>>>;
pub struct CompressionDecoder<'a>(Archive<Decoder<'a, BufReader<File>>>);

impl<'a> Debug for CompressionDecoder<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompressionDecoder").finish()
    }
}

impl<'a> CompressionDecoder<'a> {
    pub fn read<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = File::open(&path)?;
        let decoder = Decoder::new(file)?;
        let mut archive = Archive::new(decoder);
        archive.set_unpack_xattrs(true);
        archive.set_preserve_permissions(true);
        archive.set_overwrite(true);
        Ok(Self { 0: archive })
    }

    pub fn entries(
        &mut self,
    ) -> std::io::Result<
        impl Iterator<Item = std::io::Result<(FileInfo, CompressionDecoderEntry<'_, 'a>)>>,
    > {
        Ok(self.0.entries()?.map(|entry| {
            let entry = entry?;
            let path = entry.header().path()?;
            Ok((path_from_archive(&path), entry))
        }))
    }
}

#[cfg(target_os = "windows")]
fn path_to_archive(path: &PathBuf) -> String {
    archive_path_offset(if path.has_root() {
        "abs".to_string() + &path.to_string_lossy().replace('\\', "/")
    } else {
        "rel/".to_string() + &path.clean().to_string_lossy().replace('\\', "/")
    })
}

/// If the 100th byte is not a complete unicode character, add '_' to the beginning until it is.
/// This is a workaround for the naive behaviour for long filenames in the `tar` crate.
#[inline(always)]
fn archive_path_offset(path: String) -> String {
    if path.as_bytes().len() > 100 {
        // Check if the following byte is the start of a valid unicode character
        if (path.as_bytes()[100] >> 7) == 0 || (path.as_bytes()[100] >> 6) == 3 {
            path
        } else if (path.as_bytes()[99] >> 7) == 0 || (path.as_bytes()[99] >> 6) == 3 {
            "_".to_string() + &path
        } else if (path.as_bytes()[98] >> 7) == 0 || (path.as_bytes()[98] >> 6) == 3 {
            "__".to_string() + &path
        } else if (path.as_bytes()[97] >> 7) == 0 || (path.as_bytes()[98] >> 6) == 3 {
            "___".to_string() + &path
        } else {
            // At this point the string is not valid unicode!
            path
        }
    } else {
        path
    }
}

#[cfg(not(target_os = "windows"))]
fn path_to_archive(path: &PathBuf) -> String {
    archive_path_offset(if path.has_root() {
        "abs".to_string() + &path.to_string_lossy()
    } else {
        "rel/".to_string() + &path.clean().to_string_lossy()
    })
}

fn path_from_archive<P: AsRef<Path>>(path: P) -> FileInfo {
    let path = path.as_ref();
    let string = path.to_string_lossy();
    // skip the '_' added by `archive_path_offset
    let start = if string.as_bytes().len() > 100 {
        string
            .as_bytes()
            .iter()
            .take(3)
            .position(|b| *b != b'_')
            .unwrap_or(0)
    } else {
        0
    };
    if string[start..].starts_with("rel/") {
        FileInfo::from(string[(start + 4)..].to_string())
    } else if string[start..].starts_with("abs") {
        FileInfo::from(string[(start + 3)..].to_string())
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

    #[test]
    fn unicode_split() {
        let string = "aåååååååååååååååååååååååååååååååååååååååååååååååååååååååååååå";
        let new = path_to_archive(&PathBuf::from(string));
        std::str::from_utf8(&string.as_bytes()[..100])
            .expect_err("the original string should split a unicode character at byte 100");
        std::str::from_utf8(&new.as_bytes()[..100])
            .expect("the new string should be offset not to split a unicode character at byte 100");
        assert_eq!(b'_', new.as_bytes()[0]);
        assert_eq!(string, path_from_archive(new).get_string().as_str());
    }
}
