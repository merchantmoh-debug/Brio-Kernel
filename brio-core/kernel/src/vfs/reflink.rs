use reflink;
use std::fs;
use std::path::Path;
use tracing::{debug, info};
use walkdir::WalkDir;

/// Recursively copies a directory using reflink if possible, falling back to standard copy.
///
/// # Errors
///
/// Returns an error if directory creation or file copy operations fail.
pub fn copy_dir_reflink(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();

        // Calculate relative path from src
        let Ok(relative_path) = path.strip_prefix(src) else {
            continue; // Should not happen given WalkDir logic
        };

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let target_path = dst.join(relative_path);

        if path.is_dir() {
            fs::create_dir_all(&target_path)?;
        } else {
            // Attempt reflink
            match reflink::reflink(path, &target_path) {
                Ok(()) => {
                    debug!("Reflinked: {:?} -> {:?}", path, target_path);
                }
                Err(e) => {
                    debug!(
                        "Reflink failed ({}), falling back to copy: {:?} -> {:?}",
                        e, path, target_path
                    );
                    fs::copy(path, &target_path)?;
                }
            }
        }
    }

    info!("Session copy complete: {:?} -> {:?}", src, dst);
    Ok(())
}
