use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::{fmt, num::ParseIntError};
use toml;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(required(true), index(1))]
    command: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FileEntry {
    path: String,
    hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeHexError {
    OddLength,
    ParseInt(ParseIntError),
}

impl From<ParseIntError> for DecodeHexError {
    fn from(e: ParseIntError) -> Self {
        DecodeHexError::ParseInt(e)
    }
}

impl fmt::Display for DecodeHexError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DecodeHexError::OddLength => "input string has an odd number of bytes".fmt(f),
            DecodeHexError::ParseInt(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for DecodeHexError {}

fn get_config_directory() -> String {
    "./".to_string()
}

fn get_config_path(config_name: String) -> PathBuf {
    Path::new(&get_config_directory()).join(config_name)
}

fn read_config(path: PathBuf) -> Result<toml::Table, ()> {
    match fs::read_to_string(path) {
        Ok(c) => Ok(toml::from_str(&c).expect("Not a valid toml file")),
        Err(_) => Err(()),
    }
}

fn verify_file(file: FileEntry) -> Result<(), String> {
    let saved_hash = decode_hex(&file.hash).unwrap();

    let mut file = File::open(file.path).expect("Unable to open file");
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).expect("Unable to read file");
    let hash: Vec<u8> = hasher.finalize().to_vec();

    if saved_hash != hash {
        return Err("Hashes don't match".to_string());
    }
    Ok(())
}

fn scan_directory(path: String) -> Vec<FileEntry> {
    let mut entries: Vec<FileEntry> = Vec::new();
    let mut dir_queue: VecDeque<String> = VecDeque::new();
    dir_queue.push_back(path);

    while let Some(entry) = dir_queue.pop_front() {
        for e in fs::read_dir(entry).expect("Unable to open folder {}") {
            let item = e.expect("Unable to open entry");
            if item.path().is_dir() {
                dir_queue.push_back(
                    item.path()
                        .into_os_string()
                        .into_string()
                        .expect("Unable to convert path to string"),
                );
            } else {
                let mut file = File::open(item.path()).expect("Unable to open file");
                let mut hasher = Sha256::new();
                io::copy(&mut file, &mut hasher).expect("Unable to read file");
                let hash = hasher.finalize();

                print!(".");
                io::stdout().flush().unwrap();

                entries.push(FileEntry {
                    path: item.path().into_os_string().into_string().unwrap(),
                    hash: format!("{:x}", hash).to_string(),
                })
            }
        }
    }
    entries
}

pub fn decode_hex(s: &str) -> Result<Vec<u8>, DecodeHexError> {
    if s.len() % 2 != 0 {
        Err(DecodeHexError::OddLength)
    } else {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.into()))
            .collect()
    }
}

fn main() {
    let args = Cli::parse();
    let config_path = get_config_path("vboot.toml".to_string());
    let config = read_config(config_path);

    // Setup conig settings
    let mut boot_path: String = "/boot".to_string();

    if let Ok(c) = config {
        if let Some(v) = c.get("boot_path") {
            boot_path = v
                .as_str()
                .expect("Unable to parse boot_path value from config")
                .to_string();
        }
    }

    match args.command.as_str() {
        "scan" => {
            let entries = scan_directory(boot_path);
            let datafile_path = get_config_path("filelist.json".to_string());

            let content = serde_json::to_string(&entries).expect("Unable to serde struct to JSON");
            let mut f = fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(&datafile_path)
                .expect("Unable to open output file");
            f.write_all(&content.as_bytes())
                .expect("Unable to write content to file.");
            f.flush().expect("Unable to flush content to file");

            println!(
                "\nSaved file {}",
                datafile_path.into_os_string().into_string().unwrap()
            );
        }
        "verify" => {
            let datafile_path = get_config_path("filelist.json".to_string());
            let content = fs::read_to_string(datafile_path).expect("Unable to read file");
            let entries: Vec<FileEntry> =
                serde_json::from_str(&content).expect("Not a valid json file");

            for entry in entries {
                if let Err(_) = verify_file(entry.clone()) {
                    panic!("File {} does not match!", entry.path);
                }
            }
            println!("All files verified correctly");
        }
        _ => panic!("Unrecognized command {}", args.command),
    }
}
