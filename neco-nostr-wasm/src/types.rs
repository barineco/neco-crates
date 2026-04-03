use neco_secp::{
    nostr, EventId, NAddr, NProfile, Nip19, PublicKey, SecretKey, SignedEvent, UnsignedEvent,
    XOnlyPublicKey,
};
use neco_vault::SecurityConfig;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use wasm_bindgen::JsValue;

pub fn js_error<E: core::fmt::Display>(error: E) -> JsValue {
    JsValue::from_str(&error.to_string())
}

pub fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn parse_secret_key(secret_hex: &str) -> Result<SecretKey, JsValue> {
    SecretKey::from_hex(secret_hex).map_err(js_error)
}

pub fn parse_public_key(public_hex: &str) -> Result<PublicKey, JsValue> {
    PublicKey::from_hex(public_hex).map_err(js_error)
}

pub fn parse_xonly_public_key(public_hex: &str) -> Result<XOnlyPublicKey, JsValue> {
    XOnlyPublicKey::from_hex(public_hex).map_err(js_error)
}

pub fn parse_event_id(id_hex: &str) -> Result<EventId, JsValue> {
    EventId::from_hex(id_hex).map_err(js_error)
}

pub fn parse_unsigned_event(json_str: &str) -> Result<UnsignedEvent, JsValue> {
    let value: Value = serde_json::from_str(json_str).map_err(js_error)?;
    let object = value
        .as_object()
        .ok_or_else(|| JsValue::from_str("event must be a JSON object"))?;
    let tags = object
        .get("tags")
        .cloned()
        .ok_or_else(|| JsValue::from_str("missing tags"))?;
    let tags: Vec<Vec<String>> = serde_json::from_value(tags).map_err(js_error)?;
    let created_at = object
        .get("created_at")
        .and_then(Value::as_u64)
        .ok_or_else(|| JsValue::from_str("missing created_at"))?;
    let kind = object
        .get("kind")
        .and_then(Value::as_u64)
        .ok_or_else(|| JsValue::from_str("missing kind"))?
        .try_into()
        .map_err(|_| JsValue::from_str("kind out of range"))?;
    let content = object
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| JsValue::from_str("missing content"))?;
    Ok(UnsignedEvent {
        created_at,
        kind,
        tags,
        content: content.to_string(),
    })
}

pub fn parse_signed_event(json_str: &str) -> Result<SignedEvent, JsValue> {
    nostr::parse_signed_event(json_str).map_err(js_error)
}

pub fn stringify_signed_event(event: &SignedEvent) -> Result<String, JsValue> {
    nostr::serialize_signed_event(event).map_err(js_error)
}

