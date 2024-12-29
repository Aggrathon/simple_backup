# simple_backup

A minimal application for doing backups.

## Features

- [X] No propriety formats, the backups are saved in a normal compressed archives.
- [X] Modern, state-of-the-art compression using [zstd](https://www.zstd.net).
- [X] Optional multithreading for increased performance.
<!--></!-->
- [X] Incremental backups (using the "last changed" metadata from the files).
- [X] Selective restores (only deleted files, only selected files, or all files).
- [X] Merge old backups to save sapace.
<!--></!-->
- [X] Command line interface (declare includes, excludes, and regex-filters).
- [X] Configurations can be saved for easy reuse (e.g. for incremental backups).
- [X] Graphical user interface.

## Binaries

Precompiled binaries can be found on the [releases page](https://github.com/Aggrathon/simple_backup/releases/) (no installation required).


## Usage

### GUI

Just run the program and select what to do (assuming it has been compiled without disabling the GUI).

![screenshot](screenshot.avif)

### Command line

For doing backups from command line run `simple_backup direct [PARAMS]`. To create a config file run `simple_backup config <CONFIG> [PARAMS]` and then `simple_backup backup <CONFIG>` to do a backup based on the config. Finally, to restore from a backup run `simple_backup restore <PATH> [PARAMS]`. More detailed instructions is available with the help parameter: `simple_backup --help` (also works for sub commands such as `simple_backup restore --help`).

**Example:**

```{sh}
cd /tmp
mkdir dir
touch test1.txt dir/test2.txt dir/test3.txt

simple_backup config config.yml --include test.txt dir --exclude dir/test2.txt --output .
simple_backup backup config.yml

rm test1.txt dir/test2.txt dir/test3.txt dir -r

simple_backup restore config.yml

[ -f test1.txt ] && echo "test1.txt was restored."
[ ! -f dir/test2.txt ] && echo "test2.txt was excluded."
[ -f dir/test3.txt ] && echo "test3.txt was restored."

rm backup_*.tar.zst config.yml test1.txt dir/test3.txt dir -r
```

### Compilation

Download this repo and run: `cargo build --release`. To compile a version without the GUI add: `--no-default-features`.
