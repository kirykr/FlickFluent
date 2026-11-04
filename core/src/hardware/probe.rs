use sysinfo::{Disks, System};

use super::{Accelerator, MachineInput, OsInfo};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Read the real machine. Anything that cannot be read stays `None` (unknown); never panics.
pub fn probe() -> MachineInput {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let cores = sys
        .physical_core_count()
        .or_else(|| Some(sys.cpus().len()).filter(|n| *n > 0))
        .and_then(|n| u32::try_from(n).ok());
    let total = sys.total_memory();

    MachineInput {
        cpu_bits: Some(if cfg!(target_pointer_width = "64") {
            64
        } else {
            32
        }),
        cpu_cores: cores,
        ram_gb: (total > 0).then(|| total as f64 / GIB),
        free_disk_gb: free_disk_gb(),
        os: Some(OsInfo {
            family: std::env::consts::OS.to_string(),
            version: System::os_version(),
        }),
        accelerator: detect_accelerator(),
    }
}

/// Free space on the drive that holds the user's home folder (where projects and models go).
fn free_disk_gb() -> Option<f64> {
    let disks = Disks::new_with_refreshed_list();
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let home = std::path::PathBuf::from(home);
    disks
        .list()
        .iter()
        .filter(|d| home.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .or_else(|| disks.list().iter().max_by_key(|d| d.available_space()))
        .map(|d| d.available_space() as f64 / GIB)
}

fn detect_accelerator() -> Option<Accelerator> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Some(Accelerator::AppleSilicon);
    }
    discrete_gpu_present()
}

#[cfg(feature = "gpu")]
fn discrete_gpu_present() -> Option<Accelerator> {
    // wgpu can panic on odd drivers; treat that as "unknown" instead of crashing the check.
    std::panic::catch_unwind(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let discrete = instance
            .enumerate_adapters(wgpu::Backends::all())
            .iter()
            .any(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu);
        if discrete {
            Accelerator::DiscreteGpu
        } else {
            Accelerator::None
        }
    })
    .ok()
}

#[cfg(not(feature = "gpu"))]
fn discrete_gpu_present() -> Option<Accelerator> {
    None
}
