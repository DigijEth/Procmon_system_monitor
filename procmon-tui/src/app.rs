use anyhow::Result;
use procmon_core::{
    MisbehaviorDetector, SystemMetrics, SystemMonitor,
    process::ProcessSnapshot,
    ServiceManager, SystemService,
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Processes,
    Services,
    Storage,
    Network,
    Partitions,
    Alerts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Cpu,
    Memory,
    DiskIo,
    User,
}

pub struct App {
    pub monitor: SystemMonitor,
    pub detector: MisbehaviorDetector,
    pub partition_manager: procmon_core::PartitionManager,
    pub service_manager: ServiceManager,
    pub system_metrics: SystemMetrics,
    pub processes: Vec<ProcessSnapshot>,
    pub filtered_processes: Vec<ProcessSnapshot>,
    pub services: Vec<SystemService>,
    pub filtered_services: Vec<SystemService>,
    pub disks: Vec<procmon_core::Disk>,
    pub alerts: Vec<procmon_core::MisbehaviorAlert>,
    pub current_tab: Tab,
    pub selected_process: usize,
    pub selected_service: usize,
    pub selected_disk: usize,
    pub selected_partition: usize,
    pub sort_column: SortColumn,
    pub sort_ascending: bool,
    pub show_only_misbehaving: bool,
    pub show_context_menu: bool,
    pub show_service_menu: bool,
    pub show_partition_menu: bool,
    pub context_menu_pid: Option<u32>,
    pub context_menu_service: Option<String>,
    pub status_message: Option<String>,
    pub search_query: String,
    pub search_mode: bool,
    pub scroll_offset: usize,
    pub process_list_area: Option<(u16, u16, u16, u16)>, // (x, y, width, height) for process table
    last_update: Instant,
    update_interval: Duration,
    last_click_time: Option<Instant>,
    last_click_row: Option<usize>,
}

impl App {
    pub async fn new() -> Result<Self> {
        let monitor = SystemMonitor::new();
        let detector = MisbehaviorDetector::new();
        let partition_manager = procmon_core::PartitionManager::new();
        let service_manager = ServiceManager::new();

        monitor.refresh();
        let system_metrics = monitor.get_system_metrics()?;
        let processes = monitor.get_all_processes()?;
        let disks = partition_manager.list_disks().unwrap_or_default();
        let services = service_manager.list_services().unwrap_or_default();

        let filtered_processes = processes.clone();
        let filtered_services = services.clone();

        Ok(Self {
            monitor,
            detector,
            partition_manager,
            service_manager,
            system_metrics,
            processes,
            filtered_processes,
            services,
            filtered_services,
            disks,
            alerts: Vec::new(),
            current_tab: Tab::Dashboard,
            selected_process: 0,
            selected_service: 0,
            selected_disk: 0,
            selected_partition: 0,
            sort_column: SortColumn::Cpu,
            sort_ascending: false,
            show_only_misbehaving: false,
            show_context_menu: false,
            show_service_menu: false,
            show_partition_menu: false,
            context_menu_pid: None,
            context_menu_service: None,
            status_message: None,
            search_query: String::new(),
            search_mode: false,
            scroll_offset: 0,
            process_list_area: None,
            last_update: Instant::now(),
            update_interval: Duration::from_millis(1000),
            last_click_time: None,
            last_click_row: None,
        })
    }

    pub fn handle_mouse_click(&mut self, x: u16, y: u16) {
        // Check if click is within process list area
        if let Some((area_x, area_y, area_width, area_height)) = self.process_list_area {
            if x >= area_x && x < area_x + area_width && y >= area_y && y < area_y + area_height {
                // Calculate which row was clicked (accounting for borders and header)
                // area_y is the top of the block, +1 for border, +2 for header with spacing
                let header_offset = 3; // border + header + spacing
                if y >= area_y + header_offset {
                    let clicked_row = (y - area_y - header_offset) as usize;
                    let actual_index = clicked_row + self.scroll_offset;

                    if actual_index < self.filtered_processes.len() {
                        // Check for double-click (within 500ms)
                        let now = Instant::now();
                        let is_double_click = if let (Some(last_time), Some(last_row)) = (self.last_click_time, self.last_click_row) {
                            now.duration_since(last_time) < Duration::from_millis(500) && last_row == actual_index
                        } else {
                            false
                        };

                        self.selected_process = actual_index;

                        if is_double_click {
                            // Double-click opens context menu
                            self.toggle_context_menu();
                            self.last_click_time = None;
                            self.last_click_row = None;
                        } else {
                            // Single click just selects
                            self.last_click_time = Some(now);
                            self.last_click_row = Some(actual_index);
                        }
                    }
                }
            }
        }
    }

