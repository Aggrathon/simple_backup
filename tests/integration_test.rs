// This file contains integration tests for backups and restoring

use std::fs::{remove_file, File};

use simple_backup::{
    self,
    backup::{BackupReader, BackupWriter},
    cli::{backup, restore},
    config::Config,
};
use tempfile::tempdir;

#[test]
fn cli_test() {
    let dir = tempdir().unwrap();
    let dir2 = dir.path().join("dir");
    let dir3 = dir.path().join("backup");
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
        output: dir3.to_string_lossy().to_string(),
        incremental: true,
        quality: 11,
        threads: 1,
        local: false,
        time: None,
        origin: None,
    };
    let mut bw1 = BackupWriter::new(config).0;
    bw1.write(|_| (), |_, _| ()).unwrap();

    remove_file(&f1).unwrap();
    remove_file(&f2).unwrap();
    remove_file(&f3).unwrap();
    remove_file(&f4).unwrap();

    let conf = Config::from_yaml(&mut bw1.config.to_yaml().unwrap()).unwrap();
    restore(
        BackupReader::from_config(conf).unwrap(),
        &dir.path().to_string_lossy(),
        vec![&f1.to_string_lossy()],
        vec![],
        false,
        false,
        false,
        false,
    );

    assert!(f1.exists());
    assert!(!f2.exists());
    assert!(!f3.exists());
    assert!(!f4.exists());

    let conf = Config::from_yaml(&mut bw1.config.to_yaml().unwrap()).unwrap();
    restore(
        BackupReader::from_config(conf).unwrap(),
        &dir.path().to_string_lossy(),
        vec![],
        vec![&f2.to_string_lossy().replace('\\', "/")],
        false,
        true,
        false,
        false,
    );

    assert!(f1.exists());
    assert!(!f2.exists());
    assert!(f3.exists());
    assert!(f4.exists());

    let conf = Config::from_yaml(&mut bw1.config.to_yaml().unwrap()).unwrap();
    restore(
        BackupReader::from_config(conf).unwrap(),
        &dir2.to_string_lossy(),
        vec![],
        vec![],
        true,
        true,
        false,
        false,
    );

    assert!(dir2.join("a.txt").exists());
    assert!(dir2.join("b.txt").exists());
    assert!(dir2.join("c.txt").exists());
    assert!(dir2.join("d.txt").exists());
}

#[test]
fn absolute_test() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("a.txt");
    let f2 = dir.path().join("b.txt");
    let f3 = dir.path().join("c.txt");
    let f4 = dir.path().join("d.txt");
    File::create(&f1)?;
    File::create(&f2)?;
    File::create(&f3)?;
    File::create(&f4)?;

    let config = Config {
        include: vec![dir.path().to_string_lossy().to_string()],
        exclude: vec![],
        regex: vec![],
        output: dir.path().to_string_lossy().to_string(),
        incremental: true,
        quality: 11,
        local: false,
        threads: 1,
        time: None,
        origin: None,
    };
    let mut bw1 = BackupWriter::new(config).0;
    bw1.write(|_| (), |_, _| ())?;

    let f5 = dir.path().join("e.txt");
    let f6 = dir.path().join("f.txt");
    File::create(&f5)?;
    File::create(&f6)?;

    std::thread::sleep(std::time::Duration::from_secs(1));
    let mut bw2 = BackupWriter::new(bw1.config).0;
    bw2.write(|_| (), |_, _| ())?;

    remove_file(&f2)?;
    remove_file(&f5)?;
    assert!(!f2.exists());
    assert!(!f5.exists());

    let mut br1 = BackupReader::from_config(bw2.config)?;
    let mut br2 = br1.get_previous()?.unwrap();

    br1.restore_these(|fi| fi, |_| (), false)?;
    assert!(!f2.exists());
    assert!(f5.exists());

    remove_file(&f5)?;
    assert!(!f5.exists());

    br2.restore_all(|fi| fi, |_| (), false)?;
    assert!(f2.exists());
    assert!(!f5.exists());

    remove_file(&f2)?;
    assert!(!f2.exists());

    br1.restore_all(|fi| fi, |_| (), true)?;
    assert!(f2.exists());
    assert!(f5.exists());

    Ok(())
}

#[test]
fn local_test() {
    let dir = tempdir().unwrap();

    let mut config = Config {
        include: vec![".".to_string()],
        exclude: vec!["target".to_string(), ".git".to_string(), "src".to_string()],
        regex: vec![".*.md".to_string()],
        output: dir.path().to_string_lossy().to_string(),
        incremental: false,
        quality: 11,
        local: true,
        threads: 1,
        time: None,
        origin: None,
    };

    let conf = Config::from_yaml(config.to_yaml().unwrap()).unwrap();
    backup(conf, false, false, false);

    restore(
        BackupReader::from_config(config).unwrap(),
        &dir.path().to_string_lossy(),
        vec![],
        vec![],
        false,
        false,
        false,
        false,
    );

    assert!(dir.path().join("Cargo.toml").exists());
    assert!(!dir.path().join(".target").exists());
    assert!(!dir.path().join(".git").exists());
    assert!(!dir.path().join("README.md").exists());
}
