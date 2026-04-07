use crate::hex::hex_encode;
use crate::{EventId, SchnorrSignature, SecpError, SecretKey, SignedEvent, UnsignedEvent, XOnlyPublicKey};
use neco_json::JsonValue;
use neco_sha2::Sha256;

pub fn serialize_event(
    pubkey: &XOnlyPublicKey,
    event: &UnsignedEvent,
) -> Result<String, SecpError> {
    let tags = JsonValue::Array(
        event
            .tags
            .iter()
            .map(|tag| {
                JsonValue::Array(
                    tag.iter()
                        .map(|s| JsonValue::String(s.clone()))
                        .collect(),
                )
            })
            .collect(),
    );
    let payload = JsonValue::Array(vec![
        JsonValue::Number(0.0),
        JsonValue::String(hex_encode(&pubkey.to_bytes())),
        JsonValue::Number(event.created_at as f64),
        JsonValue::Number(event.kind as f64),
        tags,
        JsonValue::String(event.content.clone()),
    ]);
    let bytes = neco_json::encode(&payload).map_err(SecpError::from)?;
    String::from_utf8(bytes).map_err(|e| SecpError::Json(e.to_string()))
}

pub fn compute_event_id(
    pubkey: &XOnlyPublicKey,
    event: &UnsignedEvent,
) -> Result<EventId, SecpError> {
    let serialized = serialize_event(pubkey, event)?;
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&hasher.finalize());
    Ok(EventId::from_bytes(bytes))
}

pub fn finalize_event(
    event: UnsignedEvent,
    secret: &SecretKey,
) -> Result<SignedEvent, SecpError> {
    let pubkey = secret.xonly_public_key()?;
    let id = compute_event_id(&pubkey, &event)?;
    let sig = secret.sign_schnorr_prehash(id.to_bytes())?;
    Ok(SignedEvent {
        id,
        pubkey,
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags,
        content: event.content,
        sig,
    })
}

pub fn serialize_signed_event(event: &SignedEvent) -> Result<String, SecpError> {
    let tags = JsonValue::Array(
        event
            .tags
            .iter()
            .map(|tag| {
                JsonValue::Array(
                    tag.iter()
                        .map(|s| JsonValue::String(s.clone()))
                        .collect(),
                )
            })
            .collect(),
    );
    let content_encoded = neco_json::encode(&JsonValue::String(event.content.clone()))
        .map_err(SecpError::from)?;
    let content_str =
        String::from_utf8(content_encoded).map_err(|e| SecpError::Json(e.to_string()))?;
    let tags_encoded = neco_json::encode(&tags).map_err(SecpError::from)?;
    let tags_str =
        String::from_utf8(tags_encoded).map_err(|e| SecpError::Json(e.to_string()))?;
    Ok(format!(
        "{{\"id\":\"{}\",\"pubkey\":\"{}\",\"created_at\":{},\"kind\":{},\"tags\":{},\"content\":{},\"sig\":\"{}\"}}",
        hex_encode(&event.id.to_bytes()),
        hex_encode(&event.pubkey.to_bytes()),
        event.created_at,
        event.kind,
        tags_str,
        content_str,
        hex_encode(&event.sig.to_bytes())
    ))
}

pub fn parse_signed_event(json: &str) -> Result<SignedEvent, SecpError> {
    let value = neco_json::parse(json.as_bytes()).map_err(SecpError::from)?;
    if !value.is_object() {
        return Err(SecpError::InvalidEvent("signed event must be a JSON object"));
    }

    let id = parse_hex32(required_string(&value, "id")?, "id")?;
    let pubkey = parse_hex32(required_string(&value, "pubkey")?, "pubkey")?;
    let created_at = required_u64(&value, "created_at")?;
    let kind = required_u32(&value, "kind")?;
    let tags = parse_tags(required_value(&value, "tags")?)?;
    let content = required_string(&value, "content")?.to_string();
    let sig = parse_hex64(required_string(&value, "sig")?, "sig")?;

    Ok(SignedEvent {
        id: EventId::from_bytes(id),
        pubkey: XOnlyPublicKey::from_bytes(pubkey)?,
        created_at,
        kind,
        tags,
        content,
        sig: SchnorrSignature { bytes: sig },
    })
}

