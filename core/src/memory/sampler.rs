use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};

/// The badge turns to a warning when the computer uses this much of its memory.
pub const WARNING_PERCENT: u64 = 85;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Normal,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySample {
    /// Memory held by this app's main process. Helper processes (such as the web view) are not
    /// counted, so the system figure is the better guide to overall load.
    pub app_bytes: u64,
    pub system_used_bytes: u64,
    pub system_total_bytes: u64,
    pub level: Level,
}

/// `Warning` at or above `WARNING_PERCENT` of the total. A total of zero means the value could
/// not be read, which is treated as normal rather than alarming the user.
pub fn level(used: u64, total: u64) -> Level {
    if total == 0 {
        return Level::Normal;
    }
    // Compare as used * 100 >= total * 85 in 128 bits so huge values cannot overflow.
    if u128::from(used) * 100 >= u128::from(total) * u128::from(WARNING_PERCENT) {
        Level::Warning
    } else {
        Level::Normal
    }
}

pub fn sample() -> MemorySample {
    let mut sys = System::new();
    sys.refresh_memory();
    let pid = Pid::from_u32(std::process::id());
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

    let total = sys.total_memory();
    let used = sys.used_memory().min(total);
    MemorySample {
        app_bytes: sys.process(pid).map(|p| p.memory()).unwrap_or(0),
        system_used_bytes: used,
        system_total_bytes: total,
        level: level(used, total),
    }
}
