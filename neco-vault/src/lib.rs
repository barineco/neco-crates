//! Memory-only signing vault built on `neco-secp`.

use std::collections::HashMap;
#[cfg(feature = "security-hardening")]
use std::hint::spin_loop;

#[cfg(feature = "encrypted")]
use aes::Aes256;
#[cfg(feature = "encrypted")]
use cbc::cipher::block_padding::Pkcs7;
#[cfg(feature = "encrypted")]
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
#[cfg(feature = "nostr")]
use neco_secp::{nostr, SignedEvent, UnsignedEvent};
use neco_secp::{SecpError, SecretKey, XOnlyPublicKey};
#[cfg(feature = "encrypted")]
use scrypt::Params as ScryptParams;
#[cfg(feature = "encrypted-legacy-v1")]
use sha2::{Digest, Sha256};

#[cfg(feature = "encrypted")]
type Aes256CbcEnc = cbc::Encryptor<Aes256>;
#[cfg(feature = "encrypted")]
type Aes256CbcDec = cbc::Decryptor<Aes256>;
#[cfg(feature = "encrypted")]
const ENCRYPTED_V2_VERSION: u8 = 0x02;
#[cfg(feature = "encrypted")]
const ENCRYPTED_V2_LOG_N: u8 = 15;
#[cfg(feature = "encrypted")]
const ENCRYPTED_V2_R: u8 = 8;
#[cfg(feature = "encrypted")]
const ENCRYPTED_V2_P: u8 = 1;
#[cfg(feature = "encrypted-legacy-v1")]
const ENCRYPTED_V1_LEN: usize = 64;
#[cfg(feature = "encrypted")]
const ENCRYPTED_V2_LEN: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityConfig {
    pub enable_constant_time: bool,
    pub enable_random_delay: bool,
    pub enable_dummy_operations: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_constant_time: true,
            enable_random_delay: false,
            enable_dummy_operations: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VaultConfig {
    pub cache_timeout_seconds: u64,
    pub security: SecurityConfig,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            cache_timeout_seconds: 300,
            security: SecurityConfig::default(),
        }
    }
}

#[derive(Debug)]
pub enum VaultError {
    DuplicateLabel,
    MissingLabel,
    NoActiveAccount,
    InvalidEncrypted(&'static str),
    Crypto(SecpError),
}

/// Sealed DM result with scanning tag for stealth discovery.
#[cfg(feature = "nip17")]
pub struct SealedDmResult {
    pub event: SignedEvent,
    pub scanning_tag: [u8; 16],
}

impl core::fmt::Display for VaultError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DuplicateLabel => f.write_str("duplicate label"),
            Self::MissingLabel => f.write_str("missing label"),
            Self::NoActiveAccount => f.write_str("no active account"),
            Self::InvalidEncrypted(message) => f.write_str(message),
            Self::Crypto(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for VaultError {}

impl From<SecpError> for VaultError {
    fn from(value: SecpError) -> Self {
        Self::Crypto(value)
    }
}

#[derive(Debug)]
struct Entry {
    secret: SecretKey,
    last_used_unix_seconds: u64,
}

#[cfg(feature = "security-hardening")]
fn apply_random_delay() {
    let mut byte = [0u8; 1];
    if getrandom::getrandom(&mut byte).is_err() {
        return;
    }
    let loops = 64 + usize::from(byte[0] & 0x3f);
    for _ in 0..loops {
        spin_loop();
    }
}

#[cfg(feature = "security-hardening")]
fn apply_dummy_sign(secret: &SecretKey) {
    let _ = secret.sign_schnorr_prehash([0x5a; 32]);
}

#[cfg(all(feature = "security-hardening", feature = "nip04"))]
fn apply_dummy_nip04(secret: &SecretKey) {
    if let Ok(peer) = secret.xonly_public_key() {
        let _ = neco_secp::nip04::encrypt(secret, &peer, "", Some([0u8; 16]));
    }
}

#[cfg(all(feature = "security-hardening", feature = "nip44"))]
fn apply_dummy_nip44(secret: &SecretKey) {
    if let Ok(peer) = secret.xonly_public_key() {
        if let Ok(conversation_key) = neco_secp::nip44::get_conversation_key(secret, &peer) {
            let _ = neco_secp::nip44::encrypt("", &conversation_key, Some([0u8; 32]));
        }
    }
}

#[cfg(feature = "security-hardening")]
fn apply_security_before(security: SecurityConfig, secret: &SecretKey) {
    if security.enable_dummy_operations {
        apply_dummy_sign(secret);
    }
    if security.enable_random_delay {
        apply_random_delay();
    }
    if security.enable_constant_time {
        std::hint::black_box(secret.to_bytes());
    }
}

#[cfg(feature = "security-hardening")]
fn apply_security_after(security: SecurityConfig, secret: &SecretKey) {
    if security.enable_constant_time {
        std::hint::black_box(secret.to_bytes());
    }
    if security.enable_dummy_operations {
        apply_dummy_sign(secret);
    }
    if security.enable_random_delay {
        apply_random_delay();
    }
}

#[cfg(feature = "encrypted-legacy-v1")]
fn sha256(input: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(input));
    out
}

#[cfg(feature = "encrypted")]
fn scrypt_derive(
    passphrase: &[u8],
    salt: &[u8; 32],
    log_n: u8,
    r: u8,
    p: u8,
) -> Result<[u8; 32], VaultError> {
    let params = ScryptParams::new(log_n, r.into(), p.into(), 32)
        .map_err(|_| VaultError::InvalidEncrypted("invalid scrypt params"))?;
    let mut out = [0u8; 32];
    scrypt::scrypt(passphrase, salt, &params, &mut out)
        .map_err(|_| VaultError::InvalidEncrypted("failed to derive key"))?;
    Ok(out)
}

#[cfg(feature = "encrypted")]
fn aes256_cbc_encrypt(
    key: &[u8; 32],
    iv: &[u8; 16],
    plaintext: &[u8],
) -> Result<Vec<u8>, VaultError> {
    let mut buf = plaintext.to_vec();
    let msg_len = buf.len();
    buf.resize(msg_len + 16, 0);
    let ciphertext = Aes256CbcEnc::new(key.into(), iv.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, msg_len)
        .map_err(|_| VaultError::InvalidEncrypted("failed to encrypt"))?;
    Ok(ciphertext.to_vec())
}

#[cfg(feature = "encrypted")]
fn aes256_cbc_decrypt(
    key: &[u8; 32],
    iv: &[u8; 16],
    ciphertext: &[u8],
) -> Result<Vec<u8>, VaultError> {
    let mut buf = ciphertext.to_vec();
    let plaintext = Aes256CbcDec::new(key.into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| VaultError::InvalidEncrypted("failed to decrypt"))?;
    Ok(plaintext.to_vec())
}

#[derive(Debug)]
pub struct Vault {
    config: VaultConfig,
    entries: HashMap<String, Entry>,
    active_label: Option<String>,
}

impl Vault {
    pub fn new(config: VaultConfig) -> Result<Self, VaultError> {
        Ok(Self {
            config,
            entries: HashMap::new(),
            active_label: None,
        })
    }

    pub fn import_plaintext(
        &mut self,
        label: &str,
        secret: SecretKey,
        now_unix_seconds: u64,
    ) -> Result<(), VaultError> {
        if self.entries.contains_key(label) {
            return Err(VaultError::DuplicateLabel);
        }
        let set_active = self.entries.is_empty();
        self.entries.insert(
            label.to_string(),
            Entry {
                secret,
                last_used_unix_seconds: now_unix_seconds,
            },
        );
        if set_active {
            self.active_label = Some(label.to_string());
        }
        Ok(())
    }

    pub fn contains(&self, label: &str) -> bool {
        self.entries.contains_key(label)
    }

    pub fn set_active(&mut self, label: &str) -> Result<(), VaultError> {
        if !self.entries.contains_key(label) {
            return Err(VaultError::MissingLabel);
        }
        self.active_label = Some(label.to_string());
        Ok(())
    }

