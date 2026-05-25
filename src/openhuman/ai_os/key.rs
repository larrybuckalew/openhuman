use std::path::PathBuf;

use crate::openhuman::encryption::{EncryptionKey, KEY_LENGTH};

fn key_file_path() -> Result<PathBuf, String> {
    let data_dir = crate::openhuman::encryption::get_data_dir()?;
    Ok(data_dir.join("ai_os.key"))
}

/// Load or generate the machine-local AI OS encryption key.
/// On first call, generates 32 random bytes and writes them to
/// `~/.openhuman[/users/{id}]/ai_os.key`.
/// On subsequent calls, reads the existing file.
pub fn load_or_create_key() -> Result<EncryptionKey, String> {
    use aes_gcm::aead::rand_core::RngCore;
    use aes_gcm::aead::OsRng;

    let path = key_file_path()?;
    let key_bytes: [u8; KEY_LENGTH] = if path.exists() {
        let bytes = std::fs::read(&path).map_err(|e| format!("[ai_os] read key: {e}"))?;
        if bytes.len() != KEY_LENGTH {
            return Err(format!(
                "[ai_os] key file corrupt: expected {KEY_LENGTH} bytes, got {}",
                bytes.len()
            ));
        }
        let mut arr = [0u8; KEY_LENGTH];
        arr.copy_from_slice(&bytes);
        arr
    } else {
        let mut arr = [0u8; KEY_LENGTH];
        OsRng.fill_bytes(&mut arr);
        // Set restrictive permissions before writing on Unix
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .map_err(|e| format!("[ai_os] create key file: {e}"))?;
            f.write_all(&arr)
                .map_err(|e| format!("[ai_os] write key: {e}"))?;
        }
        #[cfg(windows)]
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            // create_new prevents overwriting; full ACL restriction requires Windows-specific APIs
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| format!("[ai_os] create key file: {e}"))?;
            f.write_all(&arr)
                .map_err(|e| format!("[ai_os] write key: {e}"))?;
        }
        #[cfg(not(any(unix, windows)))]
        {
            std::fs::write(&path, &arr).map_err(|e| format!("[ai_os] write key: {e}"))?;
        }
        tracing::debug!("[ai_os] generated new machine key at {:?}", path);
        arr
    };
    Ok(EncryptionKey::from_bytes(key_bytes))
}

/// Encrypt an API key string. Returns the encrypted form (JSON payload).
pub fn encrypt_api_key(plaintext: &str) -> Result<String, String> {
    let key = load_or_create_key()?;
    key.encrypt_string(plaintext)
}

/// Decrypt an API key string. Returns the plaintext form.
/// If the input looks like it is already plaintext (not a valid JSON object),
/// returns it as-is to handle providers created before encryption was introduced.
pub fn decrypt_api_key(encrypted: &str) -> Result<String, String> {
    // Graceful migration: if it doesn't look like our JSON payload, return as-is
    if !encrypted.trim_start().starts_with('{') {
        return Ok(encrypted.to_string());
    }
    let key = load_or_create_key()?;
    key.decrypt_string(encrypted)
}
