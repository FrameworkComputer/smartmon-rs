//! Rust bindings for libsmartmon, smartmontools built as a library
//!
//! Reads disk identify information (model, serial, firmware version).
//!
//! Supports ATA, NVMe and SCSI disks, including NVMe drives behind USB bridges,
//! like the Framework Storage Expansion Cards, which show up as /dev/sdX.

use std::ffi::{c_char, c_int, CStr};
use std::sync::Mutex;

const MAX_DISKS: usize = 32;

/// libsmartmon has global state (the platform interface), don't scan concurrently
static SCAN_LOCK: Mutex<()> = Mutex::new(());

/// Must match `struct smartmon_disk` in wrapper.cpp
#[repr(C)]
#[derive(Clone, Copy)]
struct SmartmonDisk {
    name: [c_char; 64],
    dev_type: [c_char; 32],
    protocol: [c_char; 8],
    model: [c_char; 48],
    serial: [c_char; 24],
    firmware: [c_char; 16],
}

extern "C" {
    fn smartmon_scan(out: *mut SmartmonDisk, max: c_int) -> c_int;
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    /// OS device path, for example /dev/nvme0 or /dev/sdc
    pub name: String,
    /// smartmontools device type, for example nvme, sat or sntasmedia
    pub dev_type: String,
    /// ATA, NVMe or SCSI
    pub protocol: String,
    pub model: String,
    pub serial: String,
    pub firmware: String,
}

fn to_string(chars: &[c_char]) -> String {
    // Safe because the C side always null-terminates via snprintf
    unsafe { CStr::from_ptr(chars.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// Scan all disks and read their identify information
pub fn scan_disks() -> Result<Vec<DiskInfo>, String> {
    let empty = SmartmonDisk {
        name: [0; 64],
        dev_type: [0; 32],
        protocol: [0; 8],
        model: [0; 48],
        serial: [0; 24],
        firmware: [0; 16],
    };
    let mut disks = vec![empty; MAX_DISKS];
    let _guard = SCAN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let count = unsafe { smartmon_scan(disks.as_mut_ptr(), MAX_DISKS as c_int) };
    if count < 0 {
        return Err("Failed to scan disks".to_string());
    }
    Ok(disks[..count as usize]
        .iter()
        .map(|d| DiskInfo {
            name: to_string(&d.name),
            dev_type: to_string(&d.dev_type),
            protocol: to_string(&d.protocol),
            model: to_string(&d.model),
            serial: to_string(&d.serial),
            firmware: to_string(&d.firmware),
        })
        .collect())
}