    pub fn active_label(&self) -> Option<&str> {
        self.active_label.as_deref()
    }

    pub fn set_security_config(&mut self, security: SecurityConfig) {
        self.config.security = security;
    }

    pub fn security_config(&self) -> SecurityConfig {
        self.config.security
    }

    pub fn remove(&mut self, label: &str) -> Result<(), VaultError> {
        if self.entries.remove(label).is_none() {
            return Err(VaultError::MissingLabel);
        }
        if self.active_label.as_deref() == Some(label) {
            self.active_label = None;
        }
        Ok(())
    }

    pub fn labels(&self) -> Vec<&str> {
        let mut labels: Vec<_> = self.entries.keys().map(String::as_str).collect();
        labels.sort();
        labels
    }

    pub fn public_key(&self, label: &str) -> Result<XOnlyPublicKey, VaultError> {
        let entry = self.entries.get(label).ok_or(VaultError::MissingLabel)?;
        entry.secret.xonly_public_key().map_err(VaultError::from)
    }

    pub fn public_key_active(&self) -> Result<XOnlyPublicKey, VaultError> {
        let label = self
            .active_label
            .as_deref()
            .ok_or(VaultError::NoActiveAccount)?;
        self.public_key(label)
    }

    #[cfg(feature = "nostr")]
    pub fn sign_event(
        &mut self,
        label: &str,
        unsigned: UnsignedEvent,
        now_unix_seconds: u64,
    ) -> Result<SignedEvent, VaultError> {
        #[cfg(feature = "security-hardening")]
        let security = self.config.security;
        let entry = self
            .entries
            .get_mut(label)
            .ok_or(VaultError::MissingLabel)?;
        entry.last_used_unix_seconds = now_unix_seconds;
        #[cfg(feature = "security-hardening")]
        apply_security_before(security, &entry.secret);
        let signed = nostr::finalize_event(unsigned, &entry.secret).map_err(VaultError::from)?;
        #[cfg(feature = "security-hardening")]
        apply_security_after(security, &entry.secret);
        Ok(signed)
    }

    #[cfg(feature = "nostr")]
    pub fn sign_event_active(
        &mut self,
        unsigned: UnsignedEvent,
        now_unix_seconds: u64,
    ) -> Result<SignedEvent, VaultError> {
        let label = self
            .active_label
            .clone()
            .ok_or(VaultError::NoActiveAccount)?;
        self.sign_event(&label, unsigned, now_unix_seconds)
    }

    #[cfg(feature = "nostr")]
    pub fn create_auth_event(
        &mut self,
        label: &str,
        challenge: &str,
        relay_url: &str,
        now_unix_seconds: u64,
    ) -> Result<SignedEvent, VaultError> {
        #[cfg(feature = "security-hardening")]
        let security = self.config.security;
        let entry = self
            .entries
            .get_mut(label)
            .ok_or(VaultError::MissingLabel)?;
        entry.last_used_unix_seconds = now_unix_seconds;
        #[cfg(feature = "security-hardening")]
        apply_security_before(security, &entry.secret);
        let event = neco_secp::nip42::create_auth_event(
            challenge,
            relay_url,
            &entry.secret,
            now_unix_seconds,
        )
        .map_err(VaultError::from)?;
        #[cfg(feature = "security-hardening")]
        apply_security_after(security, &entry.secret);
        Ok(event)
    }

    #[cfg(feature = "nostr")]
    pub fn create_auth_event_active(
        &mut self,
        challenge: &str,
        relay_url: &str,
        now_unix_seconds: u64,
    ) -> Result<SignedEvent, VaultError> {
        let label = self
            .active_label
            .clone()
            .ok_or(VaultError::NoActiveAccount)?;
        self.create_auth_event(&label, challenge, relay_url, now_unix_seconds)
    }

    pub fn clear_cache(&mut self) {
        self.entries.clear();
        self.active_label = None;
    }

    pub fn clear_expired_cache(&mut self, now_unix_seconds: u64) {
        let timeout = self.config.cache_timeout_seconds;
        self.entries.retain(|_, entry| {
            now_unix_seconds.saturating_sub(entry.last_used_unix_seconds) <= timeout
        });
        if self
            .active_label
            .as_deref()
            .is_some_and(|label| !self.entries.contains_key(label))
        {
            self.active_label = None;
        }
    }
}

#[cfg(feature = "nip04")]
impl Vault {
    pub fn nip04_encrypt(
        &mut self,
        label: &str,
        peer: &XOnlyPublicKey,
        plaintext: &str,
        now_unix_seconds: u64,
    ) -> Result<String, VaultError> {
        #[cfg(feature = "security-hardening")]
        let security = self.config.security;
        let entry = self
            .entries
            .get_mut(label)
            .ok_or(VaultError::MissingLabel)?;
        entry.last_used_unix_seconds = now_unix_seconds;
        #[cfg(feature = "security-hardening")]
        {
            apply_security_before(security, &entry.secret);
            if security.enable_dummy_operations {
                apply_dummy_nip04(&entry.secret);
            }
        }
        let payload = neco_secp::nip04::encrypt(&entry.secret, peer, plaintext, None)
            .map_err(VaultError::from)?;
        #[cfg(feature = "security-hardening")]
        apply_security_after(security, &entry.secret);
        Ok(payload)
    }

    pub fn nip04_decrypt(
        &mut self,
        label: &str,
        peer: &XOnlyPublicKey,
        payload: &str,
        now_unix_seconds: u64,
    ) -> Result<String, VaultError> {
        #[cfg(feature = "security-hardening")]
        let security = self.config.security;
        let entry = self
            .entries
            .get_mut(label)
            .ok_or(VaultError::MissingLabel)?;
        entry.last_used_unix_seconds = now_unix_seconds;
        #[cfg(feature = "security-hardening")]
        {
            apply_security_before(security, &entry.secret);
            if security.enable_dummy_operations {
                apply_dummy_nip04(&entry.secret);
            }
        }
        let plaintext =
            neco_secp::nip04::decrypt(&entry.secret, peer, payload).map_err(VaultError::from)?;
        #[cfg(feature = "security-hardening")]
        apply_security_after(security, &entry.secret);
        Ok(plaintext)
    }

    pub fn nip04_encrypt_active(
        &mut self,
        peer: &XOnlyPublicKey,
        plaintext: &str,
        now_unix_seconds: u64,
    ) -> Result<String, VaultError> {
        let label = self
            .active_label
            .as_deref()
            .ok_or(VaultError::NoActiveAccount)?
            .to_string();
        self.nip04_encrypt(&label, peer, plaintext, now_unix_seconds)
    }

    pub fn nip04_decrypt_active(
        &mut self,
        peer: &XOnlyPublicKey,
        payload: &str,
        now_unix_seconds: u64,
    ) -> Result<String, VaultError> {
        let label = self
            .active_label
            .as_deref()
            .ok_or(VaultError::NoActiveAccount)?
            .to_string();
        self.nip04_decrypt(&label, peer, payload, now_unix_seconds)
    }
}

#[cfg(feature = "nip44")]
impl Vault {
    pub fn nip44_encrypt(
        &mut self,
        label: &str,
        peer: &XOnlyPublicKey,
        plaintext: &str,
        now_unix_seconds: u64,
    ) -> Result<String, VaultError> {
        #[cfg(feature = "security-hardening")]
        let security = self.config.security;
        let entry = self
            .entries
            .get_mut(label)
            .ok_or(VaultError::MissingLabel)?;
        entry.last_used_unix_seconds = now_unix_seconds;
        #[cfg(feature = "security-hardening")]
        {
            apply_security_before(security, &entry.secret);
            if security.enable_dummy_operations {
                apply_dummy_nip44(&entry.secret);
            }
        }
        let conversation_key = neco_secp::nip44::get_conversation_key(&entry.secret, peer)
            .map_err(VaultError::from)?;
        let payload = neco_secp::nip44::encrypt(plaintext, &conversation_key, None)
            .map_err(VaultError::from)?;
        #[cfg(feature = "security-hardening")]
        apply_security_after(security, &entry.secret);
        Ok(payload)
    }

