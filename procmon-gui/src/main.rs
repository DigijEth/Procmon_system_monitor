use eframe::egui;
use procmon_core::{
    MisbehaviorDetector, SystemMetrics, SystemMonitor, PartitionManager, Disk,
    ServiceManager, SystemService, ServiceState,
    process::ProcessSnapshot,
    detector::Severity,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("Process Monitor with Partition Manager"),
        ..Default::default()
    };

    eframe::run_native(
        "Process Monitor",
        options,
        Box::new(|_cc| Ok(Box::new(ProcessMonitorApp::new()))),
    )
}

struct ProcessMonitorApp {
    monitor: Arc<RwLock<SystemMonitor>>,
    detector: Arc<RwLock<MisbehaviorDetector>>,
    partition_manager: Arc<RwLock<PartitionManager>>,
    service_manager: Arc<RwLock<ServiceManager>>,
    system_metrics: Arc<RwLock<SystemMetrics>>,
    processes: Arc<RwLock<Vec<ProcessSnapshot>>>,
    disks: Arc<RwLock<Vec<Disk>>>,
    services: Arc<RwLock<Vec<SystemService>>>,
    alerts: Arc<RwLock<Vec<procmon_core::MisbehaviorAlert>>>,
    selected_tab: usize,
    sort_by_cpu: bool,
    selected_process: Option<usize>,
    selected_process_pid: Option<u32>,
    show_process_context_menu: bool,
    context_menu_pos: egui::Pos2,
    selected_disk: Option<usize>,
    selected_partition: Option<usize>,
    status_message: String,
    show_format_dialog: bool,
    format_filesystem: String,
    show_delete_confirm: bool,
}

