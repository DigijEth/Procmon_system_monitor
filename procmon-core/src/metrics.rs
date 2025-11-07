use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub total_usage: f32,
    pub per_core_usage: Vec<f32>,
    pub temperature: Option<f32>,
    pub frequency: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMetrics {
    pub name: String,
    pub usage: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub interface_name: String,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub errors_in: u64,
    pub errors_out: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIoMetrics {
    pub device_name: String,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub read_ops: u64,
    pub write_ops: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbIoMetrics {
    pub device_id: String,
    pub device_name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bytes_transferred: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub gpus: Vec<GpuMetrics>,
    pub network: HashMap<String, NetworkMetrics>,
    pub disk_io: HashMap<String, DiskIoMetrics>,
    pub usb_io: Vec<UsbIoMetrics>,
}

impl Default for CpuMetrics {
    fn default() -> Self {
        Self {
            total_usage: 0.0,
            per_core_usage: Vec::new(),
            temperature: None,
            frequency: None,
        }
    }
}

impl Default for MemoryMetrics {
    fn default() -> Self {
        Self {
            total: 0,
            used: 0,
            available: 0,
            swap_total: 0,
            swap_used: 0,
        }
    }
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            cpu: CpuMetrics::default(),
            memory: MemoryMetrics::default(),
            gpus: Vec::new(),
            network: HashMap::new(),
            disk_io: HashMap::new(),
            usb_io: Vec::new(),
        }
    }
}