    pub fn nip44_decrypt(
        &mut self,
        label: &str,
        peer: &XOnlyPublicKey,
        payload: &str,
        now_unix_seconds: u64,
    ) -> Result<String, VaultError> {
        #[cfg(feature = "security-hardening")]
        let security = self.config.security;
        let entry = self
            .entries
            .get_mut(label)
            .ok_or(VaultError::MissingLabel)?;
        entry.last_used_unix_seconds = now_unix_seconds;
        #[cfg(feature = "security-hardening")]
        {
            apply_security_before(security, &entry.secret);
            if security.enable_dummy_operations {
                apply_dummy_nip44(&entry.secret);
            }
        }
        let conversation_key = neco_secp::nip44::get_conversation_key(&entry.secret, peer)
            .map_err(VaultError::from)?;
        let plaintext =
            neco_secp::nip44::decrypt(payload, &conversation_key).map_err(VaultError::from)?;
        #[cfg(feature = "security-hardening")]
        apply_security_after(security, &entry.secret);
        Ok(plaintext)
    }

    pub fn nip44_encrypt_active(
        &mut self,
        peer: &XOnlyPublicKey,
        plaintext: &str,
        now_unix_seconds: u64,
    ) -> Result<String, VaultError> {
        let label = self
            .active_label
            .as_deref()
            .ok_or(VaultError::NoActiveAccount)?
            .to_string();
        self.nip44_encrypt(&label, peer, plaintext, now_unix_seconds)
    }

    pub fn nip44_decrypt_active(
        &mut self,
        peer: &XOnlyPublicKey,
        payload: &str,
        now_unix_seconds: u64,
    ) -> Result<String, VaultError> {
        let label = self
            .active_label
            .as_deref()
            .ok_or(VaultError::NoActiveAccount)?
            .to_string();
        self.nip44_decrypt(&label, peer, payload, now_unix_seconds)
    }
}

#[cfg(feature = "nip17")]
impl Vault {
    pub fn create_sealed_dm(
        &mut self,
        label: &str,
        content: &str,
        recipient: &XOnlyPublicKey,
        now_unix_seconds: u64,
    ) -> Result<SignedEvent, VaultError> {
        #[cfg(feature = "security-hardening")]
        let security = self.config.security;
        let entry = self
            .entries
            .get_mut(label)
            .ok_or(VaultError::MissingLabel)?;
        entry.last_used_unix_seconds = now_unix_seconds;
        #[cfg(feature = "security-hardening")]
        {
            apply_security_before(security, &entry.secret);
            if security.enable_dummy_operations {
                apply_dummy_nip44(&entry.secret);
            }
        }
        let inner = UnsignedEvent {
            created_at: now_unix_seconds,
            kind: 14,
            tags: vec![vec!["p".to_string(), recipient.to_hex()]],
            content: content.to_string(),
        };
        let seal = neco_secp::nip17::create_seal(inner, &entry.secret, recipient)
            .map_err(VaultError::from)?;
        let gift_wrap =
            neco_secp::nip17::create_gift_wrap(&seal, recipient).map_err(VaultError::from)?;
        #[cfg(feature = "security-hardening")]
        apply_security_after(security, &entry.secret);
        Ok(gift_wrap)
    }

    pub fn create_sealed_dm_with_scan_tag(
        &mut self,
        label: &str,
        content: &str,
        recipient: &XOnlyPublicKey,
        recipient_scan_pub: &XOnlyPublicKey,
        now_unix_seconds: u64,
    ) -> Result<SealedDmResult, VaultError> {
        #[cfg(feature = "security-hardening")]
        let security = self.config.security;
        let entry = self
            .entries
            .get_mut(label)
            .ok_or(VaultError::MissingLabel)?;
        entry.last_used_unix_seconds = now_unix_seconds;
        #[cfg(feature = "security-hardening")]
        {
            apply_security_before(security, &entry.secret);
            if security.enable_dummy_operations {
                apply_dummy_nip44(&entry.secret);
            }
        }
        let inner = UnsignedEvent {
            created_at: now_unix_seconds,
            kind: 14,
            tags: vec![vec!["p".to_string(), recipient.to_hex()]],
            content: content.to_string(),
        };
        let seal = neco_secp::nip17::create_seal(inner, &entry.secret, recipient)
            .map_err(VaultError::from)?;
        let result =
            neco_secp::nip17::create_gift_wrap_with_scan_tag(&seal, recipient, recipient_scan_pub)
                .map_err(VaultError::from)?;
        #[cfg(feature = "security-hardening")]
        apply_security_after(security, &entry.secret);
        Ok(SealedDmResult {
            event: result.event,
            scanning_tag: result.scanning_tag,
        })
    }

    /// Compute scanning tag using an explicit scan private key.
    /// scan_priv is independent from the signing key (DJ-I1).
    pub fn compute_scan_tag(
        scan_priv: &SecretKey,
        ephemeral_pubkey: &XOnlyPublicKey,
    ) -> Result<[u8; 16], VaultError> {
        neco_secp::nip17::compute_scan_tag(scan_priv, ephemeral_pubkey).map_err(VaultError::from)
    }