    pub fn set_process_list_area(&mut self, x: u16, y: u16, width: u16, height: u16) {
        self.process_list_area = Some((x, y, width, height));
    }

    pub fn toggle_search_mode(&mut self) {
        self.search_mode = !self.search_mode;
        if !self.search_mode {
            self.search_query.clear();
            self.filter_processes();
        }
    }

    pub fn add_search_char(&mut self, c: char) {
        self.search_query.push(c);
        self.filter_processes();
        self.selected_process = 0;
        self.scroll_offset = 0;
    }

    pub fn remove_search_char(&mut self) {
        self.search_query.pop();
        self.filter_processes();
        self.selected_process = 0;
        self.scroll_offset = 0;
    }

    fn filter_processes(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_processes = self.processes.clone();
        } else {
            let query_lower = self.search_query.to_lowercase();
            self.filtered_processes = self.processes
                .iter()
                .filter(|p| {
                    p.info.name.to_lowercase().contains(&query_lower)
                        || p.info.pid.to_string().contains(&query_lower)
                        || p.info.user.to_lowercase().contains(&query_lower)
                })
                .cloned()
                .collect();
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        if self.scroll_offset >= amount {
            self.scroll_offset -= amount;
        } else {
            self.scroll_offset = 0;
        }
    }

    pub fn scroll_down(&mut self, amount: usize, max_visible: usize) {
        let max_scroll = self.filtered_processes.len().saturating_sub(max_visible);
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
    }

    pub fn get_filtered_processes(&self) -> &[ProcessSnapshot] {
        &self.filtered_processes
    }

    pub fn next_disk(&mut self) {
        if !self.disks.is_empty() {
            self.selected_disk = (self.selected_disk + 1) % self.disks.len();
            self.selected_partition = 0;
        }
    }

    pub fn previous_disk(&mut self) {
        if !self.disks.is_empty() {
            if self.selected_disk == 0 {
                self.selected_disk = self.disks.len() - 1;
            } else {
                self.selected_disk -= 1;
            }
            self.selected_partition = 0;
        }
    }

    pub fn next_partition(&mut self) {
        if self.selected_disk < self.disks.len() {
            let partitions = &self.disks[self.selected_disk].partitions;
            if !partitions.is_empty() {
                self.selected_partition = (self.selected_partition + 1) % partitions.len();
            }
        }
    }

    pub fn previous_partition(&mut self) {
        if self.selected_disk < self.disks.len() {
            let partitions = &self.disks[self.selected_disk].partitions;
            if !partitions.is_empty() {
                if self.selected_partition == 0 {
                    self.selected_partition = partitions.len() - 1;
                } else {
                    self.selected_partition -= 1;
                }
            }
        }
    }

    pub fn toggle_partition_menu(&mut self) {
        self.show_partition_menu = !self.show_partition_menu;
    }

    pub fn refresh_disks(&mut self) {
        if let Ok(disks) = self.partition_manager.list_disks() {
            self.disks = disks;
            self.status_message = Some("Disk list refreshed".to_string());
        } else {
            self.status_message = Some("Failed to refresh disks".to_string());
        }
    }

    pub fn format_selected_partition(&mut self, filesystem: &str) -> Result<()> {
        if self.selected_disk >= self.disks.len() {
            self.status_message = Some("No disk selected".to_string());
            return Ok(());
        }

        let disk = &self.disks[self.selected_disk];
        if self.selected_partition >= disk.partitions.len() {
            self.status_message = Some("No partition selected".to_string());
            return Ok(());
        }

        let partition = &disk.partitions[self.selected_partition];
        let device = &partition.device;

        match self.partition_manager.format_partition(device, filesystem, None) {
            Ok(_) => {
                self.status_message = Some(format!("Formatted {} as {}", device, filesystem));
                self.refresh_disks();
            }
            Err(e) => {
                self.status_message = Some(format!("Format failed: {}", e));
            }
        }

        Ok(())
    }

