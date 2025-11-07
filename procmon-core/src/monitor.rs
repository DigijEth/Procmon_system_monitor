use crate::metrics::*;
use crate::process::{ProcessInfo, ProcessStats, ProcessSnapshot, ProcessStatus};
use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use sysinfo::{System, Process, Pid, Networks, Disks};

pub struct SystemMonitor {
    system: Arc<RwLock<System>>,
    networks: Arc<RwLock<Networks>>,
    disks: Arc<RwLock<Disks>>,
    previous_disk_stats: Arc<RwLock<HashMap<String, (u64, u64)>>>,
    previous_net_stats: Arc<RwLock<HashMap<String, (u64, u64)>>>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        // Start with empty system, we'll populate it on first refresh
        let system = System::new();

        Self {
            system: Arc::new(RwLock::new(system)),
            networks: Arc::new(RwLock::new(Networks::new_with_refreshed_list())),
            disks: Arc::new(RwLock::new(Disks::new_with_refreshed_list())),
            previous_disk_stats: Arc::new(RwLock::new(HashMap::new())),
            previous_net_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn refresh(&self) {
        let mut system = self.system.write();
        // IMPORTANT: We need to completely rebuild the process list to avoid stale PIDs
        // sysinfo has a known issue where it doesn't properly remove terminated processes
        // So we clear the process list and rebuild it from scratch
        use sysinfo::{ProcessRefreshKind, RefreshKind, MemoryRefreshKind, CpuRefreshKind};

        // Create a completely fresh system to avoid accumulated stale processes
        *system = System::new_with_specifics(RefreshKind::new()
            .with_processes(ProcessRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything())
            .with_cpu(CpuRefreshKind::everything()));

        let mut networks = self.networks.write();
        networks.refresh();

        let mut disks = self.disks.write();
        disks.refresh();
    }

    pub fn get_system_metrics(&self) -> Result<SystemMetrics> {
        let system = self.system.read();
        let networks = self.networks.read();

        let cpu = self.get_cpu_metrics(&system)?;
        let memory = self.get_memory_metrics(&system)?;
        let gpus = self.get_gpu_metrics()?;
        let network = self.get_network_metrics(&networks)?;
        let disk_io = self.get_disk_io_metrics()?;
        let usb_io = self.get_usb_io_metrics()?;

        Ok(SystemMetrics {
            timestamp: chrono::Utc::now(),
            cpu,
            memory,
            gpus,
            network,
            disk_io,
            usb_io,
        })
    }

    fn get_cpu_metrics(&self, system: &System) -> Result<CpuMetrics> {
        let cpus = system.cpus();
        let total_usage = system.global_cpu_usage();
        let per_core_usage: Vec<f32> = cpus.iter().map(|cpu| cpu.cpu_usage()).collect();

        let temperature = self.read_cpu_temperature();
        let frequency = cpus.first().map(|cpu| cpu.frequency());

        Ok(CpuMetrics {
            total_usage,
            per_core_usage,
            temperature,
            frequency,
        })
    }

    fn get_memory_metrics(&self, system: &System) -> Result<MemoryMetrics> {
        Ok(MemoryMetrics {
            total: system.total_memory(),
            used: system.used_memory(),
            available: system.available_memory(),
            swap_total: system.total_swap(),
            swap_used: system.used_swap(),
        })
    }

    fn get_gpu_metrics(&self) -> Result<Vec<GpuMetrics>> {
        // GPU monitoring is complex and platform-specific
        // On Linux, we can read from /sys/class/drm or use nvml for NVIDIA
        let mut gpus = Vec::new();

        // Try to detect AMD GPUs via sysfs
        if let Ok(entries) = fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str.starts_with("card") && !name_str.contains('-') {
                    if let Some(gpu) = self.read_amd_gpu_info(&path) {
                        gpus.push(gpu);
                    }
                }
            }
        }

