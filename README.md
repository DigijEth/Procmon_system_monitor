Note: The TUI is more complete and functional than the GUI

# Process Monitor

A comprehensive system and process monitoring application written in Rust with both Terminal UI (TUI) and Graphical UI (GUI) interfaces.

## Features

### System Monitoring
- **CPU Monitoring**
  - Overall CPU usage percentage
  - Per-core CPU usage with visualization
  - CPU temperature tracking
  - CPU frequency monitoring

- **Memory Monitoring**
  - Total, used, and available memory
  - Swap memory usage
  - Per-process memory consumption

- **GPU Monitoring**
  - GPU usage percentage
  - VRAM usage
  - GPU temperature
  - Support for AMD GPUs (via sysfs)

- **Network Monitoring**
  - Per-interface network statistics
  - Bytes sent/received
  - Packets sent/received
  - Error tracking

- **Disk I/O Monitoring**
  - Read/write operations per device
  - Bytes read/written
  - Per-process disk I/O tracking

- **USB Monitoring**
  - Connected USB device detection
  - Device identification (vendor/product IDs)

### Process Monitoring
- Real-time process listing
- Process owner (user) tracking
- Per-process statistics:
  - CPU usage
  - Memory usage (RSS and virtual)
  - Disk I/O (read/write bytes)
  - Thread count
  - Process status
  - Runtime duration

### Misbehavior Detection
Automatic detection of misbehaving applications based on configurable rules:

- **High CPU Usage**: Alerts when processes exceed CPU thresholds
- **Memory Leaks**: Detects processes with excessive memory consumption
- **Excessive Disk I/O**: Identifies processes with high disk activity
- **Zombie Processes**: Flags processes in zombie state
- **Network I/O**: Monitors excessive network usage

Default alert levels:
- **Critical**: Immediate attention required (>95% CPU, >8GB RAM)
- **Warning**: Potential issues (>80% CPU for 60s, >2GB RAM for 30s)
- **Info**: Informational alerts

## Architecture

The project is organized as a Cargo workspace with three crates:

```
procmon/
├── procmon-core/       # Core monitoring library
│   ├── monitor.rs      # System monitoring implementation
│   ├── metrics.rs      # Metric data structures
│   ├── process.rs      # Process information types
│   └── detector.rs     # Misbehavior detection logic
├── procmon-tui/        # Terminal UI application
│   ├── main.rs         # TUI entry point
│   ├── app.rs          # Application state
│   └── ui.rs           # Rendering logic
└── procmon-gui/        # Graphical UI application
    └── main.rs         # GUI application with egui
```

## Building

### Prerequisites
- Rust 1.70 or later
- Linux (designed for Linux systems)

### Build Commands

Build all components:
```bash
cargo build --release
```

Build individual components:
```bash
cargo build --release -p procmon-tui
cargo build --release -p procmon-gui
```

## Running

### Terminal UI (TUI)
```bash
cargo run --release -p procmon-tui
```

### Graphical UI (GUI)
```bash
cargo run --release -p procmon-gui
```

## TUI Controls

- **q** or **Ctrl+C**: Quit application
- **Tab**: Next tab
- **Shift+Tab**: Previous tab
- **1-4**: Jump to specific tab (Dashboard, Processes, Network, Alerts)
- **↑/↓**: Navigate process list
- **s**: Change sort column
- **a**: Toggle sort order (ascending/descending)
- **f**: Toggle filter for misbehaving processes

## TUI Tabs

1. **Dashboard**: System overview with CPU, memory, temperature, and top processes
2. **Processes**: Detailed process list with sorting and filtering
3. **Network**: Network interfaces and disk I/O statistics
4. **Alerts**: Real-time misbehavior alerts

## GUI Features

The GUI provides an alternative interface with the same monitoring capabilities:

- **Dashboard Tab**: Visual system overview with graphs and gauges
- **Processes Tab**: Sortable process table
- **Network & I/O Tab**: Network interfaces and disk statistics
- **Alerts Tab**: Color-coded alert list

## Dependencies

### Core Monitoring
- `sysinfo`: Cross-platform system information
- `procfs`: Linux /proc filesystem parsing
- `nix`: Unix system APIs

### TUI
- `ratatui`: Terminal UI framework
- `crossterm`: Terminal manipulation

### GUI
- `egui`: Immediate mode GUI framework
- `eframe`: egui application framework

### Common
- `tokio`: Async runtime
- `serde`: Serialization
- `chrono`: Date/time handling
- `tracing`: Logging

## Platform Support

Currently optimized for **Linux**. The application reads from:
- `/proc/` filesystem for process information
- `/sys/class/thermal/` for CPU temperature
- `/sys/class/drm/` for GPU information
- `/sys/bus/usb/devices/` for USB devices
- `/proc/diskstats` for disk I/O
- `/etc/passwd` for user information

## Customizing Misbehavior Rules

The misbehavior detector can be customized by modifying `procmon-core/src/detector.rs`. Default rules include:

```rust
MisbehaviorRule {
    name: "High CPU Usage",
    description: "Process using more than 80% CPU for extended period",
    condition: MisbehaviorCondition::CpuUsageAbove {
        threshold: 80.0,
        duration_secs: 60,
    },
    severity: Severity::Warning,
}
```

## Performance

- Updates every 1 second by default
- Minimal CPU overhead (typically <1% on modern systems)
- Efficient memory usage with bounded alert history (last 100 alerts)

## Permissions

Some features may require elevated permissions:
- Reading certain `/proc` entries may require root access
- Hardware sensor data may need appropriate permissions

Run with sudo if you encounter permission errors:
```bash
sudo cargo run --release -p procmon-tui
```

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Areas for improvement:
- NVIDIA GPU support (via NVML)
- macOS and Windows support
- Per-process network I/O tracking
- Historical data graphing
- Configuration file support
- Export monitoring data to formats (CSV, JSON)

## Troubleshooting

### No GPU detected
- Ensure GPU drivers are installed
- Check `/sys/class/drm/` exists and is accessible
- AMD GPUs are currently better supported than NVIDIA

### No temperature readings
- Install `lm-sensors` package
- Run `sensors-detect` to configure thermal sensors
- Check `/sys/class/thermal/` permissions

### Inaccurate network stats
- Network statistics are cumulative since boot
- Ensure proper permissions to read network interface data

## Technical Notes

### CPU Usage Calculation
CPU usage is calculated by the `sysinfo` crate using process CPU time divided by elapsed time.

### Memory Values
- RSS (Resident Set Size): Physical memory used
- Virtual Memory: Total virtual address space

### Disk I/O
Disk I/O statistics are read from `/proc/diskstats` and represent cumulative values since boot.
