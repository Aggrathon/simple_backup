/// This module creates a library of the program (for testing)
#[macro_use]
pub mod utils;
pub mod backup;
pub mod cli;
pub mod compression;
pub mod config;
pub mod files;
pub mod gui;
pub mod parse_date;

#[allow(unused_imports)]
use crate::backup::BackupReader;
#[allow(unused_imports)]
use crate::backup::BackupWriter;
#[allow(unused_imports)]
use crate::config::Config;
