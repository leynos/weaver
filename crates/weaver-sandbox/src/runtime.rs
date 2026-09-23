//! Platform helpers for sandbox defaults and preflight checks.

use std::{io, path::PathBuf};

/// Returns existing Linux runtime roots using their original path spellings.
///
/// Birdcage needs the original spelling of symlinked roots such as `/lib64`
/// when it recreates the dynamic loader's path inside the sandbox.
#[must_use]
pub fn linux_runtime_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        use std::path::Path;

        let candidates = [
            "/lib",
            "/lib64",
            "/usr/lib",
            "/usr/lib64",
            "/lib/x86_64-linux-gnu",
            "/usr/lib/x86_64-linux-gnu",
            "/lib/aarch64-linux-gnu",
            "/usr/lib/aarch64-linux-gnu",
        ];
        candidates
            .iter()
            .map(Path::new)
            .filter(|candidate| candidate.exists())
            .map(Path::to_path_buf)
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
        let status = std::fs::read_to_string("/proc/self/status")?;
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

#[cfg(test)]
mod tests {
    //! Executable runtime defaults retain the paths Birdcage must recreate.

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_runtime_roots_preserve_existing_aliases() {
        let roots = super::linux_runtime_roots();
        for alias in ["/lib", "/lib64"] {
            let path = std::path::Path::new(alias);
            if path.exists() && path.is_symlink() {
                assert!(
                    roots.iter().any(|root| root == path),
                    "runtime root {alias} must retain its original spelling"
                );
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn runtime_roots_are_linux_only() {
        assert!(super::linux_runtime_roots().is_empty());
    }
}
