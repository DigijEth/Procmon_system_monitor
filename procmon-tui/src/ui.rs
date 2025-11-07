use crate::app::{App, SortColumn, Tab};
use procmon_core::detector::Severity;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row,
        Table, Tabs,
    },
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    draw_tabs(f, app, chunks[0]);
    draw_main_content(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec![
        "Dashboard (1)",
        "Processes (2)",
        "Services (3)",
        "Storage (4)",
        "Network (5)",
        "Partitions (6)",
        "Alerts (7)"
    ];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Process Monitor with Partition Manager"))
        .select(app.get_tab_index())
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

fn draw_main_content(f: &mut Frame, app: &mut App, area: Rect) {
    match app.current_tab {
        Tab::Dashboard => draw_dashboard(f, app, area),
        Tab::Processes => draw_processes(f, app, area),
        Tab::Services => draw_services(f, app, area),
        Tab::Storage => draw_storage(f, app, area),
        Tab::Network => draw_network(f, app, area),
        Tab::Partitions => draw_partitions(f, app, area),
        Tab::Alerts => draw_alerts(f, app, area),
    }
}

fn draw_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Min(0),
        ])
        .split(area);

    draw_system_overview(f, app, chunks[0]);
    draw_cpu_cores(f, app, chunks[1]);
    draw_top_processes(f, app, chunks[2]);
}

fn draw_system_overview(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    // CPU Usage
    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("CPU Usage"))
        .gauge_style(Style::default().fg(get_usage_color(app.system_metrics.cpu.total_usage)))
        .percent(app.system_metrics.cpu.total_usage as u16)
        .label(format!("{:.1}%", app.system_metrics.cpu.total_usage));
    f.render_widget(cpu_gauge, chunks[0]);

    // Memory Usage
    let mem_percent = (app.system_metrics.memory.used as f64 / app.system_metrics.memory.total as f64 * 100.0) as u16;
    let mem_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Memory"))
        .gauge_style(Style::default().fg(get_usage_color(mem_percent as f32)))
        .percent(mem_percent)
        .label(format!(
            "{:.1} / {:.1} GB",
            app.system_metrics.memory.used as f64 / (1024.0 * 1024.0 * 1024.0),
            app.system_metrics.memory.total as f64 / (1024.0 * 1024.0 * 1024.0)
        ));
    f.render_widget(mem_gauge, chunks[1]);

    // CPU Temperature
    let temp_text = if let Some(temp) = app.system_metrics.cpu.temperature {
        format!("{:.1}°C", temp)
    } else {
        "N/A".to_string()
    };
    let temp_color = app.system_metrics.cpu.temperature
        .map(|t| {
            if t > 80.0 {
                Color::Red
            } else if t > 60.0 {
                Color::Yellow
            } else {
                Color::Green
            }
        })
        .unwrap_or(Color::Gray);
    let temp_para = Paragraph::new(temp_text)
        .block(Block::default().borders(Borders::ALL).title("CPU Temp"))
        .style(Style::default().fg(temp_color))
        .alignment(Alignment::Center);
    f.render_widget(temp_para, chunks[2]);

    // GPU Info
    let gpu_text = if let Some(gpu) = app.system_metrics.gpus.first() {
        format!("{}\n{:.1}%", gpu.name, gpu.usage)
    } else {
        "No GPU\nDetected".to_string()
    };
    let gpu_para = Paragraph::new(gpu_text)
        .block(Block::default().borders(Borders::ALL).title("GPU"))
        .alignment(Alignment::Center);
    f.render_widget(gpu_para, chunks[3]);
}

