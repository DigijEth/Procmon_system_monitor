# Quick Start Guide

## Installation

### Install Rust (if not already installed)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Clone and Build
```bash
cd /home/snake/RustroverProjects/untitled

# Build everything
cargo build --release

# Or build individual components
cargo build --release -p procmon-tui   # Terminal UI
cargo build --release -p procmon-gui   # Graphical UI
```

## Running the Applications

### Terminal UI (Recommended for servers/SSH)
```bash
cargo run --release -p procmon-tui
```

Or run the compiled binary:
```bash
./target/release/procmon-tui
```

### Graphical UI (Recommended for desktop)
```bash
cargo run --release -p procmon-gui
```

Or run the compiled binary:
```bash
./target/release/procmon-gui
```

## First Time Setup

For best results, ensure the following packages are installed on your Linux system:

### Ubuntu/Debian
```bash
sudo apt-get install lm-sensors build-essential
sudo sensors-detect --auto  # Detect and configure thermal sensors
```

### Arch Linux
```bash
sudo pacman -S lm_sensors base-devel
sudo sensors-detect --auto
```

### Fedora/RHEL
```bash
sudo dnf install lm_sensors gcc
sudo sensors-detect --auto
```

## TUI Quick Reference

Once the TUI is running:

1. **Navigate Tabs**
   - Press `1` for Dashboard (system overview)
   - Press `2` for Processes (detailed process list)
   - Press `3` for Network (network and disk I/O)
   - Press `4` for Alerts (misbehavior alerts)

2. **Process List Controls**
   - `↑/↓` arrows to navigate
   - `s` to cycle through sort columns
   - `a` to toggle ascending/descending
   - `f` to filter by misbehaving processes

3. **Exit**
   - Press `q` or `Ctrl+C` to quit

## GUI Quick Reference

The GUI uses tabs at the top:

1. **Dashboard**: Overview with graphs and gauges
2. **Processes**: Click column headers to sort
3. **Network & I/O**: Real-time network and disk statistics
4. **Alerts**: Color-coded alerts (Red=Critical, Yellow=Warning, Blue=Info)

## Understanding the Alerts

The application automatically detects misbehaving processes:

- **Critical (Red)**: Immediate attention needed
  - CPU usage > 95%
  - Memory usage > 8GB

- **Warning (Yellow)**: Potential issues
  - CPU > 80% for more than 60 seconds
  - Memory > 2GB for more than 30 seconds
  - High disk I/O (>100 MB/s for 60 seconds)

- **Info (Blue)**: Informational alerts
  - Zombie processes
  - Other noteworthy events

## Monitoring Specific Metrics

### CPU Monitoring
- **Dashboard Tab**: Shows overall CPU usage, per-core breakdown, and temperature
- **Temperature**: Reads from `/sys/class/thermal/` or `/sys/class/hwmon/`

### GPU Monitoring
- Currently supports AMD GPUs via sysfs (`/sys/class/drm/`)
- Shows GPU usage, VRAM, and temperature
- For NVIDIA GPUs, you may need to install additional drivers

### Network Monitoring
- **Network Tab**: Shows all active network interfaces
- Statistics are cumulative since boot
- Includes packet counts and error rates

### Disk I/O
- **Network Tab**: Shows per-device disk statistics
- Read/write operations and bytes transferred
- Filters out loop and ram devices for clarity

### Process Details
- **Processes Tab**: Complete process information
- Click or select a process for details
- Sortable by CPU, Memory, Disk I/O, Name, or User

## Performance Tips

1. **Update Interval**: Default is 1 second. This balances responsiveness with CPU usage.

2. **Running as Root**: Some metrics require root access:
   ```bash
   sudo ./target/release/procmon-tui
   ```

3. **Filtering**: Use the filter feature (`f` in TUI) to focus on problematic processes

4. **Sorting**: Sort by CPU or Memory to quickly identify resource hogs

## Troubleshooting

### No GPU Detected
- Check if `/sys/class/drm/` exists: `ls /sys/class/drm/`
- Ensure GPU drivers are properly installed
- NVIDIA users may need additional work (see README)

### No Temperature Readings
- Run `sensors` command to check if thermal sensors are detected
- Run `sudo sensors-detect` to configure
- Check permissions on `/sys/class/thermal/`

### Permission Denied Errors
- Some metrics require root access
- Try running with `sudo`
- Check that you can read `/proc/` entries

### High CPU Usage from Monitor
- This is unusual; the monitor should use <1% CPU
- Try increasing the update interval in the code
- Check for runaway processes being monitored

## Next Steps

- Explore the codebase in `procmon-core/` to customize detection rules
- Modify alert thresholds in `procmon-core/src/detector.rs`
- Add custom monitoring logic to suit your needs
- Contribute improvements back to the project

## Getting Help

If you encounter issues:
1. Check the main README.md for detailed information
2. Review system logs for errors
3. Ensure all dependencies are installed
4. Verify Rust toolchain is up to date: `rustup update`

Enjoy monitoring your system!