    pub fn open_gift_wrap_dm(
        &mut self,
        label: &str,
        gift_wrap: &SignedEvent,
        now_unix_seconds: u64,
    ) -> Result<SignedEvent, VaultError> {
        #[cfg(feature = "security-hardening")]
        let security = self.config.security;
        let entry = self
            .entries
            .get_mut(label)
            .ok_or(VaultError::MissingLabel)?;
        entry.last_used_unix_seconds = now_unix_seconds;
        #[cfg(feature = "security-hardening")]
        {
            apply_security_before(security, &entry.secret);
            if security.enable_dummy_operations {
                apply_dummy_nip44(&entry.secret);
            }
        }
        let inner =
            neco_secp::nip17::open_gift_wrap(gift_wrap, &entry.secret).map_err(VaultError::from)?;
        #[cfg(feature = "security-hardening")]
        apply_security_after(security, &entry.secret);
        Ok(inner)
    }
}

#[cfg(feature = "encrypted")]
impl Vault {
    pub fn export_encrypted(&self, label: &str, passphrase: &[u8]) -> Result<Vec<u8>, VaultError> {
        let entry = self.entries.get(label).ok_or(VaultError::MissingLabel)?;
        let mut salt = [0u8; 32];
        let mut iv = [0u8; 16];
        getrandom::getrandom(&mut salt)
            .map_err(|_| VaultError::InvalidEncrypted("failed to generate salt"))?;
        getrandom::getrandom(&mut iv)
            .map_err(|_| VaultError::InvalidEncrypted("failed to generate iv"))?;
        let key = scrypt_derive(
            passphrase,
            &salt,
            ENCRYPTED_V2_LOG_N,
            ENCRYPTED_V2_R,
            ENCRYPTED_V2_P,
        )?;
        let ciphertext = aes256_cbc_encrypt(&key, &iv, &entry.secret.to_bytes())?;
        let mut out = Vec::with_capacity(ENCRYPTED_V2_LEN);
        out.push(ENCRYPTED_V2_VERSION);
        out.push(ENCRYPTED_V2_LOG_N);
        out.push(ENCRYPTED_V2_R);
        out.push(ENCRYPTED_V2_P);
        out.extend_from_slice(&salt);
        out.extend_from_slice(&iv);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    pub fn import_encrypted(
        &mut self,
        label: &str,
        passphrase: &[u8],
        data: &[u8],
        now_unix_seconds: u64,
    ) -> Result<(), VaultError> {
        let (key, iv, ciphertext) =
            if data.len() == ENCRYPTED_V2_LEN && data[0] == ENCRYPTED_V2_VERSION {
                let log_n = data[1];
                let r = data[2];
                let p = data[3];
                let salt: [u8; 32] = data[4..36]
                    .try_into()
                    .map_err(|_| VaultError::InvalidEncrypted("invalid salt"))?;
                let iv: [u8; 16] = data[36..52]
                    .try_into()
                    .map_err(|_| VaultError::InvalidEncrypted("invalid iv"))?;
                let key = scrypt_derive(passphrase, &salt, log_n, r, p)?;
                (key, iv, &data[52..])
            } else {
                #[cfg(feature = "encrypted-legacy-v1")]
                if data.len() == ENCRYPTED_V1_LEN {
                    let iv: [u8; 16] = data[..16]
                        .try_into()
                        .map_err(|_| VaultError::InvalidEncrypted("invalid iv"))?;
                    (sha256(passphrase), iv, &data[16..])
                } else {
                    return Err(VaultError::InvalidEncrypted("invalid encrypted payload"));
                }

                #[cfg(not(feature = "encrypted-legacy-v1"))]
                {
                    return Err(VaultError::InvalidEncrypted("invalid encrypted payload"));
                }
            };
        let plaintext = aes256_cbc_decrypt(&key, &iv, ciphertext)?;
        let secret_bytes: [u8; 32] = plaintext
            .as_slice()
            .try_into()
            .map_err(|_| VaultError::InvalidEncrypted("invalid secret key"))?;
        let secret = SecretKey::from_bytes(secret_bytes)
            .map_err(|_| VaultError::InvalidEncrypted("invalid secret key"))?;
        self.import_plaintext(label, secret, now_unix_seconds)
    }
}

// --- epoch key + group key API (dm-epoch feature) ---

#[cfg(feature = "dm-epoch")]
impl Vault {
    /// Derive an epoch key for a 1:1 DM conversation.
    ///
    /// `epoch_secret` is the NIP-44 conversation key (ECDH) between `label`'s
    /// secret key and `peer_pubkey`.  The epoch key chain is then:
    ///
    /// ```text
    /// epoch_key[0] = HKDF-SHA256(epoch_secret, "dm-epoch-0", 32)
    /// epoch_key[n] = HKDF-SHA256(epoch_key[n-1], "dm-epoch-rotate", 32)
    /// ```
    pub fn derive_epoch_key(
        &self,
        label: &str,
        peer_pubkey: &XOnlyPublicKey,
        epoch_number: u32,
    ) -> Result<[u8; 32], VaultError> {
        let entry = self.entries.get(label).ok_or(VaultError::MissingLabel)?;
        let conversation_key = neco_secp::nip44::get_conversation_key(&entry.secret, peer_pubkey)?;
        let prk = neco_sha2::Prk::from_bytes(&conversation_key);
        let epoch0_bytes = prk
            .expand(b"dm-epoch-0", 32)
            .map_err(|_| VaultError::InvalidEncrypted("HKDF expand failed"))?;
        let mut key = [0u8; 32];
        key.copy_from_slice(&epoch0_bytes);
        for _ in 0..epoch_number {
            key = Self::hkdf_rotate(&key, b"dm-epoch-rotate");
        }
        Ok(key)
    }

    /// Encrypt `plaintext` with a symmetric epoch key using ChaCha20-Poly1305.
    ///
    /// Returns `nonce (12 bytes) || ciphertext || tag (16 bytes)`.
    pub fn encrypt_with_epoch(
        epoch_key: &[u8; 32],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        use chacha20poly1305::aead::{Aead, KeyInit};
        use chacha20poly1305::ChaCha20Poly1305;

        let cipher = ChaCha20Poly1305::new(epoch_key.into());
        let mut nonce = [0u8; 12];
        getrandom::getrandom(&mut nonce)
            .map_err(|_| VaultError::InvalidEncrypted("CSPRNG failed"))?;
        let ciphertext = cipher
            .encrypt(chacha20poly1305::Nonce::from_slice(&nonce), plaintext)
            .map_err(|_| VaultError::InvalidEncrypted("epoch encryption failed"))?;
        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Decrypt ciphertext produced by [`encrypt_with_epoch`](Self::encrypt_with_epoch).
    ///
    /// Expects `nonce (12 bytes) || ciphertext || tag (16 bytes)`.
    pub fn decrypt_with_epoch(
        epoch_key: &[u8; 32],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        use chacha20poly1305::aead::{Aead, KeyInit};
        use chacha20poly1305::ChaCha20Poly1305;

        if ciphertext.len() < 12 + 16 {
            return Err(VaultError::InvalidEncrypted("epoch ciphertext too short"));
        }
        let nonce = &ciphertext[..12];
        let ct = &ciphertext[12..];
        let cipher = ChaCha20Poly1305::new(epoch_key.into());
        cipher
            .decrypt(chacha20poly1305::Nonce::from_slice(nonce), ct)
            .map_err(|_| VaultError::InvalidEncrypted("epoch decryption failed"))
    }

    /// Generate a fresh random group key (32 bytes from CSPRNG).
    pub fn create_group_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        // getrandom can only fail on unsupported platforms; panic is acceptable
        // in production, but we avoid unwrap() outside tests per project rules.
        let _ = getrandom::getrandom(&mut key);
        key
    }

    /// Encrypt a group key for a recipient using an ephemeral ECDH key pair
    /// and NIP-44 encryption.
    ///
    /// Returns `ephemeral_pubkey (32 bytes) || nip44_payload (base64-encoded string as UTF-8 bytes)`.
    pub fn encrypt_group_key(
        group_key: &[u8; 32],
        recipient_pubkey: &XOnlyPublicKey,
    ) -> Result<Vec<u8>, VaultError> {
        let ephemeral = SecretKey::generate()?;
        let ephemeral_pub = ephemeral.xonly_public_key()?;
        let conversation_key =
            neco_secp::nip44::get_conversation_key(&ephemeral, recipient_pubkey)?;
        let hex_payload: String = group_key.iter().map(|b| format!("{b:02x}")).collect();
        let encrypted = neco_secp::nip44::encrypt(&hex_payload, &conversation_key, None)?;
        let encrypted_bytes = encrypted.as_bytes();
        let mut out = Vec::with_capacity(32 + encrypted_bytes.len());
        out.extend_from_slice(&ephemeral_pub.to_bytes());
        out.extend_from_slice(encrypted_bytes);
        Ok(out)
    }

    /// Decrypt a group key that was encrypted with [`encrypt_group_key`](Self::encrypt_group_key).
    pub fn decrypt_group_key(&self, label: &str, encrypted: &[u8]) -> Result<[u8; 32], VaultError> {
        if encrypted.len() < 33 {
            return Err(VaultError::InvalidEncrypted(
                "encrypted group key too short",
            ));
        }
        let entry = self.entries.get(label).ok_or(VaultError::MissingLabel)?;
        let ephemeral_pub_bytes: [u8; 32] = encrypted[..32]
            .try_into()
            .map_err(|_| VaultError::InvalidEncrypted("invalid ephemeral pubkey"))?;
        let ephemeral_pub = XOnlyPublicKey::from_bytes(ephemeral_pub_bytes)
            .map_err(|_| VaultError::InvalidEncrypted("invalid ephemeral pubkey"))?;
        let conversation_key =
            neco_secp::nip44::get_conversation_key(&entry.secret, &ephemeral_pub)?;
        let payload_str = core::str::from_utf8(&encrypted[32..])
            .map_err(|_| VaultError::InvalidEncrypted("invalid group key payload"))?;
        let hex_str = neco_secp::nip44::decrypt(payload_str, &conversation_key)?;
        let bytes =
            hex_to_bytes(&hex_str).ok_or(VaultError::InvalidEncrypted("invalid group key hex"))?;
        if bytes.len() != 32 {
            return Err(VaultError::InvalidEncrypted("group key must be 32 bytes"));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(key)
    }

    /// Encrypt a group message (alias for [`encrypt_with_epoch`](Self::encrypt_with_epoch)).
    pub fn encrypt_group_message(
        group_epoch_key: &[u8; 32],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        Self::encrypt_with_epoch(group_epoch_key, plaintext)
    }

    /// Decrypt a group message (alias for [`decrypt_with_epoch`](Self::decrypt_with_epoch)).
    pub fn decrypt_group_message(
        group_epoch_key: &[u8; 32],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        Self::decrypt_with_epoch(group_epoch_key, ciphertext)
    }

    /// Rotate a group epoch key: `HKDF-SHA256(current_key, "group-epoch-rotate", 32)`.
    pub fn rotate_group_epoch(current_key: &[u8; 32]) -> [u8; 32] {
        Self::hkdf_rotate(current_key, b"group-epoch-rotate")
    }

    // --- internal helpers ---

    fn hkdf_rotate(key: &[u8; 32], info: &[u8]) -> [u8; 32] {
        let prk = neco_sha2::Prk::from_bytes(key);
        // 32 bytes is always within HKDF limits; safe to expect in private helper.
        let expanded = prk.expand(info, 32).expect("HKDF expand 32 bytes");
        let mut out = [0u8; 32];
        out.copy_from_slice(&expanded);
        out
    }
}

#[cfg(feature = "dm-epoch")]
fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "encrypted-legacy-v1")]
    fn legacy_v1_payload(secret: &SecretKey, passphrase: &[u8]) -> Vec<u8> {
        let key = sha256(passphrase);
        let iv = [0x55; 16];
        let ciphertext = aes256_cbc_encrypt(&key, &iv, &secret.to_bytes()).expect("legacy encrypt");
        let mut exported = Vec::with_capacity(ENCRYPTED_V1_LEN);
        exported.extend_from_slice(&iv);
        exported.extend_from_slice(&ciphertext);
        exported
    }

