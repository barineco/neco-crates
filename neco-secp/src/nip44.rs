use crate::keys::decode_xonly_pubkey;
use crate::{SecpError, SecretKey, XOnlyPublicKey};
use chacha20::cipher::{KeyIvInit, StreamCipher};
use k256::ecdh::diffie_hellman;
use k256::elliptic_curve::rand_core::RngCore;
use k256::elliptic_curve::rand_core::OsRng;
use neco_sha2::{Hkdf, Hmac};

const VERSION_V2: u8 = 2;
const MIN_PLAINTEXT_SIZE: usize = 1;
const MAX_PLAINTEXT_SIZE: usize = 65_535;
const MIN_RAW_PAYLOAD_SIZE: usize = 99;
const MAX_RAW_PAYLOAD_SIZE: usize = 65_603;
const MIN_ENCODED_PAYLOAD_SIZE: usize = 132;
const MAX_ENCODED_PAYLOAD_SIZE: usize = 87_472;

pub fn get_conversation_key(
    secret: &SecretKey,
    pubkey: &XOnlyPublicKey,
) -> Result<[u8; 32], SecpError> {
    let signing_key = secret.signing_key()?;
    let secp_public = decode_xonly_pubkey(pubkey)?;
    let shared = diffie_hellman(signing_key.as_nonzero_scalar(), secp_public.as_affine());
    let prk = Hkdf::extract(b"nip44-v2", shared.raw_secret_bytes().as_ref());
    Ok(*prk.as_bytes())
}

pub fn calc_padded_len(len: usize) -> Result<usize, SecpError> {
    if len < MIN_PLAINTEXT_SIZE {
        return Err(SecpError::InvalidNip44("expected positive integer"));
    }
    if len <= 32 {
        return Ok(32);
    }
    let next_power = 1usize << (usize::BITS as usize - (len - 1).leading_zeros() as usize);
    let chunk = if next_power <= 256 {
        32
    } else {
        next_power / 8
    };
    Ok(chunk * ((len - 1) / chunk + 1))
}

pub fn encrypt(
    plaintext: &str,
    conversation_key: &[u8; 32],
    nonce: Option<[u8; 32]>,
) -> Result<String, SecpError> {
    let nonce = nonce.unwrap_or_else(random_nonce);
    let padded = pad(plaintext)?;
    let keys = get_message_keys(conversation_key, &nonce)?;
    let ciphertext = chacha20_xor(&keys.chacha_key, &keys.chacha_nonce, &padded);
    let mac = hmac_aad(&keys.hmac_key, &ciphertext, &nonce)?;

    let mut payload = Vec::with_capacity(1 + nonce.len() + ciphertext.len() + mac.len());
    payload.push(VERSION_V2);
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&ciphertext);
    payload.extend_from_slice(&mac);
    Ok(neco_base64::encode(&payload))
}

pub fn decrypt(payload: &str, conversation_key: &[u8; 32]) -> Result<String, SecpError> {
    let decoded = decode_payload(payload)?;
    let keys = get_message_keys(conversation_key, &decoded.nonce)?;
    let calculated_mac = hmac_aad(&keys.hmac_key, &decoded.ciphertext, &decoded.nonce)?;
    if calculated_mac != decoded.mac {
        return Err(SecpError::InvalidNip44("invalid MAC"));
    }

    let padded = chacha20_xor(&keys.chacha_key, &keys.chacha_nonce, &decoded.ciphertext);
    unpad(&padded)
}

struct MessageKeys {
    chacha_key: [u8; 32],
    chacha_nonce: [u8; 12],
    hmac_key: [u8; 32],
}

struct DecodedPayload {
    nonce: [u8; 32],
    ciphertext: Vec<u8>,
    mac: [u8; 32],
}

