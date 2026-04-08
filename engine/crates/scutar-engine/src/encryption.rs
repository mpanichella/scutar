//! Encryption layer for snapshot mode.
//!
//! Design (intentionally close to age / restic / kopia):
//!
//! * **Per-repo data key (DK)**: 32 random bytes generated once when the repo
//!   is initialized. The DK never leaves the engine in plaintext form.
//! * **Key encryption key (KEK)**: derived from the user password via
//!   Argon2id with parameters stored in `config.json`.
//! * **Wrap**: the DK is encrypted with the KEK using AES-256-GCM with a
//!   random 96-bit nonce. Both the wrapped DK and the nonce live in
//!   `config.json` (`EncryptionHeader`).
//! * **Per-pack encryption**: every pack is sealed with AES-256-GCM using the
//!   DK and a fresh random nonce. The nonce is prepended to the ciphertext;
//!   format is `nonce || ciphertext || gcm_tag`.
//! * **Manifests**: snapshot manifests + the index are also sealed with the
//!   same scheme. They are JSON inside, but on the wire/in the bucket they're
//!   indistinguishable from packs.
//!
//! When encryption is *disabled*, the `Sealer::None` variant is a pass-through.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use bytes::Bytes;
use rand::RngCore;
use scutar_core::{Error, Result};
use zeroize::Zeroize;

use crate::manifest::EncryptionHeader;

/// Argon2id parameters. Tuned to be slow enough to make password brute force
/// expensive on modern hardware while still completing in <1s on a typical
/// container CPU. Tunable via `RepoConfig` if needed.
const ARGON2_M_COST: u32 = 64 * 1024; // 64 MiB
const ARGON2_T_COST: u32 = 3;
const ARGON2_P_COST: u32 = 1;

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const KEY_LEN: usize = 32;

/// Sealer abstracts the encryption decision so callers don't branch on
/// `EncryptionSpec` everywhere.
pub enum Sealer {
    None,
    Gcm(Box<Aes256Gcm>),
}

impl Sealer {
    /// Build a sealer for an *unencrypted* repo.
    pub fn none() -> Self {
        Sealer::None
    }

    /// Build a sealer from a raw 32-byte data key.
    pub fn from_data_key(key: &[u8]) -> Result<Self> {
        if key.len() != KEY_LEN {
            return Err(Error::Config("data key must be 32 bytes".into()));
        }
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        Ok(Sealer::Gcm(Box::new(cipher)))
    }

    /// Encrypt a plaintext blob. Layout for `Gcm`:
    ///   `nonce (12) || ciphertext || tag (16)`
    pub fn seal(&self, plaintext: Bytes) -> Result<Bytes> {
        match self {
            Sealer::None => Ok(plaintext),
            Sealer::Gcm(cipher) => {
                let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
                let mut ct = cipher
                    .encrypt(&nonce, plaintext.as_ref())
                    .map_err(|e| Error::Other(format!("aes-gcm seal: {e}")))?;
                let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
                out.extend_from_slice(nonce.as_slice());
                out.append(&mut ct);
                Ok(Bytes::from(out))
            }
        }
    }

    /// Inverse of `seal`. Returns the plaintext.
    pub fn open(&self, ciphertext: Bytes) -> Result<Bytes> {
        match self {
            Sealer::None => Ok(ciphertext),
            Sealer::Gcm(cipher) => {
                if ciphertext.len() < NONCE_LEN + TAG_LEN {
                    return Err(Error::Other("ciphertext too short".into()));
                }
                let (nonce_bytes, body) = ciphertext.split_at(NONCE_LEN);
                let nonce = Nonce::from_slice(nonce_bytes);
                let pt = cipher
                    .decrypt(nonce, body)
                    .map_err(|e| Error::Other(format!("aes-gcm open: {e}")))?;
                Ok(Bytes::from(pt))
            }
        }
    }
}

/// Initialize encryption for a *new* repo: generate a random data key, derive
/// the KEK from the password, wrap the DK, and return both the sealer and the
/// header to persist in `config.json`.
pub fn init_encryption(password: &str) -> Result<(Sealer, EncryptionHeader)> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let mut data_key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut data_key);

    let kek = derive_kek(password, &salt)?;
    let kek_cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&kek));
    let wrap_nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let wrapped = kek_cipher
        .encrypt(&wrap_nonce, data_key.as_ref())
        .map_err(|e| Error::Other(format!("wrap data key: {e}")))?;

    let header = EncryptionHeader {
        kdf: "argon2id".to_string(),
        kdf_salt_b64: BASE64.encode(salt),
        kdf_m_cost: ARGON2_M_COST,
        kdf_t_cost: ARGON2_T_COST,
        kdf_p_cost: ARGON2_P_COST,
        wrapped_data_key_b64: BASE64.encode(&wrapped),
        wrap_nonce_b64: BASE64.encode(wrap_nonce.as_slice()),
    };

    let sealer = Sealer::from_data_key(&data_key)?;
    data_key.zeroize();
    Ok((sealer, header))
}

/// Unwrap an existing encryption header using the password and return a
/// usable sealer for the data key.
pub fn open_encryption(password: &str, header: &EncryptionHeader) -> Result<Sealer> {
    if header.kdf != "argon2id" {
        return Err(Error::Config(format!("unsupported kdf: {}", header.kdf)));
    }
    let salt = BASE64
        .decode(&header.kdf_salt_b64)
        .map_err(|e| Error::Config(format!("salt base64: {e}")))?;
    let wrapped = BASE64
        .decode(&header.wrapped_data_key_b64)
        .map_err(|e| Error::Config(format!("wrapped key base64: {e}")))?;
    let wrap_nonce = BASE64
        .decode(&header.wrap_nonce_b64)
        .map_err(|e| Error::Config(format!("wrap nonce base64: {e}")))?;

    let kek = derive_kek_with(
        password,
        &salt,
        header.kdf_m_cost,
        header.kdf_t_cost,
        header.kdf_p_cost,
    )?;
    let kek_cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&kek));
    let nonce = Nonce::from_slice(&wrap_nonce);
    let mut data_key = kek_cipher
        .decrypt(nonce, wrapped.as_ref())
        .map_err(|e| Error::Other(format!("unwrap data key (wrong password?): {e}")))?;
    let sealer = Sealer::from_data_key(&data_key)?;
    data_key.zeroize();
    Ok(sealer)
}

fn derive_kek(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    derive_kek_with(password, salt, ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST)
}

fn derive_kek_with(
    password: &str,
    salt: &[u8],
    m: u32,
    t: u32,
    p: u32,
) -> Result<[u8; KEY_LEN]> {
    let params = Params::new(m, t, p, Some(KEY_LEN))
        .map_err(|e| Error::Config(format!("argon2 params: {e}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| Error::Other(format!("argon2 derive: {e}")))?;
    Ok(out)
}

/// Read a password file and trim trailing whitespace. The file content is
/// kept on the stack as a String — callers should drop it as soon as the KEK
/// has been derived.
pub fn read_password_file(path: &std::path::Path) -> Result<String> {
    let raw = std::fs::read_to_string(path)?;
    Ok(raw.trim().to_string())
}