pub fn verify_event(event: &SignedEvent) -> Result<(), SecpError> {
    let unsigned = UnsignedEvent {
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags.clone(),
        content: event.content.clone(),
    };
    let expected = compute_event_id(&event.pubkey, &unsigned)?;
    if expected != event.id {
        return Err(SecpError::InvalidEvent("event id mismatch"));
    }
    event
        .pubkey
        .verify_schnorr_prehash(event.id.to_bytes(), &event.sig)
}

fn required_value<'a>(
    object: &'a JsonValue,
    field: &'static str,
) -> Result<&'a JsonValue, SecpError> {
    object
        .get(field)
        .ok_or(SecpError::InvalidEvent(missing_field(field)))
}

fn required_string<'a>(
    object: &'a JsonValue,
    field: &'static str,
) -> Result<&'a str, SecpError> {
    required_value(object, field)?
        .as_str()
        .ok_or(SecpError::InvalidEvent(expected_field(field)))
}

fn required_u64(object: &JsonValue, field: &'static str) -> Result<u64, SecpError> {
    required_value(object, field)?
        .as_u64()
        .ok_or(SecpError::InvalidEvent(expected_field(field)))
}

fn required_u32(object: &JsonValue, field: &'static str) -> Result<u32, SecpError> {
    required_u64(object, field)?
        .try_into()
        .map_err(|_| SecpError::InvalidEvent(expected_field(field)))
}

fn parse_tags(value: &JsonValue) -> Result<Vec<Vec<String>>, SecpError> {
    let tags = value
        .as_array()
        .ok_or(SecpError::InvalidEvent("tags must be an array"))?;
    tags.iter()
        .map(|tag| {
            let tag = tag
                .as_array()
                .ok_or(SecpError::InvalidEvent("tag must be an array"))?;
            tag.iter()
                .map(|entry| {
                    entry
                        .as_str()
                        .map(ToOwned::to_owned)
                        .ok_or(SecpError::InvalidEvent("tag entry must be a string"))
                })
                .collect()
        })
        .collect()
}

fn parse_hex32(hex: &str, field: &'static str) -> Result<[u8; 32], SecpError> {
    let bytes = crate::hex::hex_decode(hex)
        .map_err(|_| SecpError::InvalidEvent(invalid_hex_field(field)))?;
    bytes
        .try_into()
        .map_err(|_| SecpError::InvalidEvent(invalid_hex_field(field)))
}

fn parse_hex64(hex: &str, field: &'static str) -> Result<[u8; 64], SecpError> {
    let bytes = crate::hex::hex_decode(hex)
        .map_err(|_| SecpError::InvalidEvent(invalid_hex_field(field)))?;
    bytes
        .try_into()
        .map_err(|_| SecpError::InvalidEvent(invalid_hex_field(field)))
}

fn missing_field(field: &'static str) -> &'static str {
    match field {
        "id" => "missing id",
        "pubkey" => "missing pubkey",
        "created_at" => "missing created_at",
        "kind" => "missing kind",
        "tags" => "missing tags",
        "content" => "missing content",
        "sig" => "missing sig",
        _ => "missing field",
    }
}

fn expected_field(field: &'static str) -> &'static str {
    match field {
        "id" => "id must be a string",
        "pubkey" => "pubkey must be a string",
        "created_at" => "created_at must be an integer",
        "kind" => "kind must be an integer",
        "content" => "content must be a string",
        "sig" => "sig must be a string",
        _ => "invalid field type",
    }
}

fn invalid_hex_field(field: &'static str) -> &'static str {
    match field {
        "id" => "id must be 64 hex characters",
        "pubkey" => "pubkey must be 64 hex characters",
        "sig" => "sig must be 128 hex characters",
        _ => "invalid hex field",
    }
}
