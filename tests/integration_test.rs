// This file contains integration tests for backups and restoring

use std::fs::{remove_file, File};
use std::path::PathBuf;

use path_absolutize::Absolutize;
use simple_backup::backup::{BackupReader, BackupWriter};
use simple_backup::cli::{backup, inspect, merge, restore};
use simple_backup::config::Config;
use simple_backup::parse_date::naive_now;
use simple_backup::utils::{extend_pathbuf, get_backup_from_path, strip_absolute_from_path};
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
        origin: PathBuf::new(),
    };
    let mut bw1 = BackupWriter::new(config).0;
    bw1.export_list(&f4, false).unwrap();
    bw1.export_list(&f3, true).unwrap();
    bw1.write(|_, _| Ok(()), || ()).unwrap();

    remove_file(&f1).unwrap();
    remove_file(&f2).unwrap();
    remove_file(&f3).unwrap();
    remove_file(&f4).unwrap();

    let conf = Config::from_yaml(bw1.config.as_yaml().unwrap()).unwrap();
    let mut reader = BackupReader::from_config(conf).unwrap();
    reader.get_config().unwrap();
    reader.get_list().unwrap();
    let _ = reader.get_meta().unwrap();
    reader.export_list(dir.path().join("files.txt")).unwrap();
    restore::<PathBuf>(
        reader,
        None,
        vec![f1.to_string_lossy().to_string()],
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

    let conf = Config::from_yaml(bw1.config.as_yaml().unwrap()).unwrap();
    restore::<PathBuf>(
        BackupReader::from_config(conf).unwrap(),
        None,
        vec![],
        vec![f2.to_string_lossy().replace('\\', "/")],
        false,
        true,
        true,
        true,
        false,
        true,
    );

    assert!(f1.exists());
    assert!(f2.exists());
    assert!(!f3.exists());
    assert!(!f4.exists());

    let conf = Config::from_yaml(bw1.config.as_yaml().unwrap()).unwrap();
    restore(
        BackupReader::from_config(conf).unwrap(),
        Some(&dir2),
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
        regex: vec!["zst$".to_string()],
        output: dir.path().to_path_buf(),
        incremental: true,
        quality: 11,
        local: false,
        threads: 1,
        time: None,
        origin: PathBuf::new(),
    };
    let mut bw1 = BackupWriter::new(config).0;
    bw1.write(|_, _| Ok(()), || ())?;

    let f5 = dir.path().join("e.txt");
    let f6 = dir.path().join("f.txt");
    File::create(&f5)?;
    File::create(&f6)?;
    std::thread::sleep(std::time::Duration::from_millis(20));

    let mut bw2 = BackupWriter::new(bw1.config).0;
    bw2.path = dir.path().join("b2.tar.zst");
    bw2.write(|_, _| Ok(()), || ())?;

    remove_file(&f2)?;
    remove_file(&f5)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(!f2.exists());
    assert!(!f5.exists());

    let mut br2 = BackupReader::from_config(bw2.config)?;
    let mut br1 = br2.get_previous()?.unwrap();

    br2.restore_this(|fi| fi, |_| Ok(()), false)?;
    assert!(!f2.exists());
    assert!(f5.exists());

    remove_file(&f5)?;
    assert!(!f5.exists());

    br1.restore_this(|fi| fi, |_| Ok(()), false)?;
    assert!(f2.exists());
    assert!(!f5.exists());

    remove_file(&f2)?;
    assert!(!f2.exists());

    br2.restore_this(|fi| fi, |_| Ok(()), true)?;
    assert!(!f2.exists());
    assert!(f5.exists());

    br2.restore_all(|fi| fi, |_| Ok(()), false)?;
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
        origin: PathBuf::new(),
    };

    let conf = Config::from_yaml(config.as_yaml()?)?;
    backup(conf, false, false, false, true);

    let reader = BackupReader::from_config(config)?;
    restore(
        reader,
        Some(dir.path()),
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
fn flatten_test() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir().unwrap();

    let config = Config {
        include: vec![
            "./src/lib.rs".to_string(),
            PathBuf::from("./src/cli.rs")
                .absolutize()?
                .to_string_lossy()
                .to_string(),
        ],
        exclude: vec![],
        regex: vec![],
        output: dir.path().to_path_buf(),
        incremental: false,
        quality: 11,
        local: true,
        threads: 1,
        time: None,
        origin: PathBuf::new(),
    };
    backup(config.clone(), false, false, false, true);

    let reader = BackupReader::from_config(config)?;
    restore(
        reader,
        Some(&dir.path()),
        vec![],
        vec![],
        true,
        false,
        false,
        false,
        false,
        true,
    );

    assert!(dir.path().join("cli.rs").exists());
    assert!(dir.path().join("lib.rs").exists());
    Ok(())
}

#[test]
fn extract_test() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir().unwrap();

    let inc = vec![
        "./src/lib.rs".to_string(),
        PathBuf::from("./src/cli.rs")
            .absolutize()?
            .to_string_lossy()
            .to_string(),
    ];
    let mut config = Config {
        include: inc.clone(),
        exclude: vec![],
        regex: vec![],
        output: dir.path().to_path_buf(),
        incremental: false,
        quality: 11,
        local: true,
        threads: 1,
        time: None,
        origin: PathBuf::new(),
    };
    backup(config.clone(), false, false, false, true);

    let reader = BackupReader::from_config(config.clone())?;
    inspect(reader.clone(), false, false, true);
    inspect(reader.clone(), false, true, true);
    inspect(reader.clone(), true, false, true);
    inspect(reader.clone(), true, true, true);
    restore(
        reader.clone(),
        Some(&dir.path()),
        vec![],
        vec![],
        false,
        false,
        false,
        false,
        false,
        true,
    );

    for p in inc.iter() {
        assert!(dir.path().join(strip_absolute_from_path(p)).exists());
    }

    let dir = dir.path().join("tmp");
    config.output = dir.clone();
    backup(config, false, false, false, true);
    restore(
        reader,
        Some(&dir),
        vec![],
        vec!["src".to_string()],
        false,
        false,
        false,
        false,
        false,
        true,
    );
    for p in inc.iter() {
        assert!(dir.join(strip_absolute_from_path(p)).exists());
    }

    Ok(())
}

