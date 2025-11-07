use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub device: String,
    pub partition_number: Option<u32>,
    pub filesystem: Option<String>,
    pub label: Option<String>,
    pub size_bytes: u64,
    pub used_bytes: u64,
    pub mount_point: Option<String>,
    pub partition_type: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disk {
    pub device: String,
    pub model: String,
    pub size_bytes: u64,
    pub logical_sector_size: u32,
    pub physical_sector_size: u32,
    pub partitions: Vec<Partition>,
}

pub struct PartitionManager {
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {}
    }

    /// List all block devices and their partitions
    pub fn list_disks(&self) -> Result<Vec<Disk>> {
        let mut disks = Vec::new();

        // Use lsblk to get block device information
        let output = Command::new("lsblk")
            .args(&["-J", "-b", "-o", "NAME,TYPE,SIZE,FSTYPE,LABEL,MOUNTPOINT,MODEL"])
            .output()?;

        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            if let Ok(lsblk_data) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(blockdevices) = lsblk_data["blockdevices"].as_array() {
                    for device in blockdevices {
                        if device["type"].as_str() == Some("disk") {
                            disks.push(self.parse_disk(device)?);
                        }
                    }
                }
            }
        }

        Ok(disks)
    }

    fn parse_disk(&self, device: &serde_json::Value) -> Result<Disk> {
        let device_name = device["name"].as_str().unwrap_or("unknown").to_string();
        let model = device["model"].as_str().unwrap_or("Unknown").trim().to_string();
        let size_bytes = device["size"].as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        // Get sector sizes from sysfs
        let (logical_sector_size, physical_sector_size) = self.get_sector_sizes(&device_name);

        // Parse partitions
        let mut partitions = Vec::new();
        if let Some(children) = device["children"].as_array() {
            for child in children {
                if let Some(part) = self.parse_partition(child, &device_name) {
                    partitions.push(part);
                }
            }
        }

        Ok(Disk {
            device: format!("/dev/{}", device_name),
            model,
            size_bytes,
            logical_sector_size,
            physical_sector_size,
            partitions,
        })
    }

    fn parse_partition(&self, part: &serde_json::Value, parent_device: &str) -> Option<Partition> {
        let name = part["name"].as_str()?;
        let size_bytes = part["size"].as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        // Extract partition number
        let partition_number = name.trim_start_matches(parent_device)
            .trim_start_matches('p')
            .parse::<u32>().ok();

        // Get filesystem info
        let filesystem = part["fstype"].as_str().map(|s| s.to_string());
        let label = part["label"].as_str().map(|s| s.to_string());
        let mount_point = part["mountpoint"].as_str().map(|s| s.to_string());

        // Get partition type and flags from parted
        let (partition_type, flags) = self.get_partition_info(&format!("/dev/{}", name));

        // Get used space if mounted
        let used_bytes = if let Some(ref mp) = mount_point {
            self.get_used_space(mp).unwrap_or(0)
        } else {
            0
        };

        Some(Partition {
            device: format!("/dev/{}", name),
            partition_number,
            filesystem,
            label,
            size_bytes,
            used_bytes,
            mount_point,
            partition_type,
            flags,
        })
    }

    fn get_sector_sizes(&self, device: &str) -> (u32, u32) {
        let logical = fs::read_to_string(format!("/sys/block/{}/queue/logical_block_size", device))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(512);

        let physical = fs::read_to_string(format!("/sys/block/{}/queue/physical_block_size", device))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(512);

        (logical, physical)
    }

    fn get_partition_info(&self, device: &str) -> (Option<String>, Vec<String>) {
        // Use parted to get partition type and flags
        let output = Command::new("parted")
            .args(&[device, "print"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse parted output for partition type and flags
                // This is a simplified version - full parsing would be more complex
                for line in stdout.lines() {
                    if line.contains("Partition Table:") {
                        let parts: Vec<&str> = line.split(':').collect();
                        if parts.len() > 1 {
                            return (Some(parts[1].trim().to_string()), Vec::new());
                        }
                    }
                }
            }
        }

        (None, Vec::new())
    }

    fn get_used_space(&self, mount_point: &str) -> Option<u64> {
        let output = Command::new("df")
            .args(&["-B1", mount_point])
            .output()
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();
            if lines.len() > 1 {
                let fields: Vec<&str> = lines[1].split_whitespace().collect();
                if fields.len() > 2 {
                    return fields[2].parse::<u64>().ok();
                }
            }
        }

        None
    }

    /// Create a new partition table (WARNING: destroys all data)
    pub fn create_partition_table(&self, device: &str, table_type: &str) -> Result<()> {
        // table_type can be: gpt, msdos, etc.
        let output = Command::new("parted")
            .args(&["-s", device, "mklabel", table_type])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to create partition table: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Create a new partition
    pub fn create_partition(
        &self,
        device: &str,
        start: &str,
        end: &str,
        fs_type: &str,
    ) -> Result<()> {
        let output = Command::new("parted")
            .args(&["-s", device, "mkpart", "primary", fs_type, start, end])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to create partition: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Delete a partition
    pub fn delete_partition(&self, device: &str, partition_number: u32) -> Result<()> {
        let output = Command::new("parted")
            .args(&["-s", device, "rm", &partition_number.to_string()])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to delete partition: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Resize a partition
    pub fn resize_partition(
        &self,
        device: &str,
        partition_number: u32,
        end: &str,
    ) -> Result<()> {
        let output = Command::new("parted")
            .args(&["-s", device, "resizepart", &partition_number.to_string(), end])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to resize partition: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Format a partition with specified filesystem
    pub fn format_partition(&self, device: &str, filesystem: &str, label: Option<&str>) -> Result<()> {
        let mut args = vec![device];

        match filesystem {
            "ext2" | "ext3" | "ext4" => {
                let mut cmd = Command::new(&format!("mkfs.{}", filesystem));
                if let Some(lbl) = label {
                    cmd.args(&["-L", lbl]);
                }
                cmd.arg(device);
                let output = cmd.output()?;
                if !output.status.success() {
                    anyhow::bail!("Failed to format: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            "xfs" => {
                let mut cmd = Command::new("mkfs.xfs");
                cmd.args(&["-f"]);
                if let Some(lbl) = label {
                    cmd.args(&["-L", lbl]);
                }
                cmd.arg(device);
                let output = cmd.output()?;
                if !output.status.success() {
                    anyhow::bail!("Failed to format: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            "btrfs" => {
                let mut cmd = Command::new("mkfs.btrfs");
                cmd.args(&["-f"]);
                if let Some(lbl) = label {
                    cmd.args(&["-L", lbl]);
                }
                cmd.arg(device);
                let output = cmd.output()?;
                if !output.status.success() {
                    anyhow::bail!("Failed to format: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            "f2fs" => {
                let mut cmd = Command::new("mkfs.f2fs");
                if let Some(lbl) = label {
                    cmd.args(&["-l", lbl]);
                }
                cmd.arg(device);
                let output = cmd.output()?;
                if !output.status.success() {
                    anyhow::bail!("Failed to format: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            "ntfs" => {
                let mut cmd = Command::new("mkfs.ntfs");
                cmd.args(&["-f"]);
                if let Some(lbl) = label {
                    cmd.args(&["-L", lbl]);
                }
                cmd.arg(device);
                let output = cmd.output()?;
                if !output.status.success() {
                    anyhow::bail!("Failed to format: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            "fat32" | "vfat" => {
                let mut cmd = Command::new("mkfs.vfat");
                cmd.args(&["-F", "32"]);
                if let Some(lbl) = label {
                    cmd.args(&["-n", lbl]);
                }
                cmd.arg(device);
                let output = cmd.output()?;
                if !output.status.success() {
                    anyhow::bail!("Failed to format: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            _ => anyhow::bail!("Unsupported filesystem type: {}", filesystem),
        }

        Ok(())
    }

    /// Resize filesystem (must be done after partition resize)
    pub fn resize_filesystem(&self, device: &str, filesystem: &str) -> Result<()> {
        match filesystem {
            "ext2" | "ext3" | "ext4" => {
                let output = Command::new("resize2fs")
                    .arg(device)
                    .output()?;
                if !output.status.success() {
                    anyhow::bail!("Failed to resize filesystem: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            "xfs" => {
                // XFS requires the filesystem to be mounted
                anyhow::bail!("XFS filesystem must be mounted to resize. Use 'xfs_growfs' on the mount point.");
            }
            "btrfs" => {
                let output = Command::new("btrfs")
                    .args(&["filesystem", "resize", "max", device])
                    .output()?;
                if !output.status.success() {
                    anyhow::bail!("Failed to resize filesystem: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            _ => anyhow::bail!("Filesystem resize not supported for: {}", filesystem),
        }

        Ok(())
    }

    /// Set partition flags
    pub fn set_partition_flag(&self, device: &str, partition_number: u32, flag: &str, state: bool) -> Result<()> {
        let state_str = if state { "on" } else { "off" };
        let output = Command::new("parted")
            .args(&["-s", device, "set", &partition_number.to_string(), flag, state_str])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to set flag: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    /// Check filesystem for errors
    pub fn check_filesystem(&self, device: &str, filesystem: &str, repair: bool) -> Result<String> {
        let output = match filesystem {
            "ext2" | "ext3" | "ext4" => {
                let mut cmd = Command::new("e2fsck");
                if repair {
                    cmd.args(&["-p"]); // Automatic repair
                } else {
                    cmd.args(&["-n"]); // No changes, just check
                }
                cmd.arg(device).output()?
            }
            "xfs" => {
                Command::new("xfs_repair")
                    .args(&[if repair { "-n" } else { "-n" }, device])
                    .output()?
            }
            "btrfs" => {
                Command::new("btrfs")
                    .args(&["check", if repair { "--repair" } else { "" }, device])
                    .output()?
            }
            _ => anyhow::bail!("Filesystem check not supported for: {}", filesystem),
        };

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get supported filesystems on this system
    pub fn get_supported_filesystems(&self) -> Vec<String> {
        let mut filesystems = vec![
            "ext2", "ext3", "ext4", "xfs", "btrfs", "f2fs",
            "ntfs", "vfat", "fat32", "exfat", "swap"
        ];

        // Check which mkfs utilities are available
        let mut available = Vec::new();
        for fs in filesystems {
            let binary = match fs {
                "fat32" | "vfat" => "mkfs.vfat",
                _ => &format!("mkfs.{}", fs),
            };

            if Command::new("which").arg(binary).output().ok()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                available.push(fs.to_string());
            }
        }

        available
    }
}

impl Default for PartitionManager {
    fn default() -> Self {
        Self::new()
    }
}
