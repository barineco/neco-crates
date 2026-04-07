use crate::keys::decode_xonly_pubkey;
use crate::{SecpError, SecretKey, XOnlyPublicKey};
use aes::Aes256;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use k256::ecdh::diffie_hellman;
use k256::elliptic_curve::rand_core::RngCore;
use k256::elliptic_curve::rand_core::OsRng;

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
    let iv = neco_base64::decode(iv_b64)
        .map_err(|_| SecpError::InvalidNip04("invalid iv"))?;
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

fn get_shared_secret_x(
    secret: &SecretKey,
    pubkey: &XOnlyPublicKey,
) -> Result<[u8; 32], SecpError> {
    let signing_key = secret.signing_key()?;
    let secp_public = decode_xonly_pubkey(pubkey)?;
    let shared = diffie_hellman(signing_key.as_nonzero_scalar(), secp_public.as_affine());
    let mut out = [0u8; 32];
    out.copy_from_slice(shared.raw_secret_bytes().as_ref());
    Ok(out)
}

fn random_iv() -> [u8; 16] {
    let mut iv = [0u8; 16];
    OsRng.fill_bytes(&mut iv);
    iv
}