    pub fn delete_selected_partition(&mut self) -> Result<()> {
        if self.selected_disk >= self.disks.len() {
            self.status_message = Some("No disk selected".to_string());
            return Ok(());
        }

        let disk = &self.disks[self.selected_disk];
        if self.selected_partition >= disk.partitions.len() {
            self.status_message = Some("No partition selected".to_string());
            return Ok(());
        }

        let partition = &disk.partitions[self.selected_partition];
        if let Some(part_num) = partition.partition_number {
            match self.partition_manager.delete_partition(&disk.device, part_num) {
                Ok(_) => {
                    self.status_message = Some(format!("Deleted partition {}", partition.device));
                    self.refresh_disks();
                }
                Err(e) => {
                    self.status_message = Some(format!("Delete failed: {}", e));
                }
            }
        } else {
            self.status_message = Some("Cannot determine partition number".to_string());
        }

        Ok(())
    }

    pub fn check_selected_partition(&mut self) -> Result<()> {
        if self.selected_disk >= self.disks.len() {
            self.status_message = Some("No disk selected".to_string());
            return Ok(());
        }

        let disk = &self.disks[self.selected_disk];
        if self.selected_partition >= disk.partitions.len() {
            self.status_message = Some("No partition selected".to_string());
            return Ok(());
        }

        let partition = &disk.partitions[self.selected_partition];
        if let Some(ref fs) = partition.filesystem {
            match self.partition_manager.check_filesystem(&partition.device, fs, false) {
                Ok(result) => {
                    self.status_message = Some(format!("Check complete. See logs for details."));
                }
                Err(e) => {
                    self.status_message = Some(format!("Check failed: {}", e));
                }
            }
        } else {
            self.status_message = Some("No filesystem detected".to_string());
        }

        Ok(())
    }

    pub async fn update(&mut self) -> Result<()> {
        if self.last_update.elapsed() >= self.update_interval {
            self.monitor.refresh();
            self.system_metrics = self.monitor.get_system_metrics()?;
            self.processes = self.monitor.get_all_processes()?;

            // Update services list
            if let Ok(services) = self.service_manager.list_services() {
                self.services = services;
                self.filtered_services = self.services.clone();
            }

            // Check for misbehaving processes
            let mut new_alerts = Vec::new();
            for process in &self.processes {
                let process_alerts = self.detector.check_process(process);
                new_alerts.extend(process_alerts);
            }

            // Keep only recent alerts (last 100)
            self.alerts.extend(new_alerts);
            if self.alerts.len() > 100 {
                self.alerts.drain(0..self.alerts.len() - 100);
            }

            // Cleanup detector state for dead processes
            let active_pids: Vec<u32> = self.processes.iter().map(|p| p.info.pid).collect();
            self.detector.cleanup_dead_processes(&active_pids);

            // Sort processes and apply filter
            self.sort_processes();
            self.filter_processes();

            self.last_update = Instant::now();
        }

        Ok(())
    }

    fn sort_processes(&mut self) {
        let ascending = self.sort_ascending;
        match self.sort_column {
            SortColumn::Name => {
                self.processes.sort_by(|a, b| {
                    if ascending {
                        a.info.name.cmp(&b.info.name)
                    } else {
                        b.info.name.cmp(&a.info.name)
                    }
                });
            }
            SortColumn::Cpu => {
                self.processes.sort_by(|a, b| {
                    if ascending {
                        a.stats.cpu_usage.partial_cmp(&b.stats.cpu_usage).unwrap()
                    } else {
                        b.stats.cpu_usage.partial_cmp(&a.stats.cpu_usage).unwrap()
                    }
                });
            }
            SortColumn::Memory => {
                self.processes.sort_by(|a, b| {
                    if ascending {
                        a.stats.memory_usage.cmp(&b.stats.memory_usage)
                    } else {
                        b.stats.memory_usage.cmp(&a.stats.memory_usage)
                    }
                });
            }
            SortColumn::DiskIo => {
                self.processes.sort_by(|a, b| {
                    let a_io = a.stats.disk_read_bytes + a.stats.disk_write_bytes;
                    let b_io = b.stats.disk_read_bytes + b.stats.disk_write_bytes;
                    if ascending {
                        a_io.cmp(&b_io)
                    } else {
                        b_io.cmp(&a_io)
                    }
                });
            }
            SortColumn::User => {
                self.processes.sort_by(|a, b| {
                    if ascending {
                        a.info.user.cmp(&b.info.user)
                    } else {
                        b.info.user.cmp(&a.info.user)
                    }
                });
            }
        }
    }

    pub fn next_process(&mut self) {
        if !self.filtered_processes.is_empty() {
            self.selected_process = (self.selected_process + 1) % self.filtered_processes.len();
            self.ensure_selected_visible();
        }
    }

