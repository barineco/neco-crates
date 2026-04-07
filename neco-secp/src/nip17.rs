use crate::{nostr, nip44, SecpError, SecretKey, SignedEvent, UnsignedEvent, XOnlyPublicKey};
use k256::elliptic_curve::rand_core::RngCore;
use k256::elliptic_curve::rand_core::OsRng;

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

fn randomized_timestamp(base: u64) -> u64 {
    let offset = (OsRng.next_u32() % 345_601) as u64;
    base.saturating_sub(172_800) + offset
}
