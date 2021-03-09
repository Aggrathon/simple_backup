// This file contains integration tests for backups and restoring

use std::fs::{remove_file, File};

use tempfile::tempdir;

// extern crate simple_backup;

use simple_backup::{self, backup::BackupWriter};
use simple_backup::{backup::BackupReader, config::Config};

#[test]
fn absolute() {
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("a.txt");
    let f2 = dir.path().join("b.txt");
    let f3 = dir.path().join("c.txt");
    let f4 = dir.path().join("d.txt");
    File::create(&f1).unwrap();
    File::create(&f2).unwrap();
    File::create(&f3).unwrap();
    File::create(&f4).unwrap();

    let config = Config {
        include: vec![dir.path().to_string_lossy().to_string()],
        exclude: vec![],
        regex: vec![],
        output: dir.path().to_string_lossy().to_string(),
        name: "backup".to_string(),
        verbose: false,
        force: false,
        incremental: true,
        quality: 11,
        local: false,
        time: None,
        origin: None,
    };
    let mut bw1 = BackupWriter::new(config).0;
    bw1.write(|_, _| ()).unwrap();

    let f5 = dir.path().join("e.txt");
    let f6 = dir.path().join("f.txt");
    File::create(&f5).unwrap();
    File::create(&f6).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(1));
    let mut bw2 = BackupWriter::new(bw1.config).0;
    bw2.write(|_, _| ()).unwrap();

    remove_file(&f2).unwrap();
    remove_file(&f5).unwrap();
    assert!(!f2.exists());
    assert!(!f5.exists());

    let mut br1 = BackupReader::from_config(bw2.config).unwrap();
    let mut br2 = br1.get_previous().unwrap().unwrap();

    br1.restore_these(|fi| fi, |_| (), false).unwrap();
    assert!(!f2.exists());
    assert!(f5.exists());

    remove_file(&f5).unwrap();
    assert!(!f5.exists());

    br2.restore_all(|fi| fi, |_| (), false).unwrap();
    assert!(f2.exists());
    assert!(!f5.exists());

    remove_file(&f2).unwrap();
    assert!(!f2.exists());

    br1.restore_all(|fi| fi, |_| (), false).unwrap();
    assert!(f2.exists());
    assert!(f5.exists());
}
