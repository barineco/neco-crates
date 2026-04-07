use crate::keys::{decode_xonly_pubkey, ecdh_raw};
use crate::{SecpError, SecretKey, XOnlyPublicKey};
use aes::Aes256;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

pub fn encrypt(
    secret: &SecretKey,
    pubkey: &XOnlyPublicKey,
    plaintext: &str,
    iv: Option<[u8; 16]>,
) -> Result<String, SecpError> {
    let key = get_shared_secret_x(secret, pubkey)?;
    let iv = iv.unwrap_or_else(random_iv);
    let plaintext = plaintext.as_bytes();
    let mut buf = plaintext.to_vec();
    let msg_len = buf.len();
    buf.resize(msg_len + 16, 0);
    let ciphertext = Aes256CbcEnc::new((&key).into(), (&iv).into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, msg_len)
        .map_err(|_| SecpError::InvalidNip04("failed to encrypt"))?;

    Ok(format!(
        "{}?iv={}",
        neco_base64::encode(ciphertext),
        neco_base64::encode(&iv)
    ))
}

pub fn decrypt(
    secret: &SecretKey,
    pubkey: &XOnlyPublicKey,
    payload: &str,
) -> Result<String, SecpError> {
    let (ciphertext_b64, iv_b64) = payload
        .split_once("?iv=")
        .ok_or(SecpError::InvalidNip04("invalid payload"))?;
    let key = get_shared_secret_x(secret, pubkey)?;
    let iv = neco_base64::decode(iv_b64).map_err(|_| SecpError::InvalidNip04("invalid iv"))?;
    let ciphertext = neco_base64::decode(ciphertext_b64)
        .map_err(|_| SecpError::InvalidNip04("invalid ciphertext"))?;
    let iv: [u8; 16] = iv
        .try_into()
        .map_err(|_| SecpError::InvalidNip04("invalid iv"))?;

    let mut buf = ciphertext;
    let plaintext = Aes256CbcDec::new((&key).into(), (&iv).into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| SecpError::InvalidNip04("failed to decrypt"))?;
    String::from_utf8(plaintext.to_vec())
        .map_err(|_| SecpError::InvalidNip04("invalid utf-8 payload"))
}

fn get_shared_secret_x(secret: &SecretKey, pubkey: &XOnlyPublicKey) -> Result<[u8; 32], SecpError> {
    let peer = decode_xonly_pubkey(pubkey)?;
    ecdh_raw(&secret.bytes, peer).ok_or(SecpError::InvalidPublicKey)
}

fn random_iv() -> [u8; 16] {
    let mut iv = [0u8; 16];
    getrandom::getrandom(&mut iv).expect("getrandom");
    iv
}
