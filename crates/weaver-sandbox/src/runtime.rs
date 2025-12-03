//! Platform helpers for sandbox defaults and preflight checks.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Returns standard Linux library paths that should be readable by default.
#[must_use]
pub fn linux_runtime_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let candidates = [
            "/lib",
            "/lib64",
            "/usr/lib",
            "/usr/lib64",
            "/lib/x86_64-linux-gnu",
            "/usr/lib/x86_64-linux-gnu",
        ];
        candidates
            .iter()
            .filter_map(|path| {
                let candidate = Path::new(path);
                if candidate.exists() {
                    fs::canonicalize(candidate).ok()
                } else {
                    None
                }
            })
            .collect()
    }

    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

/// Returns the number of threads in the current process.
pub fn thread_count() -> io::Result<usize> {
    #[cfg(target_os = "linux")]
    {
        let status = fs::read_to_string("/proc/self/status")?;
        let (_, tail) = status
            .split_once("Threads:")
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing thread count"))?;
        let count = tail
            .split_whitespace()
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed thread count"))?;
        count
            .parse::<usize>()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    #[cfg(not(target_os = "linux"))]
    {
        Ok(1)
    }
}
