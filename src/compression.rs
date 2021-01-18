use brotli::CompressorWriter;
use path_clean::PathClean;
use std::{fs::File, path::PathBuf};
use tar::{Builder, Header};

pub struct Compression {
    archive: Builder<CompressorWriter<File>>,
}

impl Compression {
    pub fn create(path: &PathBuf, quality: u32) -> Self {
        let file = File::create(path).expect("Could not create file");
        let encoder = CompressorWriter::new(file, 16384, quality, 24);
        let archive = Builder::new(encoder);
        Compression { archive }
    }

    pub fn finish(self) {
        self.archive
            .into_inner()
            .expect("Could not create the archive")
            .into_inner()
            .sync_all()
            .expect("Could not close the backup file");
    }

    pub fn append_file(&mut self, file: &PathBuf) {
        let name = path_to_archive(&file);
        self.archive
            .append_path_with_name(file, name)
            .expect("Could not add file to archive");
    }

    pub fn append_data(&mut self, name: &str, content: &str) {
        let mut header = Header::new_gnu();
        header.set_size(content.len() as u64);
        self.archive
            .append_data(&mut header, name, content.as_bytes())
            .expect("Could not add data to archive");
    }
}

fn path_to_archive(path: &PathBuf) -> String {
    if path.is_relative() {
        "rel/".to_string() + &path.clean().to_string_lossy()
    } else {
        "abs/".to_string() + &path.to_string_lossy()
    }
}

#[allow(dead_code)]
fn path_from_archive(path: &String) -> PathBuf {
    if path.starts_with("rel/") {
        PathBuf::from(&path[4..])
    } else if path.starts_with("abs/") {
        PathBuf::from(&path[4..])
    } else {
        PathBuf::from(&path)
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
        let out = path_from_archive(&pia);
        assert_eq!(dir, out);
    }
}
