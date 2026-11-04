//! One writer per project: a `<file>.lock` sidecar holding the owner's process id.

use std::io::Write;
use std::path::{Path, PathBuf};

use sysinfo::{Pid, ProcessesToUpdate, System};

use super::sidecar;
use crate::error::AppError;

pub enum LockResult {
    Acquired(LockGuard),
    /// Another live process (or this one) already holds it. Open read-only.
    Held,
}

#[derive(Debug)]
pub struct LockGuard {
    path: PathBuf,
    pid: u32,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Only remove the lock if it is still ours.
        if read_pid(&self.path) == Some(self.pid) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn read_pid(lock: &Path) -> Option<u32> {
    std::fs::read_to_string(lock).ok()?.trim().parse().ok()
}

fn process_alive(pid: u32) -> bool {
    let mut sys = System::new();
    let pid = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).is_some()
}

pub fn acquire(project_path: &Path) -> Result<LockResult, AppError> {
    let lock = sidecar(project_path, ".lock");
    let me = std::process::id();
    // Two tries: the second one runs after a stale lock has been removed.
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
        {
            Ok(mut file) => {
                file.write_all(me.to_string().as_bytes())
                    .map_err(AppError::io)?;
                return Ok(LockResult::Acquired(LockGuard {
                    path: lock,
                    pid: me,
                }));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                match read_pid(&lock) {
                    // A garbled file or a dead owner means the lock is stale.
                    Some(pid) if process_alive(pid) => return Ok(LockResult::Held),
                    _ => {
                        let _ = std::fs::remove_file(&lock);
                    }
                }
            }
            Err(e) => return Err(AppError::io(e)),
        }
    }
    Ok(LockResult::Held)
}
