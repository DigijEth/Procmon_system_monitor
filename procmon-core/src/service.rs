use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemService {
    pub name: String,
    pub description: String,
    pub state: ServiceState,
    pub enabled: bool,
    pub active_state: String,
    pub sub_state: String,
    pub memory_usage: Option<u64>,
    pub cpu_usage: Option<f32>,
    pub main_pid: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceState {
    Running,
    Stopped,
    Failed,
    Unknown,
}

impl From<&str> for ServiceState {
    fn from(s: &str) -> Self {
        match s {
            "active" | "running" => ServiceState::Running,
            "inactive" | "dead" => ServiceState::Stopped,
            "failed" => ServiceState::Failed,
            _ => ServiceState::Unknown,
        }
    }
}

pub struct ServiceManager {
    // No state needed, operates on systemctl
}

impl ServiceManager {
    pub fn new() -> Self {
        Self {}
    }

    /// List all systemd services
    pub fn list_services(&self) -> Result<Vec<SystemService>> {
        let output = Command::new("systemctl")
            .args(&["list-units", "--type=service", "--all", "--no-pager", "--plain"])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to list services: {}", String::from_utf8_lossy(&output.stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut services = Vec::new();

        for line in stdout.lines().skip(1) { // Skip header
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            // Format: UNIT LOAD ACTIVE SUB DESCRIPTION
            let unit_name = parts[0];
            if !unit_name.ends_with(".service") {
                continue;
            }

            let name = unit_name.trim_end_matches(".service").to_string();
            let active_state = parts[2].to_string();
            let sub_state = parts[3].to_string();
            let description = if parts.len() > 4 {
                parts[4..].join(" ")
            } else {
                String::new()
            };

            let state = ServiceState::from(active_state.as_str());

            // Check if service is enabled
            let enabled = self.is_service_enabled(&name).unwrap_or(false);

            // Get detailed info including PID and resource usage
            let (main_pid, memory_usage, cpu_usage) = self.get_service_details(&name).unwrap_or((None, None, None));

            services.push(SystemService {
                name,
                description,
                state,
                enabled,
                active_state,
                sub_state,
                memory_usage,
                cpu_usage,
                main_pid,
            });
        }

        Ok(services)
    }

    /// Get detailed information about a service
    fn get_service_details(&self, service_name: &str) -> Result<(Option<u32>, Option<u64>, Option<f32>)> {
        let output = Command::new("systemctl")
            .args(&["show", &format!("{}.service", service_name), "--no-pager"])
            .output()?;

        if !output.status.success() {
            return Ok((None, None, None));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut main_pid = None;
        let mut memory_usage = None;

        for line in stdout.lines() {
            if let Some(value) = line.strip_prefix("MainPID=") {
                if let Ok(pid) = value.parse::<u32>() {
                    if pid > 0 {
                        main_pid = Some(pid);
                    }
                }
            } else if let Some(value) = line.strip_prefix("MemoryCurrent=") {
                if let Ok(mem) = value.parse::<u64>() {
                    if mem > 0 {
                        memory_usage = Some(mem);
                    }
                }
            }
        }

        // CPU usage would require tracking over time, skip for now
        Ok((main_pid, memory_usage, None))
    }

    /// Check if a service is enabled
    fn is_service_enabled(&self, service_name: &str) -> Result<bool> {
        let output = Command::new("systemctl")
            .args(&["is-enabled", &format!("{}.service", service_name)])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim() == "enabled")
    }

    /// Start a service
    pub fn start_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(&["start", &format!("{}.service", service_name)])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to start service: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Stop a service
    pub fn stop_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(&["stop", &format!("{}.service", service_name)])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to stop service: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Restart a service
    pub fn restart_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(&["restart", &format!("{}.service", service_name)])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to restart service: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Enable a service
    pub fn enable_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(&["enable", &format!("{}.service", service_name)])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to enable service: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Disable a service
    pub fn disable_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(&["disable", &format!("{}.service", service_name)])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to disable service: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Get service status details
    pub fn get_service_status(&self, service_name: &str) -> Result<String> {
        let output = Command::new("systemctl")
            .args(&["status", &format!("{}.service", service_name), "--no-pager"])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}