fn draw_cpu_cores(f: &mut Frame, app: &App, area: Rect) {
    let core_data: Vec<(&str, u64)> = app.system_metrics.cpu.per_core_usage
        .iter()
        .enumerate()
        .map(|(i, usage)| {
            (Box::leak(format!("{}", i).into_boxed_str()) as &str, *usage as u64)
        })
        .collect();

    let bars: Vec<Bar> = core_data
        .iter()
        .map(|(label, value)| {
            Bar::default()
                .value(*value)
                .label(Line::from(*label))
                .style(Style::default().fg(get_usage_color(*value as f32)))
        })
        .collect();

    let chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title("CPU Cores"))
        .data(BarGroup::default().bars(&bars))
        .bar_width(3)
        .bar_gap(1);

    f.render_widget(chart, area);
}

fn draw_top_processes(f: &mut Frame, app: &App, area: Rect) {
    let mut processes = app.processes.clone();
    processes.sort_by(|a, b| b.stats.cpu_usage.partial_cmp(&a.stats.cpu_usage).unwrap());
    processes.truncate(10);

    let rows: Vec<Row> = processes
        .iter()
        .map(|p| {
            Row::new(vec![
                Cell::from(p.info.pid.to_string()),
                Cell::from(p.info.name.clone()),
                Cell::from(p.info.user.clone()),
                Cell::from(format!("{:.1}%", p.stats.cpu_usage)),
                Cell::from(format!("{:.1} MB", p.stats.memory_usage as f64 / (1024.0 * 1024.0))),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec!["PID", "Name", "User", "CPU", "Memory"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1),
    )
    .block(Block::default().borders(Borders::ALL).title("Top Processes by CPU"));

    f.render_widget(table, area);
}

fn draw_processes(f: &mut Frame, app: &mut App, area: Rect) {
    use ratatui::widgets::TableState;

    // Split area for search bar if needed
    let (main_area, search_area) = if app.search_mode {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    // Store the area for mouse click handling
    app.set_process_list_area(main_area.x, main_area.y, main_area.width, main_area.height);

    let sort_indicator = if app.sort_ascending { "↑" } else { "↓" };
    let sort_column_name = match app.sort_column {
        SortColumn::Name => "Name",
        SortColumn::Cpu => "CPU",
        SortColumn::Memory => "Memory",
        SortColumn::DiskIo => "Disk I/O",
        SortColumn::User => "User",
    };

    let filtered_procs = app.get_filtered_processes();

    let rows: Vec<Row> = filtered_procs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            Row::new(vec![
                Cell::from(p.info.pid.to_string()),
                Cell::from(p.info.name.clone()),
                Cell::from(p.info.user.clone()),
                Cell::from(format!("{:.1}%", p.stats.cpu_usage)),
                Cell::from(format!("{:.1}", p.stats.memory_usage as f64 / (1024.0 * 1024.0))),
                Cell::from(format!("{:.1}", (p.stats.disk_read_bytes + p.stats.disk_write_bytes) as f64 / (1024.0 * 1024.0))),
                Cell::from(format!("{:?}", p.info.status)),
            ])
        })
        .collect();

    let title = if app.search_mode {
        format!("Processes ({}) - Search Mode Active", filtered_procs.len())
    } else {
        format!("Processes ({}) - Sort: {} {} - ↑↓: Select, Enter: Menu, /: Search",
            filtered_procs.len(), sort_column_name, sort_indicator)
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["PID", "Name", "User", "CPU %", "Mem (MB)", "Disk (MB)", "Status"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    )
    .highlight_symbol(">> ");

    // Create table state and set selected
    let mut table_state = TableState::default();
    table_state.select(Some(app.selected_process));

    f.render_stateful_widget(table, main_area, &mut table_state);

    // Draw search bar if in search mode
    if let Some(search_area) = search_area {
        let search_text = format!("Search: {}", app.search_query);
        let search_bar = Paragraph::new(search_text)
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("Search (ESC to exit)"));
        f.render_widget(search_bar, search_area);
    }

    // Draw context menu if active
    if app.show_context_menu {
        draw_context_menu(f, app);
    }
}

fn draw_context_menu(f: &mut Frame, app: &App) {
    // Create a centered popup
    let area = f.area();
    let popup_width = 40;
    let popup_height = 8;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    // Get selected process info from filtered processes
    let filtered_procs = app.get_filtered_processes();
    let process_info = if !filtered_procs.is_empty() && app.selected_process < filtered_procs.len() {
        let p = &filtered_procs[app.selected_process];
        format!("{} (PID: {})", p.info.name, p.info.pid)
    } else {
        "No process selected".to_string()
    };

    let menu_items = vec![
        Line::from(Span::styled(process_info, Style::default().add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::raw("k - Kill process")),
        Line::from(Span::raw("t - Kill process tree")),
        Line::from(Span::raw("o - Open process folder")),
        Line::from(Span::raw("r - Restart process")),
        Line::from(""),
        Line::from(Span::styled("ESC - Close menu", Style::default().fg(Color::Gray))),
    ];

    let paragraph = Paragraph::new(menu_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title("Process Actions")
                .style(Style::default().bg(Color::Black))
        )
        .alignment(Alignment::Left);

    f.render_widget(paragraph, popup_area);
}

fn draw_services(f: &mut Frame, app: &App, area: Rect) {
    use ratatui::widgets::TableState;
    use procmon_core::ServiceState;

    let services = &app.filtered_services;

    let rows: Vec<Row> = services
        .iter()
        .map(|s| {
            let state_style = match s.state {
                ServiceState::Running => Style::default().fg(Color::Green),
                ServiceState::Stopped => Style::default().fg(Color::Gray),
                ServiceState::Failed => Style::default().fg(Color::Red),
                ServiceState::Unknown => Style::default().fg(Color::Yellow),
            };

            let state_str = format!("{:?}", s.state);
            let enabled_str = if s.enabled { "enabled" } else { "disabled" };

            let mem_str = if let Some(mem) = s.memory_usage {
                format!("{:.1} MB", mem as f64 / (1024.0 * 1024.0))
            } else {
                "-".to_string()
            };

            let pid_str = if let Some(pid) = s.main_pid {
                pid.to_string()
            } else {
                "-".to_string()
            };

            Row::new(vec![
                Cell::from(s.name.clone()),
                Cell::from(state_str).style(state_style),
                Cell::from(s.sub_state.clone()),
                Cell::from(enabled_str),
                Cell::from(pid_str),
                Cell::from(mem_str),
                Cell::from(s.description.clone()),
            ])
        })
        .collect();

    let title = format!("Services ({}) - ↑↓: Select, Enter: Menu", services.len());

    let table = Table::new(
        rows,
        [
            Constraint::Length(25),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Min(30),
        ],
    )
    .header(
        Row::new(vec!["Name", "State", "Sub State", "Enabled", "PID", "Memory", "Description"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    )
    .highlight_symbol(">> ");

    // Create table state and set selected
    let mut table_state = TableState::default();
    table_state.select(Some(app.selected_service));

    f.render_stateful_widget(table, area, &mut table_state);

    // Draw service menu if active
    if app.show_service_menu {
        draw_service_menu(f, app);
    }
}

fn draw_service_menu(f: &mut Frame, app: &App) {
    // Create a centered popup
    let area = f.area();
    let popup_width = 40;
    let popup_height = 10;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    // Get selected service info
    let service_info = if !app.filtered_services.is_empty() && app.selected_service < app.filtered_services.len() {
        let s = &app.filtered_services[app.selected_service];
        format!("{} ({:?})", s.name, s.state)
    } else {
        "No service selected".to_string()
    };

    let menu_items = vec![
        Line::from(Span::styled(service_info, Style::default().add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::raw("s - Start service")),
        Line::from(Span::raw("p - Stop service")),
        Line::from(Span::raw("r - Restart service")),
        Line::from(Span::raw("e - Enable service")),
        Line::from(Span::raw("d - Disable service")),
        Line::from(""),
        Line::from(Span::styled("ESC - Close menu", Style::default().fg(Color::Gray))),
    ];

    let paragraph = Paragraph::new(menu_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title("Service Actions")
                .style(Style::default().bg(Color::Black))
        )
        .alignment(Alignment::Left);

    f.render_widget(paragraph, popup_area);
}

fn draw_storage(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Disk I/O summary
    let disk_items: Vec<ListItem> = app
        .system_metrics
        .disk_io
        .iter()
        .map(|(name, metrics)| {
            let content = format!(
                "{}: Read: {:.2} MB ({} ops)  Write: {:.2} MB ({} ops)",
                name,
                metrics.read_bytes as f64 / (1024.0 * 1024.0),
                metrics.read_ops,
                metrics.write_bytes as f64 / (1024.0 * 1024.0),
                metrics.write_ops
            );
            ListItem::new(content)
        })
        .collect();

    let disk_list = List::new(disk_items)
        .block(Block::default().borders(Borders::ALL).title("Disk I/O"));
    f.render_widget(disk_list, chunks[0]);

    // Top processes by disk I/O
    let mut processes = app.processes.clone();
    processes.sort_by(|a, b| {
        let a_io = a.stats.disk_read_bytes + a.stats.disk_write_bytes;
        let b_io = b.stats.disk_read_bytes + b.stats.disk_write_bytes;
        b_io.cmp(&a_io)
    });
    processes.truncate(20);

    let rows: Vec<Row> = processes
        .iter()
        .map(|p| {
            Row::new(vec![
                Cell::from(p.info.pid.to_string()),
                Cell::from(p.info.name.clone()),
                Cell::from(format!("{:.2}", p.stats.disk_read_bytes as f64 / (1024.0 * 1024.0))),
                Cell::from(format!("{:.2}", p.stats.disk_write_bytes as f64 / (1024.0 * 1024.0))),
                Cell::from(format!("{:.2}", (p.stats.disk_read_bytes + p.stats.disk_write_bytes) as f64 / (1024.0 * 1024.0))),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(15),
            Constraint::Length(15),
            Constraint::Length(15),
        ],
    )
    .header(
        Row::new(vec!["PID", "Name", "Read (MB)", "Write (MB)", "Total (MB)"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1),
    )
    .block(Block::default().borders(Borders::ALL).title("Processes by Disk I/O"));

    f.render_widget(table, chunks[1]);
}

fn draw_network(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Network interfaces
    let net_items: Vec<ListItem> = app
        .system_metrics
        .network
        .iter()
        .map(|(name, metrics)| {
            let content = format!(
                "{}: ↓ {:.2} MB  ↑ {:.2} MB  (Packets: ↓ {}  ↑ {})",
                name,
                metrics.bytes_received as f64 / (1024.0 * 1024.0),
                metrics.bytes_sent as f64 / (1024.0 * 1024.0),
                metrics.packets_received,
                metrics.packets_sent
            );
            ListItem::new(content)
        })
        .collect();

    let net_list = List::new(net_items)
        .block(Block::default().borders(Borders::ALL).title("Network Interfaces"));
    f.render_widget(net_list, chunks[0]);

    // Top processes by network (placeholder - we don't have per-process network stats yet)
    let text = Paragraph::new("Per-process network statistics not yet available.\nThis will show processes sorted by network usage.")
        .block(Block::default().borders(Borders::ALL).title("Processes by Network Usage"))
        .alignment(Alignment::Center);
    f.render_widget(text, chunks[1]);
}

fn draw_alerts(f: &mut Frame, app: &App, area: Rect) {
    let alert_items: Vec<ListItem> = app
        .alerts
        .iter()
        .rev()
        .take(50)
        .map(|alert| {
            let severity_color = match alert.severity {
                Severity::Critical => Color::Red,
                Severity::Warning => Color::Yellow,
                Severity::Info => Color::Blue,
            };

            let content = vec![
                Line::from(vec![
                    Span::styled(
                        format!("[{:?}] ", alert.severity),
                        Style::default().fg(severity_color).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(
                        "{} - {} (PID: {})",
                        alert.timestamp.format("%H:%M:%S"),
                        alert.process_name,
                        alert.pid
                    )),
                ]),
                Line::from(vec![
                    Span::raw("  "),
                    Span::raw(&alert.rule_name),
                    Span::raw(": "),
                    Span::raw(&alert.details),
                ]),
            ];

            ListItem::new(content)
        })
        .collect();

    let alert_list = List::new(alert_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Alerts ({} total)", app.alerts.len())),
    );

    f.render_widget(alert_list, area);
}

fn draw_partitions(f: &mut Frame, app: &App, area: Rect) {
    if app.disks.is_empty() {
        let text = Paragraph::new("No disks found or permission denied.\nRun with sudo for full partition management capabilities.")
            .block(Block::default().borders(Borders::ALL).title("Partition Manager"))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Disk list
    let disk_items: Vec<ListItem> = app
        .disks
        .iter()
        .map(|disk| {
            let size_gb = disk.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            let content = format!(
                "{} - {} ({:.2} GB) - {} partitions",
                disk.device,
                disk.model,
                size_gb,
                disk.partitions.len()
            );
            ListItem::new(content)
        })
        .collect();

    let disk_list = List::new(disk_items)
        .block(Block::default().borders(Borders::ALL).title("Disks (Select with ↑↓)"));
    f.render_widget(disk_list, chunks[0]);

    // Partition table for selected disk
    if app.selected_disk < app.disks.len() {
        let disk = &app.disks[app.selected_disk];

        if disk.partitions.is_empty() {
            let text = Paragraph::new(format!("No partitions on {}\n\nUse gparted or parted to create partitions.", disk.device))
                .block(Block::default().borders(Borders::ALL).title(format!("Partitions on {}", disk.device)))
                .alignment(Alignment::Center);
            f.render_widget(text, chunks[1]);
        } else {
            let rows: Vec<Row> = disk
                .partitions
                .iter()
                .map(|p| {
                    let size_gb = p.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                    let used_gb = p.used_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                    let used_percent = if p.size_bytes > 0 {
                        (p.used_bytes as f64 / p.size_bytes as f64 * 100.0)
                    } else {
                        0.0
                    };

                    Row::new(vec![
                        Cell::from(p.device.clone()),
                        Cell::from(p.filesystem.clone().unwrap_or_else(|| "unknown".to_string())),
                        Cell::from(p.label.clone().unwrap_or_else(|| "-".to_string())),
                        Cell::from(format!("{:.2}", size_gb)),
                        Cell::from(format!("{:.2} ({:.1}%)", used_gb, used_percent)),
                        Cell::from(p.mount_point.clone().unwrap_or_else(|| "-".to_string())),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Length(15),
                    Constraint::Length(10),
                    Constraint::Length(15),
                    Constraint::Length(12),
                    Constraint::Length(18),
                    Constraint::Min(20),
                ],
            )
            .header(
                Row::new(vec!["Device", "Filesystem", "Label", "Size (GB)", "Used (GB)", "Mount Point"])
                    .style(Style::default().add_modifier(Modifier::BOLD))
                    .bottom_margin(1),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Partitions on {} - {} ({:.2} GB)",
                        disk.device,
                        disk.model,
                        disk.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                    ))
            );

            f.render_widget(table, chunks[1]);
        }
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.search_mode {
        "Search Mode: Type to search, Backspace to delete, Enter/ESC to exit"
    } else {
        "q: Quit | Tab: Next Tab | 1-7: Switch Tabs | ↑↓: Navigate | /: Search | s: Sort | a: Order | m: Menu | PgUp/PgDn: Scroll | Mouse Wheel: Scroll"
    };
    let footer = Paragraph::new(text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}

fn get_usage_color(usage: f32) -> Color {
    if usage > 80.0 {
        Color::Red
    } else if usage > 60.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}
