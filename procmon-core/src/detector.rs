use crate::process::ProcessSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MisbehaviorRule {
    pub name: String,
    pub description: String,
    pub condition: MisbehaviorCondition,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MisbehaviorCondition {
    CpuUsageAbove { threshold: f32, duration_secs: u64 },
    MemoryUsageAbove { threshold_bytes: u64, duration_secs: u64 },
    MemoryPercentAbove { threshold_percent: f32, duration_secs: u64 },
    DiskIoAbove { threshold_bytes_per_sec: u64, duration_secs: u64 },
    NetworkIoAbove { threshold_bytes_per_sec: u64, duration_secs: u64 },
    TooManyThreads { threshold: u32 },
    ZombieProcess,
    HighDiskWrites { threshold_bytes_per_sec: u64, duration_secs: u64 },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MisbehaviorAlert {
    pub pid: u32,
    pub process_name: String,
    pub rule_name: String,
    pub description: String,
    pub severity: Severity,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub details: String,
}

pub struct MisbehaviorDetector {
    rules: Vec<MisbehaviorRule>,
    violation_history: HashMap<u32, Vec<ViolationRecord>>,
}

#[derive(Debug, Clone)]
struct ViolationRecord {
    rule_name: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl MisbehaviorDetector {
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
            violation_history: HashMap::new(),
        }
    }

    pub fn with_rules(rules: Vec<MisbehaviorRule>) -> Self {
        Self {
            rules,
            violation_history: HashMap::new(),
        }
    }

    fn default_rules() -> Vec<MisbehaviorRule> {
        vec![
            MisbehaviorRule {
                name: "High CPU Usage".to_string(),
                description: "Process using more than 80% CPU for extended period".to_string(),
                condition: MisbehaviorCondition::CpuUsageAbove {
                    threshold: 80.0,
                    duration_secs: 60,
                },
                severity: Severity::Warning,
            },
            MisbehaviorRule {
                name: "Extreme CPU Usage".to_string(),
                description: "Process using more than 95% CPU".to_string(),
                condition: MisbehaviorCondition::CpuUsageAbove {
                    threshold: 95.0,
                    duration_secs: 10,
                },
                severity: Severity::Critical,
            },
            MisbehaviorRule {
                name: "High Memory Usage".to_string(),
                description: "Process using more than 2GB of RAM".to_string(),
                condition: MisbehaviorCondition::MemoryUsageAbove {
                    threshold_bytes: 2 * 1024 * 1024 * 1024,
                    duration_secs: 30,
                },
                severity: Severity::Warning,
            },
            MisbehaviorRule {
                name: "Memory Leak Suspected".to_string(),
                description: "Process using more than 8GB of RAM".to_string(),
                condition: MisbehaviorCondition::MemoryUsageAbove {
                    threshold_bytes: 8 * 1024 * 1024 * 1024,
                    duration_secs: 10,
                },
                severity: Severity::Critical,
            },
            MisbehaviorRule {
                name: "Zombie Process".to_string(),
                description: "Process is in zombie state".to_string(),
                condition: MisbehaviorCondition::ZombieProcess,
                severity: Severity::Warning,
            },
            MisbehaviorRule {
                name: "High Disk I/O".to_string(),
                description: "Process performing excessive disk operations".to_string(),
                condition: MisbehaviorCondition::DiskIoAbove {
                    threshold_bytes_per_sec: 100 * 1024 * 1024, // 100 MB/s
                    duration_secs: 60,
                },
                severity: Severity::Warning,
            },
        ]
    }

    pub fn add_rule(&mut self, rule: MisbehaviorRule) {
        self.rules.push(rule);
    }

    pub fn check_process(&mut self, snapshot: &ProcessSnapshot) -> Vec<MisbehaviorAlert> {
        let mut alerts = Vec::new();
        let rules = self.rules.clone();

        for rule in &rules {
            if self.check_rule(snapshot, rule) {
                let alert = MisbehaviorAlert {
                    pid: snapshot.info.pid,
                    process_name: snapshot.info.name.clone(),
                    rule_name: rule.name.clone(),
                    description: rule.description.clone(),
                    severity: rule.severity,
                    timestamp: chrono::Utc::now(),
                    details: self.get_violation_details(snapshot, &rule.condition),
                };

                alerts.push(alert);
            }
        }

        alerts
    }

    fn check_rule(&mut self, snapshot: &ProcessSnapshot, rule: &MisbehaviorRule) -> bool {
        match &rule.condition {
            MisbehaviorCondition::CpuUsageAbove { threshold, duration_secs } => {
                if snapshot.stats.cpu_usage > *threshold {
                    self.record_violation(snapshot.info.pid, &rule.name, *duration_secs)
                } else {
                    false
                }
            }
            MisbehaviorCondition::MemoryUsageAbove { threshold_bytes, duration_secs } => {
                if snapshot.stats.memory_usage > *threshold_bytes {
                    self.record_violation(snapshot.info.pid, &rule.name, *duration_secs)
                } else {
                    false
                }
            }
            MisbehaviorCondition::MemoryPercentAbove { threshold_percent, duration_secs } => {
                if snapshot.stats.memory_percent > *threshold_percent {
                    self.record_violation(snapshot.info.pid, &rule.name, *duration_secs)
                } else {
                    false
                }
            }
            MisbehaviorCondition::DiskIoAbove { threshold_bytes_per_sec, duration_secs } => {
                let total_io = snapshot.stats.disk_read_bytes + snapshot.stats.disk_write_bytes;
                let io_per_sec = total_io / snapshot.stats.run_time.as_secs().max(1);

                if io_per_sec > *threshold_bytes_per_sec {
                    self.record_violation(snapshot.info.pid, &rule.name, *duration_secs)
                } else {
                    false
                }
            }
            MisbehaviorCondition::NetworkIoAbove { threshold_bytes_per_sec, duration_secs } => {
                let total_net = snapshot.stats.network_rx_bytes + snapshot.stats.network_tx_bytes;
                let net_per_sec = total_net / snapshot.stats.run_time.as_secs().max(1);

                if net_per_sec > *threshold_bytes_per_sec {
                    self.record_violation(snapshot.info.pid, &rule.name, *duration_secs)
                } else {
                    false
                }
            }
            MisbehaviorCondition::TooManyThreads { threshold } => {
                snapshot.stats.num_threads > *threshold
            }
            MisbehaviorCondition::ZombieProcess => {
                matches!(snapshot.info.status, crate::process::ProcessStatus::Zombie)
            }
            MisbehaviorCondition::HighDiskWrites { threshold_bytes_per_sec, duration_secs } => {
                let write_per_sec = snapshot.stats.disk_write_bytes / snapshot.stats.run_time.as_secs().max(1);

                if write_per_sec > *threshold_bytes_per_sec {
                    self.record_violation(snapshot.info.pid, &rule.name, *duration_secs)
                } else {
                    false
                }
            }
        }
    }

    fn record_violation(&mut self, pid: u32, rule_name: &str, duration_secs: u64) -> bool {
        let now = chrono::Utc::now();
        let history = self.violation_history.entry(pid).or_insert_with(Vec::new);

        // Add new violation
        history.push(ViolationRecord {
            rule_name: rule_name.to_string(),
            timestamp: now,
        });

        // Clean up old violations
        let cutoff = now - chrono::Duration::seconds(duration_secs as i64);
        history.retain(|v| v.timestamp > cutoff && v.rule_name == rule_name);

        // Check if violation has persisted for the required duration
        if let Some(first) = history.first() {
            let violation_duration = (now - first.timestamp).num_seconds() as u64;
            violation_duration >= duration_secs
        } else {
            false
        }
    }

    fn get_violation_details(&self, snapshot: &ProcessSnapshot, condition: &MisbehaviorCondition) -> String {
        match condition {
            MisbehaviorCondition::CpuUsageAbove { threshold, .. } => {
                format!("CPU usage: {:.1}% (threshold: {:.1}%)", snapshot.stats.cpu_usage, threshold)
            }
            MisbehaviorCondition::MemoryUsageAbove { threshold_bytes, .. } => {
                format!(
                    "Memory usage: {:.2} GB (threshold: {:.2} GB)",
                    snapshot.stats.memory_usage as f64 / (1024.0 * 1024.0 * 1024.0),
                    *threshold_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                )
            }
            MisbehaviorCondition::MemoryPercentAbove { threshold_percent, .. } => {
                format!("Memory usage: {:.1}% (threshold: {:.1}%)", snapshot.stats.memory_percent, threshold_percent)
            }
            MisbehaviorCondition::DiskIoAbove { threshold_bytes_per_sec, .. } => {
                let total_io = snapshot.stats.disk_read_bytes + snapshot.stats.disk_write_bytes;
                let io_per_sec = total_io / snapshot.stats.run_time.as_secs().max(1);
                format!(
                    "Disk I/O: {:.2} MB/s (threshold: {:.2} MB/s)",
                    io_per_sec as f64 / (1024.0 * 1024.0),
                    *threshold_bytes_per_sec as f64 / (1024.0 * 1024.0)
                )
            }
            MisbehaviorCondition::NetworkIoAbove { threshold_bytes_per_sec, .. } => {
                let total_net = snapshot.stats.network_rx_bytes + snapshot.stats.network_tx_bytes;
                let net_per_sec = total_net / snapshot.stats.run_time.as_secs().max(1);
                format!(
                    "Network I/O: {:.2} MB/s (threshold: {:.2} MB/s)",
                    net_per_sec as f64 / (1024.0 * 1024.0),
                    *threshold_bytes_per_sec as f64 / (1024.0 * 1024.0)
                )
            }
            MisbehaviorCondition::TooManyThreads { threshold } => {
                format!("Threads: {} (threshold: {})", snapshot.stats.num_threads, threshold)
            }
            MisbehaviorCondition::ZombieProcess => {
                "Process is in zombie state".to_string()
            }
            MisbehaviorCondition::HighDiskWrites { threshold_bytes_per_sec, .. } => {
                let write_per_sec = snapshot.stats.disk_write_bytes / snapshot.stats.run_time.as_secs().max(1);
                format!(
                    "Disk writes: {:.2} MB/s (threshold: {:.2} MB/s)",
                    write_per_sec as f64 / (1024.0 * 1024.0),
                    *threshold_bytes_per_sec as f64 / (1024.0 * 1024.0)
                )
            }
        }
    }

    pub fn cleanup_dead_processes(&mut self, active_pids: &[u32]) {
        self.violation_history.retain(|pid, _| active_pids.contains(pid));
    }

    pub fn get_rules(&self) -> &[MisbehaviorRule] {
        &self.rules
    }
}

impl Default for MisbehaviorDetector {
    fn default() -> Self {
        Self::new()
    }
}