#[test]
fn time_test() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let f1 = dir.path().join("a.txt");
    let f2 = dir.path().join("b.txt");
    let f3 = dir.path().join("c.txt");
    let f4 = dir.path().join("d.txt");
    File::create(&f1)?;
    File::create(&f2)?;
    std::thread::sleep(std::time::Duration::from_millis(20));

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
        origin: PathBuf::new(),
    };

    std::thread::sleep(std::time::Duration::from_millis(20));
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
        origin: PathBuf::new(),
    };

    restore::<PathBuf>(
        BackupReader::from_config(config)?,
        None,
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

#[test]
fn longname_test() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir().unwrap();
    let f1 = dir.path().join(format!("{:50}.txt", 3));
    File::create(&f1)?;

    let mut config = Config {
        include: vec![f1.to_string_lossy().to_string()],
        exclude: vec![],
        regex: vec![],
        output: dir.path().to_path_buf(),
        incremental: false,
        quality: 11,
        local: false,
        threads: 1,
        time: None,
        origin: PathBuf::new(),
    };

    let conf = Config::from_yaml(config.as_yaml()?)?;
    backup(conf, false, false, false, true);

    remove_file(&f1)?;

    let reader = BackupReader::from_config(config)?;
    restore::<PathBuf>(
        reader,
        None,
        vec![],
        vec![],
        false,
        false,
        false,
        false,
        false,
        true,
    );

    assert!(f1.exists());
    Ok(())
}

#[test]
fn merge_test() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let dir2 = tempdir()?;
    let f1 = dir.path().join("a.txt");
    let f2 = dir.path().join("b.txt");
    let f3 = dir.path().join("c.txt");
    let b1 = dir2.path().join("b1_2020-20-20_20-20-21.tar.zst");
    let b2 = dir2.path().join("b2_2020-20-20_20-20-22.tar.zst");
    let b3 = dir2.path().join("b3_2020-20-20_20-20-23.tar.zst");
    let b4 = dir2.path().join("b4_2020-20-20_20-20-24.tar.zst");

    let mut config = Config {
        include: vec![dir.path().to_string_lossy().to_string()],
        exclude: vec![],
        regex: vec![],
        output: b1.clone(),
        incremental: true,
        quality: 11,
        threads: 1,
        local: false,
        time: None,
        origin: PathBuf::new(),
    };

    File::create(&f1)?;

    backup(config.clone(), false, false, false, true);
    assert!(b1.exists());
    config.output = b2.clone();
    config.time = Some(naive_now());

    remove_file(&f1)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    File::create(&f2)?;

    backup(config.clone(), false, false, false, true);
    assert!(b2.exists());
    config.output = b3.clone();
    config.time = Some(naive_now());

    std::thread::sleep(std::time::Duration::from_millis(20));
    File::create(&f3)?;

    backup(config, false, false, false, true);
    assert!(b3.exists());

    remove_file(&f2)?;
    remove_file(&f3)?;

    merge(
        vec![b1.clone(), b2.clone()],
        None,
        true,
        true,
        Some(1),
        None,
        false,
        true,
        false,
        true,
    );

    assert_eq!(
        b3,
        *get_backup_from_path(dir2.path().to_path_buf())?
            .path
            .get_path()
    );

    merge(
        vec![dir2.path().to_path_buf()],
        Some(b4.clone()),
        false,
        false,
        None,
        Some(1),
        true,
        false,
        false,
        true,
    );

    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(!b1.exists());
    assert!(!b2.exists());
    assert!(!b3.exists());
    assert!(b4.exists());
    let b2 = extend_pathbuf(b2, ".old");
    let b3 = extend_pathbuf(b3, ".old");
    assert!(b2.exists());
    assert!(b3.exists());

    let mut reader = BackupReader::new(b4);
    #[cfg(target_os = "windows")]
    assert_eq!(
        reader.get_list()?.iter_included().collect::<Vec<_>>(),
        vec![
            f2.to_string_lossy().replace('\\', "/"),
            f3.to_string_lossy().replace('\\', "/")
        ]
    );
    #[cfg(not(target_os = "windows"))]
    assert_eq!(
        reader.get_list()?.iter_included().collect::<Vec<_>>(),
        vec![f2.to_string_lossy(), f3.to_string_lossy()]
    );

    restore::<PathBuf>(
        reader,
        None,
        vec![],
        vec![],
        false,
        true,
        false,
        true,
        false,
        true,
    );
    assert!(!f1.exists());
    assert!(f2.exists());
    assert!(f3.exists());

    restore::<PathBuf>(
        BackupReader::new(b2),
        None,
        vec![],
        vec![],
        false,
        true,
        false,
        false,
        false,
        true,
    );
    assert!(f1.exists());
    assert!(f2.exists());
    assert!(f3.exists());
    Ok(())
}