        Ok(gpus)
    }

    fn read_amd_gpu_info(&self, card_path: &Path) -> Option<GpuMetrics> {
        let device_path = card_path.join("device");

        let name = fs::read_to_string(device_path.join("product_name"))
            .or_else(|_| fs::read_to_string(device_path.join("model")))
            .unwrap_or_else(|_| "Unknown GPU".to_string())
            .trim()
            .to_string();

        // Try to read GPU usage
        let usage = fs::read_to_string(device_path.join("gpu_busy_percent"))
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(0.0);

        // Try to read VRAM usage
        let memory_used = fs::read_to_string(device_path.join("mem_info_vram_used"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);

        let memory_total = fs::read_to_string(device_path.join("mem_info_vram_total"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);

        // Try to read temperature
        let temperature = fs::read_to_string(device_path.join("hwmon/hwmon0/temp1_input"))
            .or_else(|_| fs::read_to_string(device_path.join("hwmon/hwmon1/temp1_input")))
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .map(|t| t / 1000.0); // Convert from millidegrees

        Some(GpuMetrics {
            name,
            usage,
            memory_used,
            memory_total,
            temperature,
        })
    }

    fn get_network_metrics(&self, networks: &Networks) -> Result<HashMap<String, NetworkMetrics>> {
        let mut result = HashMap::new();

        for (interface_name, data) in networks.iter() {
            let metrics = NetworkMetrics {
                interface_name: interface_name.to_string(),
                bytes_sent: data.total_transmitted(),
                bytes_received: data.total_received(),
                packets_sent: data.total_packets_transmitted(),
                packets_received: data.total_packets_received(),
                errors_in: data.total_errors_on_received(),
                errors_out: data.total_errors_on_transmitted(),
            };
            result.insert(interface_name.to_string(), metrics);
        }

        Ok(result)
    }

    fn get_disk_io_metrics(&self) -> Result<HashMap<String, DiskIoMetrics>> {
        let mut result = HashMap::new();

        // Read disk I/O stats from /proc/diskstats on Linux
        if let Ok(content) = fs::read_to_string("/proc/diskstats") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 14 {
                    let device_name = parts[2].to_string();

                    // Skip loop and ram devices
                    if device_name.starts_with("loop") || device_name.starts_with("ram") {
                        continue;
                    }

                    let read_ops = parts[3].parse::<u64>().unwrap_or(0);
                    let read_sectors = parts[5].parse::<u64>().unwrap_or(0);
                    let write_ops = parts[7].parse::<u64>().unwrap_or(0);
                    let write_sectors = parts[9].parse::<u64>().unwrap_or(0);

                    let metrics = DiskIoMetrics {
                        device_name: device_name.clone(),
                        read_bytes: read_sectors * 512, // sectors are 512 bytes
                        write_bytes: write_sectors * 512,
                        read_ops,
                        write_ops,
                    };

                    result.insert(device_name, metrics);
                }
            }
        }

        Ok(result)
    }

    fn get_usb_io_metrics(&self) -> Result<Vec<UsbIoMetrics>> {
        let mut usb_devices = Vec::new();

        // Read USB device information from /sys/bus/usb/devices
        if let Ok(entries) = fs::read_dir("/sys/bus/usb/devices") {
            for entry in entries.flatten() {
                let path = entry.path();

                // Read vendor and product IDs
                let vendor_id = fs::read_to_string(path.join("idVendor"))
                    .ok()
                    .and_then(|s| u16::from_str_radix(s.trim(), 16).ok())
                    .unwrap_or(0);

                let product_id = fs::read_to_string(path.join("idProduct"))
                    .ok()
                    .and_then(|s| u16::from_str_radix(s.trim(), 16).ok())
                    .unwrap_or(0);

                if vendor_id == 0 && product_id == 0 {
                    continue;
                }

                let device_name = fs::read_to_string(path.join("product"))
                    .unwrap_or_else(|_| "Unknown USB Device".to_string())
                    .trim()
                    .to_string();

                let device_id = entry.file_name().to_string_lossy().to_string();

                usb_devices.push(UsbIoMetrics {
                    device_id,
                    device_name,
                    vendor_id,
                    product_id,
                    bytes_transferred: 0, // Would need more complex tracking
                });
            }
        }

        Ok(usb_devices)
    }

    fn read_cpu_temperature(&self) -> Option<f32> {
        // Try to read from common thermal zones
        for i in 0..10 {
            let temp_path = format!("/sys/class/thermal/thermal_zone{}/temp", i);
            if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                if let Ok(temp) = temp_str.trim().parse::<f32>() {
                    return Some(temp / 1000.0); // Convert from millidegrees
                }
            }
        }

        // Try hwmon
        if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
            for entry in entries.flatten() {
                let temp_path = entry.path().join("temp1_input");
                if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                    if let Ok(temp) = temp_str.trim().parse::<f32>() {
                        return Some(temp / 1000.0);
                    }
                }
            }
        }

        None
    }

    pub fn get_all_processes(&self) -> Result<Vec<ProcessSnapshot>> {
        let system = self.system.read();
        let mut processes = Vec::new();

        let total_from_sysinfo = system.processes().len();
        let mut skipped_count = 0;

        // Build a set of actual process PIDs (not threads) by reading /proc directory
        // This is the most reliable way to distinguish processes from threads
        let mut real_pids = std::collections::HashSet::new();
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    if let Ok(pid) = file_name.parse::<u32>() {
                        real_pids.insert(pid);
                    }
                }
            }
        }

        for (pid, process) in system.processes() {
            let pid_u32 = pid.as_u32();

            // Only include PIDs that are actual processes (in /proc directory listing)
            // This filters out threads which have /proc/{tid} entries but aren't in directory listing
            if !real_pids.contains(&pid_u32) {
                skipped_count += 1;
                continue;
            }

            if let Some(snapshot) = self.process_to_snapshot(*pid, process) {
                processes.push(snapshot);
            }
        }

        #[cfg(test)]
        eprintln!("get_all_processes: sysinfo reported {}, skipped {}, returning {}",
                  total_from_sysinfo, skipped_count, processes.len());

        Ok(processes)
    }

    pub fn get_process(&self, pid: u32) -> Result<Option<ProcessSnapshot>> {
        let system = self.system.read();
        let pid = Pid::from_u32(pid);

        Ok(system.process(pid).and_then(|p| self.process_to_snapshot(pid, p)))
    }

    fn process_to_snapshot(&self, pid: Pid, process: &Process) -> Option<ProcessSnapshot> {
        let user = self.get_process_user(pid.as_u32());

        let info = ProcessInfo {
            pid: pid.as_u32(),
            name: process.name().to_string_lossy().to_string(),
            user: user.0,
            uid: user.1,
            exe_path: process.exe().map(|p| p.to_path_buf()),
            command_line: process.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect(),
            status: self.convert_process_status(process.status()),
            parent_pid: process.parent().map(|p| p.as_u32()),
        };

        let stats = ProcessStats {
            pid: pid.as_u32(),
            cpu_usage: process.cpu_usage(),
            memory_usage: process.memory(),
            memory_percent: 0.0, // Calculate if needed
            virtual_memory: process.virtual_memory(),
            disk_read_bytes: process.disk_usage().read_bytes,
            disk_write_bytes: process.disk_usage().written_bytes,
            network_rx_bytes: 0, // Would need per-process network tracking
            network_tx_bytes: 0,
            num_threads: 0, // Not available in sysinfo
            start_time: chrono::Utc::now(), // Would need to calculate from process start time
            run_time: std::time::Duration::from_secs(process.run_time()),
        };

        Some(ProcessSnapshot {
            info,
            stats,
            timestamp: chrono::Utc::now(),
        })
    }

    fn get_process_user(&self, pid: u32) -> (String, u32) {
        // Try to read user from /proc
        let status_path = format!("/proc/{}/status", pid);
        if let Ok(content) = fs::read_to_string(&status_path) {
            for line in content.lines() {
                if line.starts_with("Uid:") {
                    if let Some(uid_str) = line.split_whitespace().nth(1) {
                        if let Ok(uid) = uid_str.parse::<u32>() {
                            let username = self.uid_to_username(uid);
                            return (username, uid);
                        }
                    }
                }
            }
        }

        ("unknown".to_string(), 0)
    }

    fn uid_to_username(&self, uid: u32) -> String {
        // Try to read from /etc/passwd
        if let Ok(content) = fs::read_to_string("/etc/passwd") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    if let Ok(line_uid) = parts[2].parse::<u32>() {
                        if line_uid == uid {
                            return parts[0].to_string();
                        }
                    }
                }
            }
        }

        format!("uid:{}", uid)
    }

    fn convert_process_status(&self, status: sysinfo::ProcessStatus) -> ProcessStatus {
        match status {
            sysinfo::ProcessStatus::Run => ProcessStatus::Running,
            sysinfo::ProcessStatus::Sleep => ProcessStatus::Sleeping,
            sysinfo::ProcessStatus::Stop => ProcessStatus::Stopped,
            sysinfo::ProcessStatus::Zombie => ProcessStatus::Zombie,
            sysinfo::ProcessStatus::Dead => ProcessStatus::Dead,
            _ => ProcessStatus::Unknown,
        }
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}