impl ProcessMonitorApp {
    fn new() -> Self {
        let monitor = SystemMonitor::new();
        monitor.refresh();

        let partition_manager = PartitionManager::new();
        let disks = partition_manager.list_disks().unwrap_or_default();

        let service_manager = ServiceManager::new();
        let services = service_manager.list_services().unwrap_or_default();

        let system_metrics = monitor.get_system_metrics().unwrap_or_default();
        let processes = monitor.get_all_processes().unwrap_or_default();

        let monitor = Arc::new(RwLock::new(monitor));
        let detector = Arc::new(RwLock::new(MisbehaviorDetector::new()));
        let partition_manager = Arc::new(RwLock::new(partition_manager));
        let service_manager = Arc::new(RwLock::new(service_manager));
        let system_metrics = Arc::new(RwLock::new(system_metrics));
        let processes = Arc::new(RwLock::new(processes));
        let disks = Arc::new(RwLock::new(disks));
        let services = Arc::new(RwLock::new(services));
        let alerts = Arc::new(RwLock::new(Vec::new()));

        // Spawn background update task
        let monitor_clone = monitor.clone();
        let detector_clone = detector.clone();
        let partition_manager_clone = partition_manager.clone();
        let service_manager_clone = service_manager.clone();
        let system_metrics_clone = system_metrics.clone();
        let processes_clone = processes.clone();
        let disks_clone = disks.clone();
        let services_clone = services.clone();
        let alerts_clone = alerts.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;

                    let monitor = monitor_clone.read();
                    monitor.refresh();

                    if let Ok(metrics) = monitor.get_system_metrics() {
                        *system_metrics_clone.write() = metrics;
                    }

                    if let Ok(procs) = monitor.get_all_processes() {
                        *processes_clone.write() = procs.clone();

                        let mut detector = detector_clone.write();
                        let mut alerts = alerts_clone.write();

                        for process in &procs {
                            let process_alerts = detector.check_process(process);
                            alerts.extend(process_alerts);
                        }

                        let alerts_len = alerts.len();
                        if alerts_len > 100 {
                            alerts.drain(0..alerts_len - 100);
                        }

                        let active_pids: Vec<u32> = procs.iter().map(|p| p.info.pid).collect();
                        detector.cleanup_dead_processes(&active_pids);
                    }

                    // Refresh disks every 5 seconds
                    if Instant::now().elapsed().as_secs() % 5 == 0 {
                        let pm = partition_manager_clone.read();
                        if let Ok(disk_list) = pm.list_disks() {
                            *disks_clone.write() = disk_list;
                        }
                    }

                    // Refresh services every 3 seconds
                    if Instant::now().elapsed().as_secs() % 3 == 0 {
                        let sm = service_manager_clone.read();
                        if let Ok(service_list) = sm.list_services() {
                            *services_clone.write() = service_list;
                        }
                    }
                }
            });
        });

        Self {
            monitor,
            detector,
            partition_manager,
            service_manager,
            system_metrics,
            processes,
            disks,
            services,
            alerts,
            selected_tab: 0,
            sort_by_cpu: true,
            selected_process: None,
            selected_process_pid: None,
            show_process_context_menu: false,
            context_menu_pos: egui::Pos2::ZERO,
            selected_disk: None,
            selected_partition: None,
            status_message: String::new(),
            show_format_dialog: false,
            format_filesystem: "ext4".to_string(),
            show_delete_confirm: false,
        }
    }

    fn draw_dashboard(&mut self, ui: &mut egui::Ui) {
        let metrics = self.system_metrics.read();

        ui.heading("System Overview");
        ui.add_space(10.0);

        egui::Grid::new("system_metrics")
            .num_columns(2)
            .spacing([40.0, 10.0])
            .show(ui, |ui| {
                ui.label("CPU Usage:");
                ui.add(
                    egui::ProgressBar::new(metrics.cpu.total_usage / 100.0)
                        .text(format!("{:.1}%", metrics.cpu.total_usage)),
                );
                ui.end_row();

                let mem_percent = metrics.memory.used as f64 / metrics.memory.total as f64;
                ui.label("Memory Usage:");
                ui.add(
                    egui::ProgressBar::new(mem_percent as f32)
                        .text(format!(
                            "{:.1} / {:.1} GB",
                            metrics.memory.used as f64 / (1024.0 * 1024.0 * 1024.0),
                            metrics.memory.total as f64 / (1024.0 * 1024.0 * 1024.0)
                        )),
                );
                ui.end_row();

                ui.label("CPU Temperature:");
                if let Some(temp) = metrics.cpu.temperature {
                    ui.label(format!("{:.1}°C", temp));
                } else {
                    ui.label("N/A");
                }
                ui.end_row();
            });

        ui.add_space(20.0);
        ui.heading("CPU Core Usage");
        ui.add_space(10.0);

        let bar_width = 30.0;
        let bar_spacing = 5.0;
        let num_cores = metrics.cpu.per_core_usage.len();
        let chart_height = 150.0;

        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(
                (bar_width + bar_spacing) * num_cores as f32,
                chart_height,
            ),
            egui::Sense::hover(),
        );

        let rect = response.rect;

        for (i, usage) in metrics.cpu.per_core_usage.iter().enumerate() {
            let x = rect.left() + (bar_width + bar_spacing) * i as f32;
            let bar_height = (chart_height - 20.0) * (usage / 100.0);
            let y = rect.bottom() - bar_height - 20.0;

            let color = if *usage > 80.0 {
                egui::Color32::RED
            } else if *usage > 60.0 {
                egui::Color32::YELLOW
            } else {
                egui::Color32::GREEN
            };

            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::Pos2::new(x, y),
                    egui::Vec2::new(bar_width, bar_height),
                ),
                0.0,
                color,
            );

            painter.text(
                egui::Pos2::new(x + bar_width / 2.0, rect.bottom() - 10.0),
                egui::Align2::CENTER_CENTER,
                i.to_string(),
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );
        }

        if !metrics.gpus.is_empty() {
            ui.add_space(20.0);
            ui.heading("GPU Information");
            ui.add_space(10.0);

            for gpu in &metrics.gpus {
                ui.group(|ui| {
                    ui.label(format!("Name: {}", gpu.name));
                    ui.add(
                        egui::ProgressBar::new(gpu.usage / 100.0)
                            .text(format!("{:.1}% usage", gpu.usage)),
                    );
                    if gpu.memory_total > 0 {
                        ui.label(format!(
                            "VRAM: {:.1} / {:.1} GB",
                            gpu.memory_used as f64 / (1024.0 * 1024.0 * 1024.0),
                            gpu.memory_total as f64 / (1024.0 * 1024.0 * 1024.0)
                        ));
                    }
                    if let Some(temp) = gpu.temperature {
                        ui.label(format!("Temperature: {:.1}°C", temp));
                    }
                });
            }
        }
    }

    fn draw_processes(&mut self, ui: &mut egui::Ui) {
        ui.heading("Processes");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Sort by:");
            if ui.selectable_label(self.sort_by_cpu, "CPU").clicked() {
                self.sort_by_cpu = true;
            }
            if ui.selectable_label(!self.sort_by_cpu, "Memory").clicked() {
                self.sort_by_cpu = false;
            }
        });

        ui.add_space(10.0);

        let mut processes = self.processes.read().clone();

        if self.sort_by_cpu {
            processes.sort_by(|a, b| b.stats.cpu_usage.partial_cmp(&a.stats.cpu_usage).unwrap());
        } else {
            processes.sort_by(|a, b| b.stats.memory_usage.cmp(&a.stats.memory_usage));
        }

        // Header
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("PID").strong().size(14.0));
            ui.add_space(20.0);
            ui.label(egui::RichText::new("Name").strong().size(14.0));
            ui.add_space(120.0);
            ui.label(egui::RichText::new("User").strong().size(14.0));
            ui.add_space(60.0);
            ui.label(egui::RichText::new("CPU %").strong().size(14.0));
            ui.add_space(40.0);
            ui.label(egui::RichText::new("Memory (MB)").strong().size(14.0));
            ui.add_space(40.0);
            ui.label(egui::RichText::new("Disk I/O (MB)").strong().size(14.0));
            ui.add_space(40.0);
            ui.label(egui::RichText::new("Status").strong().size(14.0));
        });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, process) in processes.iter().take(100).enumerate() {
                let is_selected = self.selected_process == Some(i);

                // Create a single clickable row
                let row_text = format!(
                    "{:<8} {:<20} {:<12} {:>6.1} {:>12.1} {:>12.1} {:?}",
                    process.info.pid,
                    if process.info.name.len() > 20 {
                        format!("{}...", &process.info.name[..17])
                    } else {
                        process.info.name.clone()
                    },
                    if process.info.user.len() > 12 {
                        format!("{}...", &process.info.user[..9])
                    } else {
                        process.info.user.clone()
                    },
                    process.stats.cpu_usage,
                    process.stats.memory_usage as f64 / (1024.0 * 1024.0),
                    (process.stats.disk_read_bytes + process.stats.disk_write_bytes) as f64 / (1024.0 * 1024.0),
                    process.info.status
                );

                let response = ui.selectable_label(is_selected, egui::RichText::new(row_text).monospace());

                if response.clicked() {
                    self.selected_process = Some(i);
                    self.selected_process_pid = Some(process.info.pid);
                }

                response.context_menu(|ui| {
                    self.selected_process_pid = Some(process.info.pid);

                    if ui.button("Kill Process").clicked() {
                        self.kill_process(process.info.pid);
                        ui.close_menu();
                    }
                    if ui.button("Kill Process Tree").clicked() {
                        self.kill_process_tree(process.info.pid);
                        ui.close_menu();
                    }
                    if ui.button("Open Process Folder").clicked() {
                        if let Some(ref exe_path) = process.info.exe_path {
                            if let Some(parent) = exe_path.parent() {
                                let _ = std::process::Command::new("xdg-open")
                                    .arg(parent)
                                    .spawn();
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Restart Process").clicked() {
                        self.restart_process(process.info.pid, &process.info.exe_path, &process.info.command_line);
                        ui.close_menu();
                    }
                });
            }
        });
    }

    fn draw_services_redesigned(&mut self, ui: &mut egui::Ui) {
        ui.heading("Services");
        ui.add_space(10.0);

        let mut services = self.services.read().clone();
        services.sort_by(|a, b| a.name.cmp(&b.name));

        // Header
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Name").strong().size(14.0));
            ui.add_space(150.0);
            ui.label(egui::RichText::new("State").strong().size(14.0));
            ui.add_space(60.0);
            ui.label(egui::RichText::new("Enabled").strong().size(14.0));
            ui.add_space(40.0);
            ui.label(egui::RichText::new("PID").strong().size(14.0));
            ui.add_space(60.0);
            ui.label(egui::RichText::new("Memory (MB)").strong().size(14.0));
            ui.add_space(40.0);
            ui.label(egui::RichText::new("Description").strong().size(14.0));
        });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for service in services.iter() {
                // Determine state color
                let state_color = match service.state {
                    ServiceState::Running => egui::Color32::GREEN,
                    ServiceState::Failed => egui::Color32::RED,
                    ServiceState::Stopped => egui::Color32::GRAY,
                    ServiceState::Unknown => egui::Color32::YELLOW,
                };

                let row_text = format!(
                    "{:<30} {:>10} {:>8} {:>10} {:>12} {}",
                    if service.name.len() > 30 {
                        format!("{}...", &service.name[..27])
                    } else {
                        service.name.clone()
                    },
                    service.sub_state,
                    if service.enabled { "Yes" } else { "No" },
                    service.main_pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string()),
                    service.memory_usage.map(|m| format!("{:.1}", m as f64 / (1024.0 * 1024.0))).unwrap_or_else(|| "-".to_string()),
                    if service.description.len() > 40 {
                        format!("{}...", &service.description[..37])
                    } else {
                        service.description.clone()
                    }
                );

                let response = ui.horizontal(|ui| {
                    ui.colored_label(state_color, "●");
                    ui.label(egui::RichText::new(row_text).monospace())
                }).response;

                response.context_menu(|ui| {
                    let service_name = service.name.clone();

                    if ui.button("Start").clicked() {
                        let sm = self.service_manager.read();
                        match sm.start_service(&service_name) {
                            Ok(_) => self.status_message = format!("Started service: {}", service_name),
                            Err(e) => self.status_message = format!("Failed to start {}: {}", service_name, e),
                        }
                        ui.close_menu();
                    }

                    if ui.button("Stop").clicked() {
                        let sm = self.service_manager.read();
                        match sm.stop_service(&service_name) {
                            Ok(_) => self.status_message = format!("Stopped service: {}", service_name),
                            Err(e) => self.status_message = format!("Failed to stop {}: {}", service_name, e),
                        }
                        ui.close_menu();
                    }

                    if ui.button("Restart").clicked() {
                        let sm = self.service_manager.read();
                        match sm.restart_service(&service_name) {
                            Ok(_) => self.status_message = format!("Restarted service: {}", service_name),
                            Err(e) => self.status_message = format!("Failed to restart {}: {}", service_name, e),
                        }
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("Enable").clicked() {
                        let sm = self.service_manager.read();
                        match sm.enable_service(&service_name) {
                            Ok(_) => self.status_message = format!("Enabled service: {}", service_name),
                            Err(e) => self.status_message = format!("Failed to enable {}: {}", service_name, e),
                        }
                        ui.close_menu();
                    }

                    if ui.button("Disable").clicked() {
                        let sm = self.service_manager.read();
                        match sm.disable_service(&service_name) {
                            Ok(_) => self.status_message = format!("Disabled service: {}", service_name),
                            Err(e) => self.status_message = format!("Failed to disable {}: {}", service_name, e),
                        }
                        ui.close_menu();
                    }
                });
            }
        });
    }

    fn kill_process(&mut self, pid: u32) {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .output();
        self.status_message = format!("Sent kill signal to PID {}", pid);
    }

    fn kill_process_tree(&mut self, pid: u32) {
        let _ = std::process::Command::new("kill")
            .arg("-TERM")
            .arg("--")
            .arg(format!("-{}", pid))
            .output();
        self.status_message = format!("Sent kill signal to PID {} and children", pid);
    }

    fn restart_process(&mut self, pid: u32, exe_path: &Option<std::path::PathBuf>, cmd_line: &[String]) {
        // Kill first
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .output();

        // Wait a bit
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Restart
        if let Some(exe) = exe_path {
            let mut command = std::process::Command::new(exe);
            if cmd_line.len() > 1 {
                command.args(&cmd_line[1..]);
            }
            let _ = command.spawn();
            self.status_message = format!("Restarted PID {}", pid);
        } else {
            self.status_message = format!("Cannot restart PID {}: no executable path", pid);
        }
    }

    fn draw_partitions(&mut self, ui: &mut egui::Ui) {
        ui.heading("Partition Manager");
        ui.add_space(10.0);

        if !self.status_message.is_empty() {
            ui.colored_label(egui::Color32::YELLOW, &self.status_message);
            ui.add_space(10.0);
        }

        ui.horizontal(|ui| {
            if ui.button("Refresh Disks").clicked() {
                let pm = self.partition_manager.read();
                if let Ok(disk_list) = pm.list_disks() {
                    *self.disks.write() = disk_list;
                    self.status_message = "Disks refreshed".to_string();
                }
            }

            ui.label("(Requires root for full partition management)");
        });

        ui.add_space(15.0);

        let disks = self.disks.read().clone();

        if disks.is_empty() {
            ui.label("No disks found. Try running with sudo.");
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (disk_idx, disk) in disks.iter().enumerate() {
                let is_disk_selected = self.selected_disk == Some(disk_idx);

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let response = ui.selectable_label(
                            is_disk_selected,
                            format!("{} - {} ({:.2} GB)",
                                disk.device,
                                disk.model,
                                disk.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                            )
                        );
                        if response.clicked() {
                            self.selected_disk = Some(disk_idx);
                            self.selected_partition = None;
                        }
                    });

                    if is_disk_selected && !disk.partitions.is_empty() {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);

                        egui::Grid::new(format!("partitions_{}", disk_idx))
                            .num_columns(7)
                            .striped(true)
                            .spacing([10.0, 5.0])
                            .show(ui, |ui| {
                                ui.strong("Device");
                                ui.strong("Filesystem");
                                ui.strong("Label");
                                ui.strong("Size (GB)");
                                ui.strong("Used (GB)");
                                ui.strong("Mount Point");
                                ui.strong("Actions");
                                ui.end_row();

                                for (part_idx, partition) in disk.partitions.iter().enumerate() {
                                    let is_selected = self.selected_partition == Some(part_idx);

                                    let response = ui.selectable_label(is_selected, &partition.device);
                                    if response.clicked() {
                                        self.selected_partition = Some(part_idx);
                                    }

                                    ui.label(partition.filesystem.as_deref().unwrap_or("unknown"));
                                    ui.label(partition.label.as_deref().unwrap_or("-"));
                                    ui.label(format!("{:.2}", partition.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)));

                                    let used_gb = partition.used_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                                    let used_percent = if partition.size_bytes > 0 {
                                        (partition.used_bytes as f64 / partition.size_bytes as f64 * 100.0)
                                    } else {
                                        0.0
                                    };
                                    ui.label(format!("{:.2} ({:.1}%)", used_gb, used_percent));

                                    ui.label(partition.mount_point.as_deref().unwrap_or("-"));

                                    ui.horizontal(|ui| {
                                        if ui.button("Format").clicked() {
                                            self.show_format_dialog = true;
                                            self.selected_disk = Some(disk_idx);
                                            self.selected_partition = Some(part_idx);
                                        }

                                        if ui.button("Delete").clicked() {
                                            self.show_delete_confirm = true;
                                            self.selected_disk = Some(disk_idx);
                                            self.selected_partition = Some(part_idx);
                                        }

                                        if partition.filesystem.is_some() && ui.button("Check").clicked() {
                                            self.check_partition(disk_idx, part_idx);
                                        }
                                    });

                                    ui.end_row();
                                }
                            });
                    }
                });

                ui.add_space(10.0);
            }
        });

        // Format dialog
        if self.show_format_dialog {
            egui::Window::new("Format Partition")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label("WARNING: This will erase all data on the partition!");
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.label("Filesystem:");
                        egui::ComboBox::from_label("")
                            .selected_text(&self.format_filesystem)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.format_filesystem, "ext4".to_string(), "ext4");
                                ui.selectable_value(&mut self.format_filesystem, "ext3".to_string(), "ext3");
                                ui.selectable_value(&mut self.format_filesystem, "xfs".to_string(), "xfs");
                                ui.selectable_value(&mut self.format_filesystem, "btrfs".to_string(), "btrfs");
                                ui.selectable_value(&mut self.format_filesystem, "ntfs".to_string(), "ntfs");
                                ui.selectable_value(&mut self.format_filesystem, "fat32".to_string(), "fat32");
                                ui.selectable_value(&mut self.format_filesystem, "f2fs".to_string(), "f2fs");
                            });
                    });

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button("Format").clicked() {
                            self.format_partition();
                            self.show_format_dialog = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_format_dialog = false;
                        }
                    });
                });
        }

        // Delete confirmation
        if self.show_delete_confirm {
            egui::Window::new("Delete Partition")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label("Are you sure you want to delete this partition?");
                    ui.label("This action cannot be undone!");
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.delete_partition();
                            self.show_delete_confirm = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_delete_confirm = false;
                        }
                    });
                });
        }
    }

    fn format_partition(&mut self) {
        if let (Some(disk_idx), Some(part_idx)) = (self.selected_disk, self.selected_partition) {
            let disks = self.disks.read();
            if let Some(disk) = disks.get(disk_idx) {
                if let Some(partition) = disk.partitions.get(part_idx) {
                    let pm = self.partition_manager.read();
                    match pm.format_partition(&partition.device, &self.format_filesystem, None) {
                        Ok(_) => {
                            self.status_message = format!(
                                "Successfully formatted {} as {}",
                                partition.device, self.format_filesystem
                            );
                        }
                        Err(e) => {
                            self.status_message = format!("Format failed: {}", e);
                        }
                    }
                }
            }
        }
    }

    fn delete_partition(&mut self) {
        if let (Some(disk_idx), Some(part_idx)) = (self.selected_disk, self.selected_partition) {
            let disks = self.disks.read();
            if let Some(disk) = disks.get(disk_idx) {
                if let Some(partition) = disk.partitions.get(part_idx) {
                    if let Some(part_num) = partition.partition_number {
                        let pm = self.partition_manager.read();
                        match pm.delete_partition(&disk.device, part_num) {
                            Ok(_) => {
                                self.status_message = format!("Deleted partition {}", partition.device);
                            }
                            Err(e) => {
                                self.status_message = format!("Delete failed: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    fn check_partition(&mut self, disk_idx: usize, part_idx: usize) {
        let disks = self.disks.read();
        if let Some(disk) = disks.get(disk_idx) {
            if let Some(partition) = disk.partitions.get(part_idx) {
                if let Some(ref fs) = partition.filesystem {
                    let pm = self.partition_manager.read();
                    match pm.check_filesystem(&partition.device, fs, false) {
                        Ok(_) => {
                            self.status_message = format!("Filesystem check completed for {}", partition.device);
                        }
                        Err(e) => {
                            self.status_message = format!("Check failed: {}", e);
                        }
                    }
                }
            }
        }
    }

    fn draw_storage(&mut self, ui: &mut egui::Ui) {
        let metrics = self.system_metrics.read();
        let processes = self.processes.read().clone();

        ui.heading("Storage & Disk I/O");
        ui.add_space(10.0);

        // Disk I/O graphs
        ui.heading("Disk Usage");
        ui.add_space(10.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (name, disk_metrics) in &metrics.disk_io {
                if name.starts_with("loop") || name.starts_with("ram") {
                    continue;
                }

                ui.group(|ui| {
                    ui.strong(name);
                    ui.add_space(5.0);

                    // Read/Write bars
                    ui.horizontal(|ui| {
                        ui.label("Read:");
                        let read_mb = disk_metrics.read_bytes as f64 / (1024.0 * 1024.0);
                        ui.add(egui::ProgressBar::new(((read_mb / 10000.0).min(1.0)) as f32)
                            .text(format!("{:.2} MB ({} ops)", read_mb, disk_metrics.read_ops)));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Write:");
                        let write_mb = disk_metrics.write_bytes as f64 / (1024.0 * 1024.0);
                        ui.add(egui::ProgressBar::new(((write_mb / 10000.0).min(1.0)) as f32)
                            .text(format!("{:.2} MB ({} ops)", write_mb, disk_metrics.write_ops)));
                    });
                });
                ui.add_space(10.0);
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Process list sorted by disk I/O
            ui.heading("Top Processes by Disk I/O");
            ui.add_space(10.0);

            let mut sorted_processes = processes.clone();
            sorted_processes.sort_by(|a, b| {
                let a_io = a.stats.disk_read_bytes + a.stats.disk_write_bytes;
                let b_io = b.stats.disk_read_bytes + b.stats.disk_write_bytes;
                b_io.cmp(&a_io)
            });

            egui::Grid::new("disk_io_processes")
                .num_columns(5)
                .striped(true)
                .spacing([10.0, 5.0])
                .show(ui, |ui| {
                    ui.strong("PID");
                    ui.strong("Name");
                    ui.strong("User");
                    ui.strong("Read (MB)");
                    ui.strong("Write (MB)");
                    ui.end_row();

                    for process in sorted_processes.iter().take(20) {
                        let read_mb = process.stats.disk_read_bytes as f64 / (1024.0 * 1024.0);
                        let write_mb = process.stats.disk_write_bytes as f64 / (1024.0 * 1024.0);

                        // Only show processes with significant disk I/O
                        if read_mb < 0.01 && write_mb < 0.01 {
                            continue;
                        }

                        ui.label(process.info.pid.to_string());
                        ui.label(&process.info.name);
                        ui.label(&process.info.user);
                        ui.label(format!("{:.2}", read_mb));
                        ui.label(format!("{:.2}", write_mb));
                        ui.end_row();
                    }
                });
        });
    }

    fn draw_network_redesigned(&mut self, ui: &mut egui::Ui) {
        let metrics = self.system_metrics.read();
        let processes = self.processes.read().clone();

        ui.heading("Network Interfaces & Usage");
        ui.add_space(10.0);

        // Network interface statistics
        ui.heading("Network Interfaces");
        ui.add_space(10.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (name, net_metrics) in &metrics.network {
                ui.group(|ui| {
                    ui.strong(name);
                    ui.add_space(5.0);

                    // Received/Sent bars
                    ui.horizontal(|ui| {
                        ui.label("Received:");
                        let recv_mb = net_metrics.bytes_received as f64 / (1024.0 * 1024.0);
                        ui.add(egui::ProgressBar::new(((recv_mb / 10000.0).min(1.0)) as f32)
                            .text(format!("{:.2} MB ({} packets)", recv_mb, net_metrics.packets_received)));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Sent:");
                        let sent_mb = net_metrics.bytes_sent as f64 / (1024.0 * 1024.0);
                        ui.add(egui::ProgressBar::new(((sent_mb / 10000.0).min(1.0)) as f32)
                            .text(format!("{:.2} MB ({} packets)", sent_mb, net_metrics.packets_sent)));
                    });

                    if net_metrics.errors_in > 0 || net_metrics.errors_out > 0 {
                        ui.colored_label(
                            egui::Color32::RED,
                            format!("Errors: In={} Out={}", net_metrics.errors_in, net_metrics.errors_out)
                        );
                    }
                });
                ui.add_space(10.0);
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Process list sorted by network usage
            ui.heading("Top Processes by Network Usage");
            ui.add_space(10.0);

            let mut sorted_processes = processes.clone();
            sorted_processes.sort_by(|a, b| {
                let a_net = a.stats.network_rx_bytes + a.stats.network_tx_bytes;
                let b_net = b.stats.network_rx_bytes + b.stats.network_tx_bytes;
                b_net.cmp(&a_net)
            });

            egui::Grid::new("network_processes")
                .num_columns(5)
                .striped(true)
                .spacing([10.0, 5.0])
                .show(ui, |ui| {
                    ui.strong("PID");
                    ui.strong("Name");
                    ui.strong("User");
                    ui.strong("RX (MB)");
                    ui.strong("TX (MB)");
                    ui.end_row();

                    for process in sorted_processes.iter().take(20) {
                        let rx_mb = process.stats.network_rx_bytes as f64 / (1024.0 * 1024.0);
                        let tx_mb = process.stats.network_tx_bytes as f64 / (1024.0 * 1024.0);

                        // Only show processes with network activity
                        if rx_mb < 0.01 && tx_mb < 0.01 {
                            continue;
                        }

                        ui.label(process.info.pid.to_string());
                        ui.label(&process.info.name);
                        ui.label(&process.info.user);
                        ui.label(format!("{:.2}", rx_mb));
                        ui.label(format!("{:.2}", tx_mb));
                        ui.end_row();
                    }
                });
        });
    }

    fn draw_alerts(&mut self, ui: &mut egui::Ui) {
        let alerts = self.alerts.read();

        ui.heading(format!("Alerts ({})", alerts.len()));
        ui.add_space(10.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for alert in alerts.iter().rev().take(50) {
                let color = match alert.severity {
                    Severity::Critical => egui::Color32::RED,
                    Severity::Warning => egui::Color32::YELLOW,
                    Severity::Info => egui::Color32::LIGHT_BLUE,
                };

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(color, format!("[{:?}]", alert.severity));
                        ui.label(format!(
                            "{} - {} (PID: {})",
                            alert.timestamp.format("%H:%M:%S"),
                            alert.process_name,
                            alert.pid
                        ));
                    });
                    ui.label(format!("{}: {}", alert.rule_name, alert.details));
                });
                ui.add_space(5.0);
            }
        });
    }
}

impl eframe::App for ProcessMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.selected_tab, 0, "Dashboard");
                ui.selectable_value(&mut self.selected_tab, 1, "Processes");
                ui.selectable_value(&mut self.selected_tab, 2, "Services");
                ui.selectable_value(&mut self.selected_tab, 3, "Storage");
                ui.selectable_value(&mut self.selected_tab, 4, "Network");
                ui.selectable_value(&mut self.selected_tab, 5, "Partitions");
                ui.selectable_value(&mut self.selected_tab, 6, "Alerts");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.selected_tab {
                0 => self.draw_dashboard(ui),
                1 => self.draw_processes(ui),
                2 => self.draw_services_redesigned(ui),
                3 => self.draw_storage(ui),
                4 => self.draw_network_redesigned(ui),
                5 => self.draw_partitions(ui),
                6 => self.draw_alerts(ui),
                _ => {}
            }
        });
    }
}
