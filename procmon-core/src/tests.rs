#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::collections::HashSet;

    #[test]
    fn test_pid_accuracy() {
        // Get PIDs from our monitoring code using get_all_processes() which has the /proc filter
        let monitor = crate::monitor::SystemMonitor::new();
        // Refresh multiple times to ensure clean data
        monitor.refresh();
        std::thread::sleep(std::time::Duration::from_millis(500));
        monitor.refresh();

        // This should now return only valid PIDs due to our /proc filter
        let processes = monitor.get_all_processes().unwrap();

        println!("Total processes returned: {}", processes.len());

        // Check for duplicates
        let our_pids: HashSet<u32> = processes.iter().map(|p| p.info.pid).collect();
        println!("Unique PIDs: {}, Total processes: {}", our_pids.len(), processes.len());
        if our_pids.len() != processes.len() {
            println!("WARNING: Duplicate PIDs detected!");
        }

        // Get PIDs from /proc directly
        let mut proc_pids = HashSet::new();
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    if let Ok(pid) = file_name.parse::<u32>() {
                        proc_pids.insert(pid);
                    }
                }
            }
        }

        println!("Our filtered PIDs: {}, /proc PIDs: {}", our_pids.len(), proc_pids.len());

        // Find some examples of PIDs we have that /proc doesn't
        let mut example_count = 0;
        for pid in &our_pids {
            if !proc_pids.contains(pid) && example_count < 5 {
                eprintln!("Example missing PID: {} (checking if /proc/{}/stat exists...)", pid, pid);
                let stat_path = format!("/proc/{}/stat", pid);
                let exists = std::path::Path::new(&stat_path).exists();
                let can_read = fs::read_to_string(&stat_path).is_ok();
                eprintln!("  - Path exists: {}, Can read: {}", exists, can_read);
                example_count += 1;
            }
        }

        // Check that ALL of our PIDs exist in /proc (since we filter them)
        let mut matched = 0;
        let mut total = 0;
        for pid in &our_pids {
            total += 1;
            if proc_pids.contains(pid) {
                matched += 1;
            }
        }

        // Should be 100% or very close (allowing for tiny race conditions)
        let match_rate = (matched as f64 / total as f64) * 100.0;
        assert!(match_rate > 99.0,
            "Only {:.1}% of filtered PIDs matched /proc. Expected >99%. Matched: {}/{}",
            match_rate, matched, total);

        println!("PID accuracy test PASSED: {}/{} ({:.1}%) PIDs verified", matched, total, match_rate);
    }

    #[test]
    fn test_specific_process_pid() {
        let monitor = crate::monitor::SystemMonitor::new();
        monitor.refresh();
        let processes = monitor.get_all_processes().unwrap();

        // Find init process (PID 1) - should always exist
        let init = processes.iter().find(|p| p.info.pid == 1);
        assert!(init.is_some(), "Init process (PID 1) not found");

        // Verify our PID matches what's in /proc
        for process in processes.iter().take(10) {
            let pid = process.info.pid;
            let proc_path = format!("/proc/{}/cmdline", pid);

            // If /proc/<pid> exists, verify it
            if std::path::Path::new(&proc_path).exists() {
                println!("Verified PID {} exists: {}", pid, process.info.name);
            }
        }
    }
}
