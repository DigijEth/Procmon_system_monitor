use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub uid: u32,
    pub exe_path: Option<PathBuf>,
    pub command_line: Vec<String>,
    pub status: ProcessStatus,
    pub parent_pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Sleeping,
    Stopped,
    Zombie,
    Dead,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    pub pid: u32,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub memory_percent: f32,
    pub virtual_memory: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub num_threads: u32,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub run_time: std::time::Duration,
}

impl ProcessInfo {
    pub fn new(
        pid: u32,
        name: String,
        user: String,
        uid: u32,
    ) -> Self {
        Self {
            pid,
            name,
            user,
            uid,
            exe_path: None,
            command_line: Vec::new(),
            status: ProcessStatus::Unknown,
            parent_pid: None,
        }
    }
}

impl Default for ProcessStats {
    fn default() -> Self {
        Self {
            pid: 0,
            cpu_usage: 0.0,
            memory_usage: 0,
            memory_percent: 0.0,
            virtual_memory: 0,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            num_threads: 0,
            start_time: chrono::Utc::now(),
            run_time: std::time::Duration::from_secs(0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    pub info: ProcessInfo,
    pub stats: ProcessStats,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
