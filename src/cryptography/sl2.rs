// Logic adapted from SoulsFormats

use std::{fs::File, path::Path};
use std::io::Read;
use rand::{self, Rng};
use md5::{Digest, Md5};
use cipher::{block_padding::{NoPadding, Pkcs7}, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};

use crate::binary::IO;
use crate::games::Game;

type Decryptor = cbc::Decryptor<aes::Aes128>;
type Encryptor = cbc::Encryptor<aes::Aes128>;

/// Decrypt an SL2 save file.
///
/// Format: [[16 bytes MD5]][[16 bytes IV]][[AES-CBC ciphertext]]
///
/// Padding is intentionally not removed. SL2 padding does not always contain the minimum PKCS#7 padding byte.
pub fn decrypt(encrypted: &[u8], key: &[u8; 16]) -> Result<Vec<u8>, String> {
    if encrypted.len() < 32 {
        return Err("SL2 file is too short".into());
    }

    let mut iv = [0u8; 16];
    iv.copy_from_slice(&encrypted[16..32]);

    let ciphertext = &encrypted[32..];

    if ciphertext.len() % 16 != 0 {
        return Err("ciphertext length is not a multiple of AES block size".into());
    }

    let decryptor = Decryptor::new(key.into(), (&iv).into());

    let mut plaintext = ciphertext.to_vec();

    decryptor
        .decrypt_padded::<NoPadding>(&mut plaintext)
        .map(|p| p.to_vec())
        .map_err(|_| "AES-CBC decryption failed".into())
}

pub fn decrypt_from(path: &Path, game: Game) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::<u8>::new();
    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open file: {e}"))?;

    file.read_to_end(&mut buffer);

    let key = game.sl2_key;

    match key {
        Some(k) => decrypt(&buffer, &k),
        None => Ok(buffer.to_owned())
    }
}

/// Encrypt an SL2 save file.
///
/// The resulting format is: [[16 bytes MD5]][[16 bytes IV]][[AES-CBC ciphertext]]
pub fn encrypt(decrypted: &[u8], key: &[u8; 16]) -> Result<Vec<u8>, String> {
    if decrypted.len() % 16 != 0 {
        return Err("plaintext length must be a multiple of AES block size when using NoPadding".to_string());
    }

    let mut iv = [0u8; 16];
    rand::rng().fill_bytes(&mut iv);

    let encryptor = Encryptor::new(key.into(), (&iv).into());

    let mut ciphertext = decrypted.to_vec();

    let ciphertext = encryptor
        .encrypt_padded::<NoPadding>(&mut ciphertext, decrypted.len())
        .map_err(|_| "AES-CBC encryption failed".to_string())?;

    let mut encrypted = Vec::with_capacity(16 + 16 + ciphertext.len());

    // Placeholder for MD5.
    encrypted.extend_from_slice(&[0u8; 16]);

    encrypted.extend_from_slice(&iv);
    encrypted.extend_from_slice(ciphertext);

    let mut hasher = Md5::new();
    hasher.update(&encrypted[16..]);
    let hash = hasher.finalize();

    encrypted[..16].copy_from_slice(&hash);

    Ok(encrypted)
}

