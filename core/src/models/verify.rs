use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{AppError, HASH_MISMATCH};

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// `Ok` only if the file's SHA-256 equals `expected` (case does not matter).
pub fn verify_file(path: &Path, expected: &str) -> Result<(), AppError> {
    let actual = sha256_file(path).map_err(AppError::io)?;
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(AppError::new(HASH_MISMATCH, "models.error.hashMismatch"))
    }
}
