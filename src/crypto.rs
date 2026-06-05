use crate::error::MzcError;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use pbkdf2::pbkdf2_hmac;
use rand::Rng;
use sha2::Sha256;

const PBKDF2_ITERATIONS: u32 = 10_000;

/// Derives a 256-bit key from a password and salt using PBKDF2-HMAC-SHA256.
pub fn derive_key(password: &str, salt: &[u8; 16]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        salt,
        PBKDF2_ITERATIONS,
        &mut key,
    );
    key
}

/// Encrypts a payload using AES-256-GCM with a derived key.
/// Returns `[16-byte Salt] + [12-byte Nonce] + [Ciphertext + Auth Tag]`.
pub fn encrypt_payload(payload: &[u8], password: &str) -> Result<Vec<u8>, MzcError> {
    // 1. Generate random 16-byte salt and 12-byte nonce
    let mut rng = rand::thread_rng();
    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut salt);
    rng.fill(&mut nonce_bytes);

    // 2. Derive key
    let key = derive_key(password, &salt);

    // 3. Initialize cipher
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| MzcError::IoError(format!("Failed to initialize AES-GCM: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    // 4. Encrypt
    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|e| MzcError::IoError(format!("Encryption failed: {}", e)))?;

    // 5. Combine salt + nonce + ciphertext
    let mut output = Vec::with_capacity(16 + 12 + ciphertext.len());
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

/// Decrypts an encrypted payload using AES-256-GCM.
pub fn decrypt_payload(encrypted_data: &[u8], password: &str) -> Result<Vec<u8>, MzcError> {
    // 16 bytes salt + 12 bytes nonce + at least 16 bytes for GCM auth tag
    if encrypted_data.len() < 16 + 12 + 16 {
        return Err(MzcError::TruncatedBlock {
            expected: 44,
            found: encrypted_data.len(),
        });
    }

    // 1. Extract salt and nonce
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&encrypted_data[0..16]);

    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&encrypted_data[16..28]);

    let ciphertext = &encrypted_data[28..];

    // 2. Derive key
    let key = derive_key(password, &salt);

    // 3. Initialize cipher
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| MzcError::IoError(format!("Failed to initialize AES-GCM: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    // 4. Decrypt
    let decrypted = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| MzcError::DecryptionFailed)?;

    Ok(decrypted)
}
