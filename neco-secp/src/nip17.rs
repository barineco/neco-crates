use crate::{nip44, nostr, SecpError, SecretKey, SignedEvent, UnsignedEvent, XOnlyPublicKey};
use neco_sha2::Hmac;

pub fn create_seal(
    inner: UnsignedEvent,
    sender: &SecretKey,
    recipient: &XOnlyPublicKey,
) -> Result<SignedEvent, SecpError> {
    let signed_inner = nostr::finalize_event(inner, sender)?;
    let json = nostr::serialize_signed_event(&signed_inner)?;
    let conversation_key = nip44::get_conversation_key(sender, recipient)?;
    let encrypted = nip44::encrypt(&json, &conversation_key, None)?;
    let seal = UnsignedEvent {
        created_at: randomized_timestamp(signed_inner.created_at),
        kind: 13,
        tags: Vec::new(),
        content: encrypted,
    };
    nostr::finalize_event(seal, sender)
}

pub fn open_seal(seal: &SignedEvent, recipient: &SecretKey) -> Result<SignedEvent, SecpError> {
    if seal.kind != 13 {
        return Err(SecpError::InvalidEvent("seal must have kind 13"));
    }
    nostr::verify_event(seal)?;
    let conversation_key = nip44::get_conversation_key(recipient, &seal.pubkey)?;
    let json = nip44::decrypt(&seal.content, &conversation_key)?;
    let inner = nostr::parse_signed_event(&json)?;
    nostr::verify_event(&inner)?;
    Ok(inner)
}

/// Gift wrap with scanning tag for stealth DM discovery.
pub struct GiftWrapWithScanTag {
    pub event: SignedEvent,
    pub scanning_tag: [u8; 16],
}

pub fn create_gift_wrap(
    seal: &SignedEvent,
    recipient: &XOnlyPublicKey,
) -> Result<SignedEvent, SecpError> {
    if seal.kind != 13 {
        return Err(SecpError::InvalidEvent("seal must have kind 13"));
    }
    nostr::verify_event(seal)?;
    let ephemeral = SecretKey::generate()?;
    let json = nostr::serialize_signed_event(seal)?;
    let conversation_key = nip44::get_conversation_key(&ephemeral, recipient)?;
    let encrypted = nip44::encrypt(&json, &conversation_key, None)?;
    let wrap = UnsignedEvent {
        created_at: randomized_timestamp(seal.created_at),
        kind: 1059,
        tags: vec![vec!["p".to_string(), recipient.to_hex()]],
        content: encrypted,
    };
    nostr::finalize_event(wrap, &ephemeral)
}

/// Create a gift-wrapped DM with a scanning tag for the recipient.
/// The scanning tag allows the recipient to identify their messages without decryption.
pub fn create_gift_wrap_with_scan_tag(
    seal: &SignedEvent,
    recipient: &XOnlyPublicKey,
    recipient_scan_pub: &XOnlyPublicKey,
) -> Result<GiftWrapWithScanTag, SecpError> {
    if seal.kind != 13 {
        return Err(SecpError::InvalidEvent("seal must have kind 13"));
    }
    nostr::verify_event(seal)?;
    let ephemeral = SecretKey::generate()?;
    let json = nostr::serialize_signed_event(seal)?;
    let conversation_key = nip44::get_conversation_key(&ephemeral, recipient)?;
    let encrypted = nip44::encrypt(&json, &conversation_key, None)?;
    let scanning_tag = compute_scan_tag_from_secret(&ephemeral, recipient_scan_pub)?;
    let wrap = UnsignedEvent {
        created_at: randomized_timestamp(seal.created_at),
        kind: 1059,
        tags: vec![vec!["p".to_string(), recipient.to_hex()]],
        content: encrypted,
    };
    let event = nostr::finalize_event(wrap, &ephemeral)?;
    Ok(GiftWrapWithScanTag { event, scanning_tag })
}

pub fn open_gift_wrap(
    gift_wrap: &SignedEvent,
    recipient: &SecretKey,
) -> Result<SignedEvent, SecpError> {
    if gift_wrap.kind != 1059 {
        return Err(SecpError::InvalidEvent("gift wrap must have kind 1059"));
    }
    nostr::verify_event(gift_wrap)?;
    let conversation_key = nip44::get_conversation_key(recipient, &gift_wrap.pubkey)?;
    let json = nip44::decrypt(&gift_wrap.content, &conversation_key)?;
    let seal = nostr::parse_signed_event(&json)?;
    open_seal(&seal, recipient)
}

/// Compute a scanning tag for stealth DM recipient discovery.
/// tag = HMAC-SHA256(conversation_key(scan_priv, ephemeral_pub), "stealth-dm-tag")[0..16]
pub fn compute_scan_tag(
    scan_priv: &SecretKey,
    ephemeral_pubkey: &XOnlyPublicKey,
) -> Result<[u8; 16], SecpError> {
    let conversation_key = nip44::get_conversation_key(scan_priv, ephemeral_pubkey)?;
    let mut hmac = Hmac::new(&conversation_key);
    hmac.update(b"stealth-dm-tag");
    let full = hmac.finalize();
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&full[..16]);
    Ok(tag)
}

/// Internal: compute scan tag from ephemeral secret key (sender side).
fn compute_scan_tag_from_secret(
    ephemeral_secret: &SecretKey,
    scan_pub: &XOnlyPublicKey,
) -> Result<[u8; 16], SecpError> {
    let conversation_key = nip44::get_conversation_key(ephemeral_secret, scan_pub)?;
    let mut hmac = Hmac::new(&conversation_key);
    hmac.update(b"stealth-dm-tag");
    let full = hmac.finalize();
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&full[..16]);
    Ok(tag)
}

fn randomized_timestamp(base: u64) -> u64 {
    let mut buf = [0u8; 4];
    getrandom::getrandom(&mut buf).expect("getrandom");
    let offset = (u32::from_le_bytes(buf) % 345_601) as u64;
    base.saturating_sub(172_800) + offset
}