pub fn parse_security_config(json_str: &str) -> Result<SecurityConfig, JsValue> {
    let value: Value = serde_json::from_str(json_str).map_err(js_error)?;
    let object = value
        .as_object()
        .ok_or_else(|| JsValue::from_str("security config must be a JSON object"))?;
    Ok(SecurityConfig {
        enable_constant_time: object
            .get("enable_constant_time")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        enable_random_delay: object
            .get("enable_random_delay")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        enable_dummy_operations: object
            .get("enable_dummy_operations")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

pub fn parse_event_with_pubkey(json_str: &str) -> Result<(XOnlyPublicKey, UnsignedEvent), JsValue> {
    let value: Value = serde_json::from_str(json_str).map_err(js_error)?;
    let object = value
        .as_object()
        .ok_or_else(|| JsValue::from_str("event must be a JSON object"))?;

    let pubkey = object
        .get("pubkey")
        .and_then(Value::as_str)
        .ok_or_else(|| JsValue::from_str("missing pubkey"))?;
    let tags = object
        .get("tags")
        .cloned()
        .ok_or_else(|| JsValue::from_str("missing tags"))?;
    let tags: Vec<Vec<String>> = serde_json::from_value(tags).map_err(js_error)?;
    let created_at = object
        .get("created_at")
        .and_then(Value::as_u64)
        .ok_or_else(|| JsValue::from_str("missing created_at"))?;
    let kind = object
        .get("kind")
        .and_then(Value::as_u64)
        .ok_or_else(|| JsValue::from_str("missing kind"))?
        .try_into()
        .map_err(|_| JsValue::from_str("kind out of range"))?;
    let content = object
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| JsValue::from_str("missing content"))?;

    Ok((
        parse_xonly_public_key(pubkey)?,
        UnsignedEvent {
            created_at,
            kind,
            tags,
            content: content.to_string(),
        },
    ))
}

pub fn nip19_to_json(value: Nip19, allow_secret: bool) -> Result<String, JsValue> {
    let output = match value {
        Nip19::Npub(pubkey) => json!({
            "kind": "npub",
            "pubkey_hex": pubkey.to_hex(),
        }),
        Nip19::Nsec(secret) => {
            if !allow_secret {
                return Err(JsValue::from_str(
                    "nsec decode is disabled in wasm boundary",
                ));
            }
            json!({
                "kind": "nsec",
                "secret_hex": secret.to_hex(),
            })
        }
        Nip19::Note(id) => json!({
            "kind": "note",
            "id_hex": id.to_hex(),
        }),
        Nip19::NProfile(profile) => json!({
            "kind": "nprofile",
            "pubkey_hex": profile.pubkey.to_hex(),
            "relays": profile.relays,
        }),
        Nip19::NEvent(event) => json!({
            "kind": "nevent",
            "id_hex": event.id.to_hex(),
            "relays": event.relays,
            "author_hex": event.author.map(|author| author.to_hex()),
            "event_kind": event.kind,
        }),
        Nip19::NAddr(addr) => json!({
            "kind": "naddr",
            "identifier": addr.identifier,
            "relays": addr.relays,
            "author_hex": addr.author.to_hex(),
            "event_kind": addr.kind,
        }),
        Nip19::NRelay(relay) => json!({
            "kind": "nrelay",
            "relay": relay.relay,
        }),
    };
    serde_json::to_string(&output).map_err(js_error)
}

pub fn parse_nevent(json_str: &str) -> Result<neco_secp::NEvent, JsValue> {
    let value: Value = serde_json::from_str(json_str).map_err(js_error)?;
    let object = value
        .as_object()
        .ok_or_else(|| JsValue::from_str("nevent must be a JSON object"))?;
    let id_hex = object
        .get("id_hex")
        .or_else(|| object.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| JsValue::from_str("missing id_hex"))?;
    let relays = object
        .get("relays")
        .cloned()
        .map(|relays| serde_json::from_value(relays).map_err(js_error))
        .transpose()?
        .unwrap_or_default();
    let author = object
        .get("author_hex")
        .or_else(|| object.get("author"))
        .and_then(Value::as_str)
        .map(parse_xonly_public_key)
        .transpose()?;
    let kind = object
        .get("kind")
        .or_else(|| object.get("event_kind"))
        .and_then(Value::as_u64)
        .map(|kind| u32::try_from(kind).map_err(|_| JsValue::from_str("kind out of range")))
        .transpose()?;

    Ok(neco_secp::NEvent {
        id: parse_event_id(id_hex)?,
        relays,
        author,
        kind,
    })
}

pub fn parse_nprofile(json_str: &str) -> Result<NProfile, JsValue> {
    let value: Value = serde_json::from_str(json_str).map_err(js_error)?;
    let object = value
        .as_object()
        .ok_or_else(|| JsValue::from_str("nprofile must be a JSON object"))?;
    let pubkey_hex = object
        .get("pubkey_hex")
        .or_else(|| object.get("pubkey"))
        .and_then(Value::as_str)
        .ok_or_else(|| JsValue::from_str("missing pubkey_hex"))?;
    let relays = object
        .get("relays")
        .cloned()
        .map(|relays| serde_json::from_value(relays).map_err(js_error))
        .transpose()?
        .unwrap_or_default();

    Ok(NProfile {
        pubkey: parse_xonly_public_key(pubkey_hex)?,
        relays,
    })
}

pub fn parse_naddr(json_str: &str) -> Result<NAddr, JsValue> {
    let value: Value = serde_json::from_str(json_str).map_err(js_error)?;
    let object = value
        .as_object()
        .ok_or_else(|| JsValue::from_str("naddr must be a JSON object"))?;
    let identifier = object
        .get("identifier")
        .and_then(Value::as_str)
        .ok_or_else(|| JsValue::from_str("missing identifier"))?;
    let relays = object
        .get("relays")
        .cloned()
        .map(|relays| serde_json::from_value(relays).map_err(js_error))
        .transpose()?
        .unwrap_or_default();
    let author_hex = object
        .get("author_hex")
        .or_else(|| object.get("author"))
        .or_else(|| object.get("pubkey_hex"))
        .or_else(|| object.get("pubkey"))
        .and_then(Value::as_str)
        .ok_or_else(|| JsValue::from_str("missing author_hex"))?;
    let kind = object
        .get("kind")
        .or_else(|| object.get("event_kind"))
        .and_then(Value::as_u64)
        .ok_or_else(|| JsValue::from_str("missing event_kind"))?
        .try_into()
        .map_err(|_| JsValue::from_str("kind out of range"))?;

    Ok(NAddr {
        identifier: identifier.to_string(),
        relays,
        author: parse_xonly_public_key(author_hex)?,
        kind,
    })
}

pub fn public_key_json(secret: &SecretKey) -> Result<Value, JsValue> {
    let pubkey = secret.xonly_public_key().map_err(js_error)?;
    let public = secret.public_key().map_err(js_error)?;
    let npub = neco_secp::nip19::encode_npub(&pubkey).map_err(js_error)?;
    Ok(json!({
        "pubkey_hex": pubkey.to_hex(),
        "public_key_hex": public.to_hex(),
        "npub": npub,
    }))
}
