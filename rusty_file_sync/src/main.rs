use clap::{Arg, Command};
use log::{info, error, debug, LevelFilter};
use sha2::{Sha256, Digest};
use std::error::Error;
use std::path::{Path};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use walkdir::WalkDir;
use thiserror::Error;
use ctrlc;
use env_logger;
use tokio::fs;
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt};
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("Sync Tool")
        .version("1.0")
        .author("Edward Igarashi <info@igarashi.net>")
        .about("Synchronizes files and directories")
        .subcommand(Command::new("sync")
            .about("Synchronizes files between source and destination")
            .arg(Arg::new("source")
                .help("Source directory")
                .required(true)
                .index(1))
            .arg(Arg::new("destination")
                .help("Destination directory")
                .required(true)
                .index(2))
            .arg(Arg::new("mode")
                .help("Synchronization mode: one, bi, one+no_delete, bi+no_delete")
                .required(true)
                .index(3))
            .arg(Arg::new("debug")
                .help("Enable debug mode")
                .long("debug")
                .short('d')))
        .get_matches();

    let log_level = if matches.contains_id("debug") {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    env_logger::builder().filter_level(log_level).init();

    if let Some(matches) = matches.subcommand_matches("sync") {
        let source = matches.get_one::<String>("source").unwrap();
        let destination = matches.get_one::<String>("destination").unwrap();
        let mode = matches.get_one::<String>("mode").unwrap();

        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");

        let source = source.clone();
        let destination = destination.clone();
        let mode = mode.clone();

        let sync_thread_running = running.clone();
        tokio::spawn(async move {
            while sync_thread_running.load(Ordering::SeqCst) {
                let result = match mode.as_str() {
                    "one" => sync_oneway(&source, &destination, true).await,
                    "bi" => sync_bothways(&source, &destination, true).await,
                    "one+no_delete" => sync_oneway(&source, &destination, false).await,
                    "bi+no_delete" => sync_bothways(&source, &destination, false).await,
                    _ => {
                        println!("Invalid mode: {}", mode);
                        return;
                    }
                };

                if let Err(e) = result {
                    error!("Synchronization failed: {}", e);
                }

                tokio::time::sleep(Duration::from_secs(10)).await; // Sync interval
            }
        });

        // Optional: Handle 'q' to quit
        let stdin = io::BufReader::new(io::stdin());
        let mut lines = stdin.lines();
        while running.load(Ordering::SeqCst) {
            if let Some(line) = lines.next_line().await.unwrap_or(None) {
                if line == "q" {
                    running.store(false, Ordering::SeqCst);
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
enum SyncError {
    #[error("File system error: {0}")]
    FileSystemError(#[from] std::io::Error),
    #[error("Path error: {0}")]
    PathError(#[from] std::path::StripPrefixError),
    #[error("WalkDir error: {0}")]
    WalkDirError(#[from] walkdir::Error),
}

async fn calculate_hash<P: AsRef<Path>>(path: P) -> Result<String, SyncError> {
    let mut file = fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 1024];
    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

async fn is_file_updated(src_metadata: &std::fs::Metadata, dest_path: &Path) -> bool {
    if let Ok(dest_metadata) = fs::metadata(dest_path).await {
        if let Ok(dest_modified) = dest_metadata.modified() {
            if let Ok(src_modified) = src_metadata.modified() {
                if src_modified > dest_modified {
                    return true;
                } else {
                    if let (Ok(src_hash), Ok(dest_hash)) = (calculate_hash(dest_path).await, calculate_hash(dest_path).await) {
                        return src_hash != dest_hash;
                    }
                }
            }
        }
    }
    true
}

async fn sync_oneway(source: &str, destination: &str, delete: bool) -> Result<(), SyncError> {
    let mut dest_files = HashSet::new();

    if delete {
        for entry in WalkDir::new(destination) {
            let entry = entry?;
            let path = entry.path().strip_prefix(destination)?.to_path_buf();
            dest_files.insert(path);
        }
    }

    for entry in WalkDir::new(source) {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = Path::new(destination).join(source_path.strip_prefix(source)?);

        if delete {
            dest_files.remove(dest_path.strip_prefix(destination)?);
        }

        if source_path.is_dir() {
            if !dest_path.exists() {
                info!("Creating directory: {:?}", dest_path);
                fs::create_dir_all(&dest_path).await?;
            }
        } else {
            if !dest_path.exists() || is_file_updated(&std::fs::metadata(source_path)?, &dest_path).await {
                info!("Copying file from {:?} to {:?}", source_path, dest_path);
                fs::copy(&source_path, &dest_path).await?;
            } else {
                debug!("Skipping unchanged file: {:?}", source_path);
            }
        }
    }

    if delete {
        for remaining_path in dest_files {
            let full_dest_path = Path::new(destination).join(&remaining_path);
            if full_dest_path.is_dir() {
                info!("Removing directory: {:?}", full_dest_path);
                fs::remove_dir_all(full_dest_path).await?;
            } else {
                info!("Removing file: {:?}", full_dest_path);
                fs::remove_file(full_dest_path).await?;
            }
        }
    }

    Ok(())
}

async fn sync_bothways(source: &str, destination: &str, delete: bool) -> Result<(), SyncError> {
    sync_oneway(source, destination, delete).await?;
    sync_oneway(destination, source, delete).await
}
