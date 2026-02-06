use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

/// Computes a combined hash of all files in a directory for conflict detection.
pub fn compute_directory_hash(path: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut count = 0;

    for entry in WalkDir::new(path).sort_by_file_name() {
        let entry = entry.map_err(|e| format!("Failed to walk directory: {e}"))?;
        let file_path = entry.path();

        if file_path.is_file() {
            // Include relative path in hash to detect renames
            let relative = file_path
                .strip_prefix(path)
                .map_err(|e| format!("Failed to strip prefix: {e}"))?;
            hasher.update(relative.to_string_lossy().as_bytes());

            // Include file content hash
            let mut file = fs::File::open(file_path)
                .map_err(|e| format!("Failed to open file {}: {e}", file_path.display()))?;
            let mut buffer = [0u8; 8192];
            loop {
                let bytes_read = file
                    .read(&mut buffer)
                    .map_err(|e| format!("Failed to read file {}: {e}", file_path.display()))?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(&buffer[..bytes_read]);
            }
            count += 1;
        }
    }

    // Include file count to detect deletions
    hasher.update(count.to_string().as_bytes());
    Ok(hex::encode(hasher.finalize()))
}
