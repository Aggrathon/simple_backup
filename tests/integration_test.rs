// This file contains integration tests for backups and restoring

use std::fs::{remove_file, File};

use simple_backup;
use simple_backup::backup::{BackupReader, BackupWriter};
use simple_backup::cli::{backup, restore};
use simple_backup::config::Config;
use simple_backup::parse_date::naive_now;
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
        output: dir3,
        incremental: true,
        quality: 11,
        threads: 1,
        local: false,
        time: None,
        origin: None,
    };
    let mut bw1 = BackupWriter::new(config).0;
    bw1.export_list(&f4, false).unwrap();
    bw1.export_list(&f3, true).unwrap();
    bw1.write(|_| (), |_, _| (), || ()).unwrap();

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
        false,
        true,
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
        true,
        false,
        false,
        true,
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
        true,
        false,
        false,
        true,
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
        output: dir.path().to_path_buf(),
        incremental: true,
        quality: 11,
        local: false,
        threads: 1,
        time: None,
        origin: None,
    };
    let mut bw1 = BackupWriter::new(config).0;
    bw1.write(|_| (), |_, _| (), || ())?;

    let f5 = dir.path().join("e.txt");
    let f6 = dir.path().join("f.txt");
    File::create(&f5)?;
    File::create(&f6)?;

    std::thread::sleep(std::time::Duration::from_secs(1));
    let mut bw2 = BackupWriter::new(bw1.config).0;
    bw2.write(|_| (), |_, _| (), || ())?;

    remove_file(&f2)?;
    remove_file(&f5)?;
    assert!(!f2.exists());
    assert!(!f5.exists());

    let mut br1 = BackupReader::from_config(bw2.config)?;
    let mut br2 = br1.get_previous()?.unwrap();

    br1.restore_this(|fi| fi, |_| (), false)?;
    assert!(!f2.exists());
    assert!(f5.exists());

    remove_file(&f5)?;
    assert!(!f5.exists());

    br2.restore_this(|fi| fi, |_| (), false)?;
    assert!(f2.exists());
    assert!(!f5.exists());

    remove_file(&f2)?;
    assert!(!f2.exists());

    br1.restore_this(|fi| fi, |_| (), true)?;
    assert!(!f2.exists());
    assert!(f5.exists());

    br1.restore_all(|fi| fi, |_| (), false)?;
    assert!(f2.exists());

    Ok(())
}

#[test]
fn local_test() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir().unwrap();

    let mut config = Config {
        include: vec![".".to_string()],
        exclude: vec!["target".to_string(), ".git".to_string(), "src".to_string()],
        regex: vec![".*.md".to_string()],
        output: dir.path().to_path_buf(),
        incremental: false,
        quality: 11,
        local: true,
        threads: 1,
        time: None,
        origin: None,
    };

    let conf = Config::from_yaml(config.to_yaml()?)?;
    backup(conf, false, false, false, true);

    let reader = BackupReader::from_config(config)?;
    restore(
        reader,
        &dir.path().to_string_lossy(),
        vec![],
        vec![],
        false,
        false,
        false,
        false,
        false,
        true,
    );

    assert!(dir.path().join("Cargo.toml").exists());
    assert!(!dir.path().join(".target").exists());
    assert!(!dir.path().join(".git").exists());
    assert!(!dir.path().join("README.md").exists());
    Ok(())
}

#[test]
fn time_test() -> std::io::Result<()> {
    let dir = tempdir()?;
    let f1 = dir.path().join("a.txt");
    let f2 = dir.path().join("b.txt");
    let f3 = dir.path().join("c.txt");
    let f4 = dir.path().join("d.txt");
    File::create(&f1)?;
    File::create(&f2)?;
    std::thread::sleep(std::time::Duration::from_millis(100));

    let config = Config {
        include: vec![dir.path().to_string_lossy().to_string()],
        exclude: vec![],
        regex: vec![],
        output: dir.path().to_path_buf(),
        incremental: true,
        quality: 11,
        threads: 1,
        local: false,
        time: Some(naive_now()),
        origin: None,
    };

    std::thread::sleep(std::time::Duration::from_millis(100));
    File::create(&f3)?;
    File::create(&f4)?;

    backup(config, false, false, false, true);

    remove_file(&f1)?;
    remove_file(&f2)?;
    remove_file(&f3)?;
    remove_file(&f4)?;

    let config = Config {
        include: vec![dir.path().to_string_lossy().to_string()],
        exclude: vec![],
        regex: vec![],
        output: dir.path().to_path_buf(),
        incremental: true,
        quality: 11,
        threads: 1,
        local: false,
        time: Some(naive_now()),
        origin: None,
    };

    restore(
        BackupReader::from_config(config)?,
        &dir.path().to_string_lossy(),
        vec![],
        vec![],
        false,
        false,
        false,
        false,
        false,
        true,
    );

    assert!(!f1.exists());
    assert!(!f2.exists());
    assert!(f3.exists());
    assert!(f4.exists());

    Ok(())
}
