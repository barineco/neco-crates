use crate::{nostr, SecpError, SecretKey, SignedEvent, UnsignedEvent, XOnlyPublicKey};

pub fn create_auth_event(
    challenge: &str,
    relay_url: &str,
    signer: &SecretKey,
    now_unix_seconds: u64,
) -> Result<SignedEvent, SecpError> {
    let event = UnsignedEvent {
        created_at: now_unix_seconds,
        kind: 22_242,
        tags: vec![
            vec!["relay".to_string(), relay_url.to_string()],
            vec!["challenge".to_string(), challenge.to_string()],
        ],
        content: String::new(),
    };
    nostr::finalize_event(event, signer)
}

pub fn validate_auth_event(
    event: &SignedEvent,
    challenge: &str,
    relay_url: &str,
) -> Result<XOnlyPublicKey, SecpError> {
    nostr::verify_event(event)?;
    if event.kind != 22_242 {
        return Err(SecpError::InvalidEvent("auth event must have kind 22242"));
    }
    if tag_value(&event.tags, "relay") != Some(relay_url) {
        return Err(SecpError::InvalidEvent("auth event relay tag mismatch"));
    }
    if tag_value(&event.tags, "challenge") != Some(challenge) {
        return Err(SecpError::InvalidEvent("auth event challenge tag mismatch"));
    }
    Ok(event.pubkey)
}

fn tag_value<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|tag| tag.first().is_some_and(|value| value == name))
        .and_then(|tag| tag.get(1))
        .map(String::as_str)
}
