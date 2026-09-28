// Basic logic adapted from SoulsFormatsNext and Fromformats

use std::{fs::File, path::Path};
use std::io::Read;
use cipher::{block_padding::{NoPadding, Pkcs7}, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};

use crate::binary::IO;
use crate::games::Game;

type Decryptor = cbc::Decryptor<aes::Aes256>;
type Encryptor = cbc::Encryptor<aes::Aes256>;

pub const IV_LENGTH: usize = 16;


pub struct RawRegulation {
    bnd: Vec<u8>,

}
impl RawRegulation {
    pub fn from_path(path: &Path, game: Game) -> Result<Self, &'static str> {
        let mut file = File::open(path).expect("File not found.");
        let mut data = Vec::new();
        file.read_to_end(&mut data);

        let key = game.regulation_key;

        let decrypted_data = match key {
            Some(k) => Self::decrypt(&k, &data)?,
            None => data.to_owned()
        };

        Ok(Self {
            bnd: decrypted_data
        })
    }

    pub fn decrypt(key: &[u8; 32], secret: &[u8]) -> Result<Vec<u8>, &'static str> {
        if secret.len() < IV_LENGTH {
            return Err("Secret must contain at least a 16-byte IV");
        }

        //first 16 bytes is iv
        let iv: &[u8; IV_LENGTH] = secret[..IV_LENGTH]
            .try_into()
            .map_err(|_| "Invalid IV")?;

        let ciphertext = &secret[IV_LENGTH..];

        //pad ciphertext until its len is a multiple of 16
        let padded_len = (ciphertext.len() + 15) / 16 * 16;
        let mut encrypted = vec![0u8; padded_len];
        encrypted[..ciphertext.len()].copy_from_slice(ciphertext);

        let decryptor = Decryptor::new(key.into(), iv.into());

        decryptor
            .decrypt_padded::<NoPadding>(&mut encrypted)
            .map(|result| result.to_vec())
            .map_err(|_| "AES decryption failed")
    }

    pub fn encrypt(key: &[u8; 32], secret: &[u8]) -> Vec<u8> {
        let iv = [0u8; IV_LENGTH];
        let padded_len = ((secret.len() / 16) + 1) * 16;

        let mut encrypted = vec![0u8; padded_len];
        encrypted[..secret.len()].copy_from_slice(secret);

        let encryptor = Encryptor::new(key.into(), (&iv).into());

        let encrypted = encryptor
            .encrypt_padded::<Pkcs7>(&mut encrypted, secret.len())
            .expect("Buffer should be large enough");

        let mut result = Vec::with_capacity(16 + encrypted.len());

        result.extend_from_slice(&iv);
        result.extend_from_slice(encrypted);

        result
    }

}

