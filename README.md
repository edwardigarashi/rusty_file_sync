# Rusty File Sync

## Overview

Rusty File Sync is a command-line utility written in Rust for synchronizing files and directories between a source and a destination. It supports one-way and bi-directional synchronization modes with optional deletion of files and directories that are no longer present in the source.

## Features

- **One-way synchronization**: Synchronizes files from the source to the destination.
- **Bi-directional synchronization**: Synchronizes files between the source and the destination in both directions.
- **Optional Deletion**: Optionally delete files and directories in the destination that are not present in the source.
- **File Hashing**: Uses SHA-256 hashing to detect file changes.
- **Continuous Sync**: Continuously syncs until interrupted with `Ctrl+C` or by pressing `q`.
- **Debug Logging**: Provides detailed logging with a debug mode.

## Requirements

- Rust (version 1.53+)
- Cargo (Rust package manager)

## Installation

1. **Clone the repository**:
    ```bash
    git clone https://github.com/edwardigarashi/rusty_file_sync.git
    cd rusty-file-sync
    ```

2. **Build the project**:
    ```bash
    cargo build --release
    ```

3. **Run the executable**:
    ```bash
    ./target/release/rusty_file_sync
    ```

## Usage

### Command Structure

```bash
rusty_file_sync sync <source> <destination> <mode> [options]
