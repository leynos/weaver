use std::io::{self, Write};
use std::path::Path;

use tempfile::Builder;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Writes the provided bytes to the path using an atomic persist step.
///
/// Data is flushed and fsync'd before the temporary file is renamed into
/// place so readers never observe a partially written payload.
pub(super) fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let directory = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "target path did not have a parent directory",
        )
    })?;

    let mut builder = Builder::new();
    builder.prefix(
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("weaver"),
    );
    #[cfg(unix)]
    {
        use std::fs::Permissions;
        builder.permissions(Permissions::from_mode(0o600));
    }

    let mut file = builder.tempfile_in(directory)?;
    file.write_all(contents)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}