fn get_message_keys(
    conversation_key: &[u8; 32],
    nonce: &[u8; 32],
) -> Result<MessageKeys, SecpError> {
    use neco_sha2::Prk;
    let prk = Prk::from_bytes(conversation_key);
    let keys = prk
        .expand(nonce, 76)
        .map_err(|_| SecpError::InvalidNip44("failed to derive message keys"))?;

    let mut chacha_key = [0u8; 32];
    chacha_key.copy_from_slice(&keys[..32]);

    let mut chacha_nonce = [0u8; 12];
    chacha_nonce.copy_from_slice(&keys[32..44]);

    let mut hmac_key = [0u8; 32];
    hmac_key.copy_from_slice(&keys[44..76]);

    Ok(MessageKeys {
        chacha_key,
        chacha_nonce,
        hmac_key,
    })
}

fn random_nonce() -> [u8; 32] {
    let mut nonce = [0u8; 32];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

fn pad(plaintext: &str) -> Result<Vec<u8>, SecpError> {
    let unpadded = plaintext.as_bytes();
    let len = unpadded.len();
    if !(MIN_PLAINTEXT_SIZE..=MAX_PLAINTEXT_SIZE).contains(&len) {
        return Err(SecpError::InvalidNip44(
            "invalid plaintext size: must be between 1 and 65535 bytes",
        ));
    }

    let padded_len = calc_padded_len(len)?;
    let mut out = Vec::with_capacity(2 + padded_len);
    let len_u16 = u16::try_from(len).expect("validated plaintext size <= u16::MAX");
    out.extend_from_slice(&len_u16.to_be_bytes());
    out.extend_from_slice(unpadded);
    out.resize(2 + padded_len, 0);
    Ok(out)
}

fn unpad(padded: &[u8]) -> Result<String, SecpError> {
    if padded.len() < 2 {
        return Err(SecpError::InvalidNip44("invalid padding"));
    }
    let len = u16::from_be_bytes([padded[0], padded[1]]) as usize;
    if !(MIN_PLAINTEXT_SIZE..=MAX_PLAINTEXT_SIZE).contains(&len) {
        return Err(SecpError::InvalidNip44("invalid padding"));
    }
    let expected = 2 + calc_padded_len(len)?;
    if padded.len() != expected || 2 + len > padded.len() {
        return Err(SecpError::InvalidNip44("invalid padding"));
    }
    let unpadded = &padded[2..2 + len];
    String::from_utf8(unpadded.to_vec())
        .map_err(|_| SecpError::InvalidNip44("invalid utf-8 payload"))
}

fn hmac_aad(key: &[u8; 32], message: &[u8], aad: &[u8; 32]) -> Result<[u8; 32], SecpError> {
    let mut mac = Hmac::new(key);
    mac.update(aad);
    mac.update(message);
    Ok(mac.finalize())
}

fn chacha20_xor(key: &[u8; 32], nonce: &[u8; 12], data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    let mut cipher = chacha20::ChaCha20::new(key.into(), nonce.into());
    cipher.apply_keystream(&mut out);
    out
}

fn decode_payload(payload: &str) -> Result<DecodedPayload, SecpError> {
    let len = payload.len();
    if !(MIN_ENCODED_PAYLOAD_SIZE..=MAX_ENCODED_PAYLOAD_SIZE).contains(&len) {
        return Err(SecpError::InvalidNip44("invalid payload length"));
    }
    if payload.starts_with('#') {
        return Err(SecpError::InvalidNip44("unknown encryption version"));
    }

    let data = neco_base64::decode(payload)
        .map_err(|_| SecpError::InvalidNip44("invalid base64"))?;
    if !(MIN_RAW_PAYLOAD_SIZE..=MAX_RAW_PAYLOAD_SIZE).contains(&data.len()) {
        return Err(SecpError::InvalidNip44("invalid data length"));
    }
    if data[0] != VERSION_V2 {
        return Err(SecpError::InvalidNip44("unknown encryption version"));
    }
    if data.len() < 65 {
        return Err(SecpError::InvalidNip44("invalid data length"));
    }

    let mut nonce = [0u8; 32];
    nonce.copy_from_slice(&data[1..33]);
    let mut mac = [0u8; 32];
    mac.copy_from_slice(&data[data.len() - 32..]);
    let ciphertext = data[33..data.len() - 32].to_vec();

    Ok(DecodedPayload {
        nonce,
        ciphertext,
        mac,
    })
}