    pub fn previous_process(&mut self) {
        if !self.filtered_processes.is_empty() {
            if self.selected_process == 0 {
                self.selected_process = self.filtered_processes.len() - 1;
            } else {
                self.selected_process -= 1;
            }
            self.ensure_selected_visible();
        }
    }

    fn ensure_selected_visible(&mut self) {
        // Assume visible area is around 20 rows (will be adjusted dynamically in UI)
        let visible_rows = 20;

        // If selected is below visible area, scroll down
        if self.selected_process >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selected_process.saturating_sub(visible_rows - 1);
        }

        // If selected is above visible area, scroll up
        if self.selected_process < self.scroll_offset {
            self.scroll_offset = self.selected_process;
        }
    }

    pub fn set_visible_rows(&mut self, rows: usize) {
        // This will be called from UI to set the actual visible area
        // For now we use a default in ensure_selected_visible
    }

    pub fn next_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Dashboard => Tab::Processes,
            Tab::Processes => Tab::Services,
            Tab::Services => Tab::Storage,
            Tab::Storage => Tab::Network,
            Tab::Network => Tab::Partitions,
            Tab::Partitions => Tab::Alerts,
            Tab::Alerts => Tab::Dashboard,
        };
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = match self.current_tab {
            Tab::Dashboard => Tab::Alerts,
            Tab::Processes => Tab::Dashboard,
            Tab::Services => Tab::Processes,
            Tab::Storage => Tab::Services,
            Tab::Network => Tab::Storage,
            Tab::Partitions => Tab::Network,
            Tab::Alerts => Tab::Partitions,
        };
    }

    pub fn set_tab(&mut self, index: usize) {
        self.current_tab = match index {
            0 => Tab::Dashboard,
            1 => Tab::Processes,
            2 => Tab::Services,
            3 => Tab::Storage,
            4 => Tab::Network,
            5 => Tab::Partitions,
            6 => Tab::Alerts,
            _ => self.current_tab,
        };
    }

    pub fn toggle_sort_ascending(&mut self) {
        self.sort_ascending = !self.sort_ascending;
        self.sort_processes();
    }

    pub fn next_sort_column(&mut self) {
        self.sort_column = match self.sort_column {
            SortColumn::Name => SortColumn::Cpu,
            SortColumn::Cpu => SortColumn::Memory,
            SortColumn::Memory => SortColumn::DiskIo,
            SortColumn::DiskIo => SortColumn::User,
            SortColumn::User => SortColumn::Name,
        };
        self.sort_processes();
    }

    pub fn toggle_filter(&mut self) {
        self.show_only_misbehaving = !self.show_only_misbehaving;
    }

    pub fn get_tab_index(&self) -> usize {
        match self.current_tab {
            Tab::Dashboard => 0,
            Tab::Processes => 1,
            Tab::Services => 2,
            Tab::Storage => 3,
            Tab::Network => 4,
            Tab::Partitions => 5,
            Tab::Alerts => 6,
        }
    }

    pub fn toggle_context_menu(&mut self) {
        if !self.filtered_processes.is_empty() && self.selected_process < self.filtered_processes.len() {
            self.show_context_menu = !self.show_context_menu;
            if self.show_context_menu {
                self.context_menu_pid = Some(self.filtered_processes[self.selected_process].info.pid);
            } else {
                self.context_menu_pid = None;
            }
        }
    }

    pub fn kill_process(&mut self) -> Result<()> {
        if let Some(pid) = self.context_menu_pid {
            use std::process::Command;
            Command::new("kill")
                .arg(pid.to_string())
                .output()?;
            self.show_context_menu = false;
            self.context_menu_pid = None;

            // Immediately refresh the process list
            self.monitor.refresh();
            self.processes = self.monitor.get_all_processes()?;
            self.sort_processes();
            self.filter_processes();
        }
        Ok(())
    }

    pub fn kill_process_tree(&mut self) -> Result<()> {
        if let Some(pid) = self.context_menu_pid {
            use std::process::Command;
            // Kill process and all children
            Command::new("kill")
                .arg("-TERM")
                .arg("--")
                .arg(format!("-{}", pid))
                .output()?;
            self.show_context_menu = false;
            self.context_menu_pid = None;

            // Immediately refresh the process list
            self.monitor.refresh();
            self.processes = self.monitor.get_all_processes()?;
            self.sort_processes();
            self.filter_processes();
        }
        Ok(())
    }

    pub fn open_process_folder(&mut self) -> Result<()> {
        if let Some(pid) = self.context_menu_pid {
            if let Some(process) = self.processes.iter().find(|p| p.info.pid == pid) {
                if let Some(exe_path) = &process.info.exe_path {
                    if let Some(parent) = exe_path.parent() {
                        use std::process::Command;
                        Command::new("xdg-open")
                            .arg(parent)
                            .spawn()?;
                    }
                }
            }
            self.show_context_menu = false;
            self.context_menu_pid = None;
        }
        Ok(())
    }

    pub fn restart_process(&mut self) -> Result<()> {
        if let Some(pid) = self.context_menu_pid {
            if let Some(process) = self.processes.iter().find(|p| p.info.pid == pid) {
                // Get the command line and executable path
                let exe_path = process.info.exe_path.clone();
                let cmd_line = process.info.command_line.clone();

                // Kill the process first
                use std::process::Command;
                Command::new("kill")
                    .arg(pid.to_string())
                    .output()?;

                // Wait a bit for the process to terminate
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Restart the process with the same command line
                if let Some(exe) = exe_path {
                    let mut command = Command::new(exe);
                    if cmd_line.len() > 1 {
                        // Skip the first argument (the executable itself)
                        command.args(&cmd_line[1..]);
                    }
                    command.spawn()?;
                }
            }
            self.show_context_menu = false;
            self.context_menu_pid = None;

            // Immediately refresh the process list
            self.monitor.refresh();
            self.processes = self.monitor.get_all_processes()?;
            self.sort_processes();
            self.filter_processes();
        }
        Ok(())
    }

    // Service navigation methods
    pub fn next_service(&mut self) {
        if !self.filtered_services.is_empty() {
            self.selected_service = (self.selected_service + 1) % self.filtered_services.len();
        }
    }

    pub fn previous_service(&mut self) {
        if !self.filtered_services.is_empty() {
            if self.selected_service == 0 {
                self.selected_service = self.filtered_services.len() - 1;
            } else {
                self.selected_service -= 1;
            }
        }
    }

    pub fn toggle_service_menu(&mut self) {
        if !self.filtered_services.is_empty() && self.selected_service < self.filtered_services.len() {
            self.show_service_menu = !self.show_service_menu;
            if self.show_service_menu {
                self.context_menu_service = Some(self.filtered_services[self.selected_service].name.clone());
            } else {
                self.context_menu_service = None;
            }
        }
    }

    // Service management methods
    pub fn start_service(&mut self) -> Result<()> {
        if let Some(ref service_name) = self.context_menu_service {
            self.service_manager.start_service(service_name)?;
            self.show_service_menu = false;
            self.context_menu_service = None;

            // Refresh service list
            if let Ok(services) = self.service_manager.list_services() {
                self.services = services;
                self.filtered_services = self.services.clone();
            }
        }
        Ok(())
    }

    pub fn stop_service(&mut self) -> Result<()> {
        if let Some(ref service_name) = self.context_menu_service {
            self.service_manager.stop_service(service_name)?;
            self.show_service_menu = false;
            self.context_menu_service = None;

            // Refresh service list
            if let Ok(services) = self.service_manager.list_services() {
                self.services = services;
                self.filtered_services = self.services.clone();
            }
        }
        Ok(())
    }

    pub fn restart_service(&mut self) -> Result<()> {
        if let Some(ref service_name) = self.context_menu_service {
            self.service_manager.restart_service(service_name)?;
            self.show_service_menu = false;
            self.context_menu_service = None;

            // Refresh service list
            if let Ok(services) = self.service_manager.list_services() {
                self.services = services;
                self.filtered_services = self.services.clone();
            }
        }
        Ok(())
    }

    pub fn enable_service(&mut self) -> Result<()> {
        if let Some(ref service_name) = self.context_menu_service {
            self.service_manager.enable_service(service_name)?;
            self.show_service_menu = false;
            self.context_menu_service = None;

            // Refresh service list
            if let Ok(services) = self.service_manager.list_services() {
                self.services = services;
                self.filtered_services = self.services.clone();
            }
        }
        Ok(())
    }

    pub fn disable_service(&mut self) -> Result<()> {
        if let Some(ref service_name) = self.context_menu_service {
            self.service_manager.disable_service(service_name)?;
            self.show_service_menu = false;
            self.context_menu_service = None;

            // Refresh service list
            if let Ok(services) = self.service_manager.list_services() {
                self.services = services;
                self.filtered_services = self.services.clone();
            }
        }
        Ok(())
    }
}
