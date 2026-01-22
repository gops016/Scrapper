use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};
use log::{info, error};

const PROGRESS_FILE: &str = "progress.json";

#[derive(Serialize, Deserialize, Default)]
pub struct ProgressState {
    pub processed_urls: HashSet<String>,
}

impl ProgressState {
    pub fn load() -> Self {
        if Path::new(PROGRESS_FILE).exists() {
            let mut file = match File::open(PROGRESS_FILE) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to open progress file: {}", e);
                    return ProgressState::default();
                }
            };
            let mut content = String::new();
            if let Err(e) = file.read_to_string(&mut content) {
                error!("Failed to read progress file: {}", e);
                return ProgressState::default();
            }
            match serde_json::from_str::<ProgressState>(&content) {
                Ok(state) => {
                    info!("Resumed previous session: {} sites processed.", state.processed_urls.len());
                    state
                },
                Err(e) => {
                    error!("Failed to parse progress file: {}. Starting fresh.", e);
                    ProgressState::default()
                }
            }
        } else {
            info!("No progress file found. Starting fresh.");
            ProgressState::default()
        }
    }

    pub fn mark_complete(&mut self, url: String) {
        self.processed_urls.insert(url);
        self.save();
    }

    pub fn contains(&self, url: &str) -> bool {
        self.processed_urls.contains(url)
    }

    fn save(&self) {
        let json = match serde_json::to_string_pretty(self) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to serialize progress state: {}", e);
                return;
            }
        };

        let mut file = match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(PROGRESS_FILE) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open progress file for writing: {}", e);
                return;
            }
        };

        if let Err(e) = file.write_all(json.as_bytes()) {
            error!("Failed to write to progress file: {}", e);
        }
    }
}