    #[cfg(all(feature = "encrypted", not(feature = "encrypted-legacy-v1")))]
    fn legacy_v1_payload(secret: &SecretKey, passphrase: &[u8]) -> Vec<u8> {
        use scrypt::Params as ScryptParams;

        let mut out = [0u8; 32];
        let params = ScryptParams::new(15, 8, 1, 32).expect("scrypt params");
        scrypt::scrypt(passphrase, &[0u8; 32], &params, &mut out).expect("scrypt");
        let iv = [0x55; 16];
        let ciphertext = aes256_cbc_encrypt(&out, &iv, &secret.to_bytes()).expect("ciphertext");
        let mut exported = Vec::with_capacity(64);
        exported.extend_from_slice(&iv);
        exported.extend_from_slice(&ciphertext);
        exported
    }

    #[test]
    fn first_import_sets_active() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let secret = SecretKey::generate().expect("secret");
        vault.import_plaintext("main", secret, 100).expect("import");
        assert_eq!(vault.active_label(), Some("main"));
    }

    #[test]
    fn second_import_does_not_change_active() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        vault
            .import_plaintext("main", SecretKey::generate().expect("main secret"), 100)
            .expect("first import");
        vault
            .import_plaintext("backup", SecretKey::generate().expect("backup secret"), 101)
            .expect("second import");
        assert_eq!(vault.active_label(), Some("main"));
    }

    #[test]
    fn set_active_switches_label() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        vault
            .import_plaintext("main", SecretKey::generate().expect("main secret"), 100)
            .expect("first import");
        vault
            .import_plaintext("backup", SecretKey::generate().expect("backup secret"), 101)
            .expect("second import");
        vault.set_active("backup").expect("set active");
        assert_eq!(vault.active_label(), Some("backup"));
    }

    #[test]
    fn set_active_missing_label_fails() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let error = vault
            .set_active("missing")
            .expect_err("missing label must fail");
        assert!(matches!(error, VaultError::MissingLabel));
    }

    #[test]
    fn remove_active_sets_none() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        vault
            .import_plaintext("main", SecretKey::generate().expect("secret"), 100)
            .expect("import");
        vault.remove("main").expect("remove");
        assert_eq!(vault.active_label(), None);
    }

    #[test]
    fn remove_non_active_keeps_active() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        vault
            .import_plaintext("main", SecretKey::generate().expect("main secret"), 100)
            .expect("first import");
        vault
            .import_plaintext("backup", SecretKey::generate().expect("backup secret"), 101)
            .expect("second import");
        vault.remove("backup").expect("remove");
        assert_eq!(vault.active_label(), Some("main"));
    }

    #[test]
    fn labels_returns_all() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        vault
            .import_plaintext("main", SecretKey::generate().expect("main secret"), 100)
            .expect("first import");
        vault
            .import_plaintext("backup", SecretKey::generate().expect("backup secret"), 101)
            .expect("second import");
        assert_eq!(vault.labels(), vec!["backup", "main"]);
    }

    #[test]
    fn public_key_returns_expected_xonly() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let secret = SecretKey::generate().expect("secret");
        let expected = secret.xonly_public_key().expect("public key");
        vault.import_plaintext("main", secret, 100).expect("import");
        assert_eq!(
            vault.public_key("main").expect("vault public key"),
            expected
        );
    }

    #[test]
    fn public_key_active_returns_expected_xonly() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let secret = SecretKey::generate().expect("secret");
        let expected = secret.xonly_public_key().expect("public key");
        vault.import_plaintext("main", secret, 100).expect("import");
        assert_eq!(
            vault.public_key_active().expect("active public key"),
            expected
        );
    }

    #[test]
    fn public_key_missing_label_fails() {
        let vault = Vault::new(VaultConfig::default()).expect("vault");
        let error = vault
            .public_key("missing")
            .expect_err("missing label must fail");
        assert!(matches!(error, VaultError::MissingLabel));
    }

    #[test]
    fn public_key_active_without_active_fails() {
        let vault = Vault::new(VaultConfig::default()).expect("vault");
        let error = vault
            .public_key_active()
            .expect_err("active label must exist");
        assert!(matches!(error, VaultError::NoActiveAccount));
    }

    #[test]
    fn default_security_config_matches_spec() {
        let vault = Vault::new(VaultConfig::default()).expect("vault");
        assert_eq!(
            vault.security_config(),
            SecurityConfig {
                enable_constant_time: true,
                enable_random_delay: false,
                enable_dummy_operations: false,
            }
        );
    }

    #[test]
    fn set_security_config_updates_vault_immediately() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let security = SecurityConfig {
            enable_constant_time: false,
            enable_random_delay: true,
            enable_dummy_operations: true,
        };
        vault.set_security_config(security);
        assert_eq!(vault.security_config(), security);
    }

    #[test]
    fn clear_cache_resets_active() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        vault
            .import_plaintext("main", SecretKey::generate().expect("secret"), 100)
            .expect("import");
        vault.clear_cache();
        assert_eq!(vault.active_label(), None);
    }

    #[test]
    fn clear_expired_resets_active_when_expired() {
        let mut vault = Vault::new(VaultConfig {
            cache_timeout_seconds: 10,
            security: SecurityConfig::default(),
        })
        .expect("vault");
        vault
            .import_plaintext("main", SecretKey::generate().expect("secret"), 100)
            .expect("import");
        vault.clear_expired_cache(111);
        assert_eq!(vault.active_label(), None);
    }

    #[test]
    fn clear_expired_keeps_active_when_fresh() {
        let mut vault = Vault::new(VaultConfig {
            cache_timeout_seconds: 10,
            security: SecurityConfig::default(),
        })
        .expect("vault");
        vault
            .import_plaintext("main", SecretKey::generate().expect("secret"), 100)
            .expect("import");
        vault.clear_expired_cache(110);
        assert_eq!(vault.active_label(), Some("main"));
    }

    #[test]
    #[cfg(feature = "nostr")]
    fn sign_event_active_works() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let secret = SecretKey::generate().expect("secret");
        vault.import_plaintext("main", secret, 100).expect("import");

        let unsigned = UnsignedEvent {
            created_at: 101,
            kind: 1,
            tags: Vec::new(),
            content: "hello".to_string(),
        };

        let signed = vault.sign_event_active(unsigned, 102).expect("sign");
        nostr::verify_event(&signed).expect("verify");
    }

    #[test]
    fn duplicate_label_is_rejected() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let secret = SecretKey::generate().expect("secret");
        vault
            .import_plaintext("main", secret, 100)
            .expect("first import");
        let error = vault
            .import_plaintext("main", secret, 101)
            .expect_err("must reject duplicate");
        assert!(matches!(error, VaultError::DuplicateLabel));
    }

    #[test]
    #[cfg(feature = "nostr")]
    fn sign_event_active_no_active_fails() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let unsigned = UnsignedEvent {
            created_at: 101,
            kind: 1,
            tags: Vec::new(),
            content: "hello".to_string(),
        };
        let error = vault
            .sign_event_active(unsigned, 102)
            .expect_err("active label must exist");
        assert!(matches!(error, VaultError::NoActiveAccount));
    }

    #[test]
    #[cfg(feature = "nostr")]
    fn missing_label_is_rejected() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let unsigned = UnsignedEvent {
            created_at: 101,
            kind: 1,
            tags: Vec::new(),
            content: "hello".to_string(),
        };
        let error = vault
            .sign_event("missing", unsigned, 102)
            .expect_err("missing label must fail");
        assert!(matches!(error, VaultError::MissingLabel));
    }

    #[test]
    fn clear_expired_cache_removes_old_entries() {
        let mut vault = Vault::new(VaultConfig {
            cache_timeout_seconds: 10,
            security: SecurityConfig::default(),
        })
        .expect("vault");
        let secret = SecretKey::generate().expect("secret");
        vault.import_plaintext("main", secret, 100).expect("import");
        vault.clear_expired_cache(111);
        assert!(!vault.contains("main"));
    }

    #[test]
    fn clear_expired_cache_keeps_entry_at_timeout_boundary() {
        let mut vault = Vault::new(VaultConfig {
            cache_timeout_seconds: 10,
            security: SecurityConfig::default(),
        })
        .expect("vault");
        let secret = SecretKey::generate().expect("secret");
        vault.import_plaintext("main", secret, 100).expect("import");
        vault.clear_expired_cache(110);
        assert!(vault.contains("main"));
    }

    #[test]
    fn clear_expired_cache_removes_only_expired_labels() {
        let mut vault = Vault::new(VaultConfig {
            cache_timeout_seconds: 10,
            security: SecurityConfig::default(),
        })
        .expect("vault");
        vault
            .import_plaintext("old", SecretKey::generate().expect("old secret"), 100)
            .expect("old import");
        vault
            .import_plaintext("fresh", SecretKey::generate().expect("fresh secret"), 105)
            .expect("fresh import");
        vault.clear_expired_cache(111);
        assert!(!vault.contains("old"));
        assert!(vault.contains("fresh"));
    }

    #[test]
    #[cfg(feature = "nostr")]
    fn vault_sign_matches_core_finalize_event() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let secret = SecretKey::generate().expect("secret");
        vault.import_plaintext("main", secret, 100).expect("import");

        let unsigned = UnsignedEvent {
            created_at: 101,
            kind: 7,
            tags: vec![vec!["t".to_string(), "rust".to_string()]],
            content: "hello".to_string(),
        };

        // vault と core 経路はどちらも BIP-340 推奨のランダム aux_rand で
        // 署名するため、sig のバイト列は呼び出しごとに異なる。よって等価性は
        // 「id/pubkey/created_at/kind/tags/content が一致し、両方とも verify OK」
        // という意味不変量で検証する。
        let from_vault = vault
            .sign_event("main", unsigned.clone(), 102)
            .expect("vault sign");
        let from_core = nostr::finalize_event(unsigned, &secret).expect("core sign");
        assert_eq!(from_vault.id, from_core.id);
        assert_eq!(from_vault.pubkey, from_core.pubkey);
        assert_eq!(from_vault.created_at, from_core.created_at);
        assert_eq!(from_vault.kind, from_core.kind);
        assert_eq!(from_vault.tags, from_core.tags);
        assert_eq!(from_vault.content, from_core.content);
        nostr::verify_event(&from_vault).expect("verify vault");
        nostr::verify_event(&from_core).expect("verify core");
    }

    #[test]
    #[cfg(feature = "nostr")]
    fn vault_nip42_create_auth() {
        let mut vault = Vault::new(VaultConfig {
            cache_timeout_seconds: 10,
            security: SecurityConfig::default(),
        })
        .expect("vault");
        let secret = SecretKey::generate().expect("secret");
        let expected = secret.xonly_public_key().expect("pubkey");
        vault.import_plaintext("main", secret, 100).expect("import");

        let event = vault
            .create_auth_event("main", "challenge-123", "wss://relay.example.com", 105)
            .expect("create auth event");

        assert_eq!(
            neco_secp::nip42::validate_auth_event(
                &event,
                "challenge-123",
                "wss://relay.example.com"
            )
            .expect("validate auth event"),
            expected
        );

        vault.clear_expired_cache(115);
        assert!(vault.contains("main"));
    }

    #[test]
    #[cfg(feature = "nostr")]
    fn vault_nip42_active_auth() {
        let mut vault = Vault::new(VaultConfig {
            cache_timeout_seconds: 10,
            security: SecurityConfig::default(),
        })
        .expect("vault");
        let secret = SecretKey::generate().expect("secret");
        let expected = secret.xonly_public_key().expect("pubkey");
        vault.import_plaintext("main", secret, 100).expect("import");

        let event = vault
            .create_auth_event_active("challenge-456", "wss://relay.example.com", 105)
            .expect("create auth event");

        assert_eq!(
            neco_secp::nip42::validate_auth_event(
                &event,
                "challenge-456",
                "wss://relay.example.com"
            )
            .expect("validate auth event"),
            expected
        );

        vault.clear_expired_cache(115);
        assert!(vault.contains("main"));
    }

    #[test]
    #[cfg(feature = "encrypted")]
    fn encrypted_v2_roundtrip() {
        let mut source = Vault::new(VaultConfig::default()).expect("source vault");
        let mut dest = Vault::new(VaultConfig::default()).expect("dest vault");
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        source
            .import_plaintext("main", secret, 100)
            .expect("source import");

        let exported = source
            .export_encrypted("main", b"passphrase")
            .expect("export encrypted");
        assert_eq!(exported.len(), ENCRYPTED_V2_LEN);
        assert_eq!(exported[0], ENCRYPTED_V2_VERSION);

        dest.import_encrypted("main", b"passphrase", &exported, 200)
            .expect("import encrypted");
        assert!(dest.contains("main"));
    }

    #[test]
    #[cfg(all(feature = "encrypted", not(feature = "encrypted-legacy-v1")))]
    fn encrypted_v1_payload_rejected_without_legacy_feature() {
        let mut dest = Vault::new(VaultConfig::default()).expect("dest vault");
        let secret = SecretKey::from_bytes([0x44; 32]).expect("secret");
        let exported = legacy_v1_payload(&secret, b"passphrase");

        let error = dest
            .import_encrypted("main", b"passphrase", &exported, 200)
            .expect_err("v1 payload must be rejected by default");
        assert!(matches!(
            error,
            VaultError::InvalidEncrypted("invalid encrypted payload")
        ));
    }

    #[test]
    #[cfg(all(feature = "encrypted", feature = "encrypted-legacy-v1"))]
    fn encrypted_v1_backward_compat() {
        let mut dest = Vault::new(VaultConfig::default()).expect("dest vault");
        let secret = SecretKey::from_bytes([0x44; 32]).expect("secret");
        let exported = legacy_v1_payload(&secret, b"passphrase");

        dest.import_encrypted("main", b"passphrase", &exported, 200)
            .expect("import encrypted");
        assert!(dest.contains("main"));
        assert_eq!(
            dest.public_key("main").expect("public key"),
            secret.xonly_public_key().expect("expected public key")
        );
    }

    #[test]
    #[cfg(feature = "encrypted")]
    fn encrypted_wrong_passphrase_fails() {
        let mut source = Vault::new(VaultConfig::default()).expect("source vault");
        let mut dest = Vault::new(VaultConfig::default()).expect("dest vault");
        source
            .import_plaintext(
                "main",
                SecretKey::from_bytes([0x22; 32]).expect("secret"),
                100,
            )
            .expect("source import");
        let exported = source
            .export_encrypted("main", b"correct")
            .expect("export encrypted");

        let error = dest
            .import_encrypted("main", b"wrong", &exported, 200)
            .expect_err("wrong passphrase must fail");
        assert!(matches!(
            error,
            VaultError::InvalidEncrypted("failed to decrypt")
        ));
    }

    #[test]
    #[cfg(feature = "encrypted")]
    fn encrypted_invalid_data_fails() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let error = vault
            .import_encrypted("main", b"passphrase", &[1, 2, 3], 100)
            .expect_err("invalid data must fail");
        assert!(matches!(
            error,
            VaultError::InvalidEncrypted("invalid encrypted payload")
        ));
    }

    #[test]
    #[cfg(feature = "encrypted")]
    fn encrypted_import_sets_active_on_empty_vault() {
        let mut source = Vault::new(VaultConfig::default()).expect("source vault");
        let mut dest = Vault::new(VaultConfig::default()).expect("dest vault");
        source
            .import_plaintext(
                "main",
                SecretKey::from_bytes([0x33; 32]).expect("secret"),
                100,
            )
            .expect("source import");
        let exported = source
            .export_encrypted("main", b"passphrase")
            .expect("export encrypted");

        dest.import_encrypted("main", b"passphrase", &exported, 200)
            .expect("import encrypted");
        assert_eq!(dest.active_label(), Some("main"));
    }

    #[test]
    #[cfg(feature = "encrypted")]
    fn encrypted_export_missing_label_fails() {
        let vault = Vault::new(VaultConfig::default()).expect("vault");
        let error = vault
            .export_encrypted("missing", b"passphrase")
            .expect_err("missing label must fail");
        assert!(matches!(error, VaultError::MissingLabel));
    }

    #[test]
    #[cfg(feature = "nip04")]
    fn nip04_roundtrip_between_vaults() {
        let mut alice = Vault::new(VaultConfig::default()).expect("alice vault");
        let mut bob = Vault::new(VaultConfig::default()).expect("bob vault");
        let alice_secret = SecretKey::generate().expect("alice secret");
        let bob_secret = SecretKey::generate().expect("bob secret");
        let alice_pubkey = alice_secret.xonly_public_key().expect("alice pubkey");
        let bob_pubkey = bob_secret.xonly_public_key().expect("bob pubkey");
        alice
            .import_plaintext("alice", alice_secret, 100)
            .expect("alice import");
        bob.import_plaintext("bob", bob_secret, 100)
            .expect("bob import");

        let payload = alice
            .nip04_encrypt("alice", &bob_pubkey, "hello", 101)
            .expect("encrypt");
        let plaintext = bob
            .nip04_decrypt("bob", &alice_pubkey, &payload, 102)
            .expect("decrypt");
        assert_eq!(plaintext, "hello");
    }

    #[test]
    #[cfg(feature = "nip04")]
    fn nip04_active_roundtrip_between_vaults() {
        let mut alice = Vault::new(VaultConfig::default()).expect("alice vault");
        let mut bob = Vault::new(VaultConfig::default()).expect("bob vault");
        let alice_secret = SecretKey::generate().expect("alice secret");
        let bob_secret = SecretKey::generate().expect("bob secret");
        let alice_pubkey = alice_secret.xonly_public_key().expect("alice pubkey");
        let bob_pubkey = bob_secret.xonly_public_key().expect("bob pubkey");
        alice
            .import_plaintext("alice", alice_secret, 100)
            .expect("alice import");
        bob.import_plaintext("bob", bob_secret, 100)
            .expect("bob import");

        let payload = alice
            .nip04_encrypt_active(&bob_pubkey, "hello", 101)
            .expect("encrypt");
        let plaintext = bob
            .nip04_decrypt_active(&alice_pubkey, &payload, 102)
            .expect("decrypt");
        assert_eq!(plaintext, "hello");
    }

    #[test]
    #[cfg(feature = "nip04")]
    fn nip04_missing_label_fails() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let peer = SecretKey::generate()
            .expect("peer secret")
            .xonly_public_key()
            .expect("peer pubkey");
        let error = vault
            .nip04_encrypt("missing", &peer, "hello", 100)
            .expect_err("missing label must fail");
        assert!(matches!(error, VaultError::MissingLabel));
    }

    #[test]
    #[cfg(feature = "nip04")]
    fn nip04_active_without_active_fails() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let peer = SecretKey::generate()
            .expect("peer secret")
            .xonly_public_key()
            .expect("peer pubkey");
        let error = vault
            .nip04_encrypt_active(&peer, "hello", 100)
            .expect_err("active label must exist");
        assert!(matches!(error, VaultError::NoActiveAccount));
    }

    #[test]
    #[cfg(feature = "nip44")]
    fn nip44_roundtrip_between_vaults() {
        let mut alice = Vault::new(VaultConfig::default()).expect("alice vault");
        let mut bob = Vault::new(VaultConfig::default()).expect("bob vault");
        let alice_secret = SecretKey::generate().expect("alice secret");
        let bob_secret = SecretKey::generate().expect("bob secret");
        let alice_pubkey = alice_secret.xonly_public_key().expect("alice pubkey");
        let bob_pubkey = bob_secret.xonly_public_key().expect("bob pubkey");
        alice
            .import_plaintext("alice", alice_secret, 100)
            .expect("alice import");
        bob.import_plaintext("bob", bob_secret, 100)
            .expect("bob import");

        let payload = alice
            .nip44_encrypt("alice", &bob_pubkey, "hello", 101)
            .expect("encrypt");
        let plaintext = bob
            .nip44_decrypt("bob", &alice_pubkey, &payload, 102)
            .expect("decrypt");
        assert_eq!(plaintext, "hello");
    }

    #[test]
    #[cfg(feature = "nip44")]
    fn nip44_active_roundtrip_between_vaults() {
        let mut alice = Vault::new(VaultConfig::default()).expect("alice vault");
        let mut bob = Vault::new(VaultConfig::default()).expect("bob vault");
        let alice_secret = SecretKey::generate().expect("alice secret");
        let bob_secret = SecretKey::generate().expect("bob secret");
        let alice_pubkey = alice_secret.xonly_public_key().expect("alice pubkey");
        let bob_pubkey = bob_secret.xonly_public_key().expect("bob pubkey");
        alice
            .import_plaintext("alice", alice_secret, 100)
            .expect("alice import");
        bob.import_plaintext("bob", bob_secret, 100)
            .expect("bob import");

        let payload = alice
            .nip44_encrypt_active(&bob_pubkey, "hello", 101)
            .expect("encrypt");
        let plaintext = bob
            .nip44_decrypt_active(&alice_pubkey, &payload, 102)
            .expect("decrypt");
        assert_eq!(plaintext, "hello");
    }

    #[test]
    #[cfg(feature = "nip44")]
    fn nip44_missing_label_fails() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let peer = SecretKey::generate()
            .expect("peer secret")
            .xonly_public_key()
            .expect("peer pubkey");
        let error = vault
            .nip44_encrypt("missing", &peer, "hello", 100)
            .expect_err("missing label must fail");
        assert!(matches!(error, VaultError::MissingLabel));
    }

    #[test]
    #[cfg(feature = "nip44")]
    fn nip44_active_without_active_fails() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let peer = SecretKey::generate()
            .expect("peer secret")
            .xonly_public_key()
            .expect("peer pubkey");
        let error = vault
            .nip44_encrypt_active(&peer, "hello", 100)
            .expect_err("active label must exist");
        assert!(matches!(error, VaultError::NoActiveAccount));
    }

    #[test]
    #[cfg(all(feature = "security-hardening", feature = "nip44"))]
    fn security_hardening_preserves_nip44_roundtrip() {
        let mut alice = Vault::new(VaultConfig::default()).expect("alice vault");
        let mut bob = Vault::new(VaultConfig::default()).expect("bob vault");
        let alice_secret = SecretKey::generate().expect("alice secret");
        let bob_secret = SecretKey::generate().expect("bob secret");
        let alice_pubkey = alice_secret.xonly_public_key().expect("alice pubkey");
        let bob_pubkey = bob_secret.xonly_public_key().expect("bob pubkey");
        let security = SecurityConfig {
            enable_constant_time: true,
            enable_random_delay: true,
            enable_dummy_operations: true,
        };
        alice.set_security_config(security);
        bob.set_security_config(security);
        alice
            .import_plaintext("alice", alice_secret, 100)
            .expect("alice import");
        bob.import_plaintext("bob", bob_secret, 100)
            .expect("bob import");

        let payload = alice
            .nip44_encrypt("alice", &bob_pubkey, "hello", 101)
            .expect("encrypt");
        let plaintext = bob
            .nip44_decrypt("bob", &alice_pubkey, &payload, 102)
            .expect("decrypt");
        assert_eq!(plaintext, "hello");
    }

    #[test]
    #[cfg(feature = "nip17")]
    fn vault_nip17_dm_roundtrip() {
        let mut vault_sender = Vault::new(VaultConfig::default()).expect("vault");
        let mut vault_recipient = Vault::new(VaultConfig::default()).expect("vault");
        let sender_secret = SecretKey::generate().expect("sender");
        let recipient_secret = SecretKey::generate().expect("recipient");
        vault_sender
            .import_plaintext("sender", sender_secret, 100)
            .expect("import");
        vault_recipient
            .import_plaintext("recipient", recipient_secret, 100)
            .expect("import");
        let recipient_pubkey = vault_recipient.public_key("recipient").expect("pubkey");
        let gift_wrap = vault_sender
            .create_sealed_dm("sender", "hello via vault", &recipient_pubkey, 101)
            .expect("create dm");
        assert_eq!(gift_wrap.kind, 1059);
        let inner = vault_recipient
            .open_gift_wrap_dm("recipient", &gift_wrap, 102)
            .expect("open dm");
        assert_eq!(inner.kind, 14);
        assert_eq!(inner.content, "hello via vault");
    }

    #[test]
    #[cfg(feature = "nip17")]
    fn vault_nip17_missing_label_fails() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let peer = SecretKey::generate()
            .expect("s")
            .xonly_public_key()
            .expect("x");
        assert!(vault
            .create_sealed_dm("missing", "test", &peer, 100)
            .is_err());
    }

    #[test]
    #[cfg(feature = "nip17")]
    fn vault_nip17_scan_tag_roundtrip() {
        let mut vault_sender = Vault::new(VaultConfig::default()).expect("vault");
        let mut vault_recipient = Vault::new(VaultConfig::default()).expect("vault");
        let sender_secret = SecretKey::generate().expect("sender");
        let recipient_secret = SecretKey::generate().expect("recipient");
        vault_sender
            .import_plaintext("sender", sender_secret, 100)
            .expect("import");
        vault_recipient
            .import_plaintext("recipient", recipient_secret, 100)
            .expect("import");
        let recipient_pubkey = vault_recipient.public_key("recipient").expect("pubkey");
        let result = vault_sender
            .create_sealed_dm_with_scan_tag(
                "sender",
                "hello via scan tag",
                &recipient_pubkey,
                &recipient_pubkey,
                101,
            )
            .expect("create dm");
        assert_eq!(result.event.kind, 1059);
        let computed =
            Vault::compute_scan_tag(&recipient_secret, &result.event.pubkey).expect("compute tag");
        assert_eq!(result.scanning_tag, computed);
        let inner = vault_recipient
            .open_gift_wrap_dm("recipient", &result.event, 102)
            .expect("open dm");
        assert_eq!(inner.kind, 14);
        assert_eq!(inner.content, "hello via scan tag");
    }

    // --- dm-epoch tests ---

    #[test]
    #[cfg(feature = "dm-epoch")]
    fn epoch_key_derivation_is_deterministic() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let secret = SecretKey::generate().expect("secret");
        let peer = SecretKey::generate().expect("peer");
        let peer_pub = peer.xonly_public_key().expect("peer pubkey");
        vault.import_plaintext("a", secret, 100).expect("import");

        let k1 = vault.derive_epoch_key("a", &peer_pub, 3).expect("k1");
        let k2 = vault.derive_epoch_key("a", &peer_pub, 3).expect("k2");
        assert_eq!(k1, k2);
    }

    #[test]
    #[cfg(feature = "dm-epoch")]
    fn epoch_key_rotation_produces_distinct_keys() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let secret = SecretKey::generate().expect("secret");
        let peer = SecretKey::generate().expect("peer");
        let peer_pub = peer.xonly_public_key().expect("peer pubkey");
        vault.import_plaintext("a", secret, 100).expect("import");

        let k0 = vault.derive_epoch_key("a", &peer_pub, 0).expect("k0");
        let k1 = vault.derive_epoch_key("a", &peer_pub, 1).expect("k1");
        let k2 = vault.derive_epoch_key("a", &peer_pub, 2).expect("k2");
        assert_ne!(k0, k1);
        assert_ne!(k1, k2);
        assert_ne!(k0, k2);
    }

    #[test]
    #[cfg(feature = "dm-epoch")]
    fn encrypt_decrypt_epoch_roundtrip() {
        let key = [0xab; 32];
        let plaintext = b"hello epoch encryption";
        let ct = Vault::encrypt_with_epoch(&key, plaintext).expect("encrypt");
        let pt = Vault::decrypt_with_epoch(&key, &ct).expect("decrypt");
        assert_eq!(pt, plaintext);
    }

    #[test]
    #[cfg(feature = "dm-epoch")]
    fn epoch_decrypt_wrong_key_fails() {
        let key = [0xab; 32];
        let wrong = [0xcd; 32];
        let ct = Vault::encrypt_with_epoch(&key, b"secret").expect("encrypt");
        assert!(Vault::decrypt_with_epoch(&wrong, &ct).is_err());
    }

    #[test]
    #[cfg(feature = "dm-epoch")]
    fn create_group_key_uniqueness() {
        let k1 = Vault::create_group_key();
        let k2 = Vault::create_group_key();
        assert_ne!(k1, k2);
    }

    #[test]
    #[cfg(feature = "dm-epoch")]
    fn encrypt_decrypt_group_key_roundtrip() {
        let mut vault = Vault::new(VaultConfig::default()).expect("vault");
        let secret = SecretKey::generate().expect("secret");
        let pubkey = secret.xonly_public_key().expect("pubkey");
        vault.import_plaintext("r", secret, 100).expect("import");

        let group_key = Vault::create_group_key();
        let encrypted = Vault::encrypt_group_key(&group_key, &pubkey).expect("encrypt group key");
        let decrypted = vault
            .decrypt_group_key("r", &encrypted)
            .expect("decrypt group key");
        assert_eq!(group_key, decrypted);
    }

    #[test]
    #[cfg(feature = "dm-epoch")]
    fn group_message_roundtrip() {
        let key = Vault::create_group_key();
        let msg = b"group message content";
        let ct = Vault::encrypt_group_message(&key, msg).expect("encrypt");
        let pt = Vault::decrypt_group_message(&key, &ct).expect("decrypt");
        assert_eq!(pt, msg);
    }

    #[test]
    #[cfg(feature = "dm-epoch")]
    fn rotate_group_epoch_deterministic_and_distinct() {
        let k0 = [0x42; 32];
        let k1 = Vault::rotate_group_epoch(&k0);
        let k1_again = Vault::rotate_group_epoch(&k0);
        assert_eq!(k1, k1_again, "rotation must be deterministic");
        assert_ne!(k0, k1, "rotated key must differ from input");
    }
}
