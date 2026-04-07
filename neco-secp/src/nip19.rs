use crate::{
    EventId, NAddr, NEvent, NProfile, NRelay, Nip19, SecpError, SecretKey, XOnlyPublicKey,
};
use bech32::{self, FromBase32, ToBase32, Variant};

pub fn encode_npub(pubkey: &XOnlyPublicKey) -> Result<String, SecpError> {
    bech32::encode("npub", pubkey.to_bytes().to_base32(), Variant::Bech32)
        .map_err(|_| SecpError::InvalidNip19("failed to encode npub"))
}

pub fn encode_nsec(secret: &SecretKey) -> Result<String, SecpError> {
    bech32::encode("nsec", secret.to_bytes().to_base32(), Variant::Bech32)
        .map_err(|_| SecpError::InvalidNip19("failed to encode nsec"))
}

pub fn encode_note(id: &EventId) -> Result<String, SecpError> {
    bech32::encode("note", id.to_bytes().to_base32(), Variant::Bech32)
        .map_err(|_| SecpError::InvalidNip19("failed to encode note"))
}

pub fn encode_nprofile(profile: &NProfile) -> Result<String, SecpError> {
    encode_tlv_entity(
        "nprofile",
        &[
            (0, vec![profile.pubkey.to_bytes().to_vec()]),
            (
                1,
                profile
                    .relays
                    .iter()
                    .map(|relay| relay.as_bytes().to_vec())
                    .collect(),
            ),
        ],
    )
}

pub fn encode_nevent(event: &NEvent) -> Result<String, SecpError> {
    let mut fields = vec![
        (0, vec![event.id.to_bytes().to_vec()]),
        (
            1,
            event
                .relays
                .iter()
                .map(|relay| relay.as_bytes().to_vec())
                .collect(),
        ),
    ];

    if let Some(author) = event.author {
        fields.push((2, vec![author.to_bytes().to_vec()]));
    }
    if let Some(kind) = event.kind {
        fields.push((3, vec![kind.to_be_bytes().to_vec()]));
    }

    encode_tlv_entity("nevent", &fields)
}

pub fn encode_naddr(addr: &NAddr) -> Result<String, SecpError> {
    encode_tlv_entity(
        "naddr",
        &[
            (0, vec![addr.identifier.as_bytes().to_vec()]),
            (
                1,
                addr.relays
                    .iter()
                    .map(|relay| relay.as_bytes().to_vec())
                    .collect(),
            ),
            (2, vec![addr.author.to_bytes().to_vec()]),
            (3, vec![addr.kind.to_be_bytes().to_vec()]),
        ],
    )
}

pub fn encode_nrelay(relay: &NRelay) -> Result<String, SecpError> {
    encode_tlv_entity("nrelay", &[(0, vec![relay.relay.as_bytes().to_vec()])])
}

pub fn decode(s: &str) -> Result<Nip19, SecpError> {
    let (hrp, data, variant) =
        bech32::decode(s).map_err(|_| SecpError::InvalidNip19("invalid bech32 string"))?;
    if variant != Variant::Bech32 {
        return Err(SecpError::InvalidNip19("unexpected bech32 variant"));
    }

    let bytes = Vec::<u8>::from_base32(&data)
        .map_err(|_| SecpError::InvalidNip19("invalid bech32 payload"))?;

    match hrp.as_str() {
        "npub" => {
            let payload: [u8; 32] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| SecpError::InvalidNip19("expected 32-byte payload"))?;
            Ok(Nip19::Npub(XOnlyPublicKey::from_bytes(payload)?))
        }
        "nsec" => {
            let payload: [u8; 32] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| SecpError::InvalidNip19("expected 32-byte payload"))?;
            Ok(Nip19::Nsec(SecretKey::from_bytes(payload)?))
        }
        "note" => {
            let payload: [u8; 32] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| SecpError::InvalidNip19("expected 32-byte payload"))?;
            Ok(Nip19::Note(EventId::from_bytes(payload)))
        }
        "nprofile" => decode_nprofile(&bytes),
        "nevent" => decode_nevent(&bytes),
        "naddr" => decode_naddr(&bytes),
        "nrelay" => decode_nrelay(&bytes),
        _ => Err(SecpError::InvalidNip19("unsupported nip19 prefix")),
    }
}

fn decode_nprofile(bytes: &[u8]) -> Result<Nip19, SecpError> {
    let tlv = parse_tlv(bytes)?;
    let pubkey = required_bytes32(&tlv, 0, "nprofile")?;
    let relays = utf8_entries(&tlv, 1, "nprofile")?;
    Ok(Nip19::NProfile(NProfile {
        pubkey: XOnlyPublicKey::from_bytes(pubkey)?,
        relays,
    }))
}

fn decode_nevent(bytes: &[u8]) -> Result<Nip19, SecpError> {
    let tlv = parse_tlv(bytes)?;
    let id = required_bytes32(&tlv, 0, "nevent")?;
    let relays = utf8_entries(&tlv, 1, "nevent")?;
    let author = optional_bytes32(&tlv, 2, "nevent")?
        .map(XOnlyPublicKey::from_bytes)
        .transpose()?;
    let kind = optional_u32(&tlv, 3, "nevent")?;

    Ok(Nip19::NEvent(NEvent {
        id: EventId::from_bytes(id),
        relays,
        author,
        kind,
    }))
}

fn decode_naddr(bytes: &[u8]) -> Result<Nip19, SecpError> {
    let tlv = parse_tlv(bytes)?;
    let identifier = required_utf8(&tlv, 0, "naddr")?;
    let relays = utf8_entries(&tlv, 1, "naddr")?;
    let author = required_bytes32(&tlv, 2, "naddr")?;
    let kind = required_u32(&tlv, 3, "naddr")?;

    Ok(Nip19::NAddr(NAddr {
        identifier,
        relays,
        author: XOnlyPublicKey::from_bytes(author)?,
        kind,
    }))
}

fn decode_nrelay(bytes: &[u8]) -> Result<Nip19, SecpError> {
    let tlv = parse_tlv(bytes)?;
    let relay = required_utf8(&tlv, 0, "nrelay")?;
    Ok(Nip19::NRelay(NRelay { relay }))
}

fn encode_tlv_entity(prefix: &str, fields: &[(u8, Vec<Vec<u8>>)]) -> Result<String, SecpError> {
    let tlv = encode_tlv(fields)?;
    bech32::encode(prefix, tlv.to_base32(), Variant::Bech32)
        .map_err(|_| SecpError::InvalidNip19("failed to encode tlv entity"))
}

fn encode_tlv(fields: &[(u8, Vec<Vec<u8>>)]) -> Result<Vec<u8>, SecpError> {
    let mut out = Vec::new();
    for (tag, values) in fields.iter().rev() {
        for value in values {
            let len: u8 = value
                .len()
                .try_into()
                .map_err(|_| SecpError::InvalidNip19("tlv value too long"))?;
            out.push(*tag);
            out.push(len);
            out.extend_from_slice(value);
        }
    }
    Ok(out)
}

fn parse_tlv(bytes: &[u8]) -> Result<Vec<Vec<Vec<u8>>>, SecpError> {
    let mut tlv = vec![Vec::new(); 256];
    let mut offset = 0usize;
    while offset < bytes.len() {
        if offset + 2 > bytes.len() {
            return Err(SecpError::InvalidNip19("truncated tlv header"));
        }
        let tag = bytes[offset] as usize;
        let len = bytes[offset + 1] as usize;
        offset += 2;
        if offset + len > bytes.len() {
            return Err(SecpError::InvalidNip19("not enough data for tlv entry"));
        }
        tlv[tag].push(bytes[offset..offset + len].to_vec());
        offset += len;
    }
    Ok(tlv)
}

fn required_bytes32(
    tlv: &[Vec<Vec<u8>>],
    tag: usize,
    entity: &'static str,
) -> Result<[u8; 32], SecpError> {
    let value = tlv[tag]
        .first()
        .ok_or(SecpError::InvalidNip19(missing_required_field(entity, tag)))?;
    value
        .as_slice()
        .try_into()
        .map_err(|_| SecpError::InvalidNip19(expected_length(entity, tag, 32)))
}

fn optional_bytes32(
    tlv: &[Vec<Vec<u8>>],
    tag: usize,
    entity: &'static str,
) -> Result<Option<[u8; 32]>, SecpError> {
    tlv[tag]
        .first()
        .map(|value| {
            value
                .as_slice()
                .try_into()
                .map_err(|_| SecpError::InvalidNip19(expected_length(entity, tag, 32)))
        })
        .transpose()
}

fn required_u32(tlv: &[Vec<Vec<u8>>], tag: usize, entity: &'static str) -> Result<u32, SecpError> {
    optional_u32(tlv, tag, entity)?
        .ok_or(SecpError::InvalidNip19(missing_required_field(entity, tag)))
}

fn optional_u32(
    tlv: &[Vec<Vec<u8>>],
    tag: usize,
    entity: &'static str,
) -> Result<Option<u32>, SecpError> {
    tlv[tag]
        .first()
        .map(|value| {
            let bytes: [u8; 4] = value
                .as_slice()
                .try_into()
                .map_err(|_| SecpError::InvalidNip19(expected_length(entity, tag, 4)))?;
            Ok(u32::from_be_bytes(bytes))
        })
        .transpose()
}

fn required_utf8(
    tlv: &[Vec<Vec<u8>>],
    tag: usize,
    entity: &'static str,
) -> Result<String, SecpError> {
    let value = tlv[tag]
        .first()
        .ok_or(SecpError::InvalidNip19(missing_required_field(entity, tag)))?;
    String::from_utf8(value.clone()).map_err(|_| SecpError::InvalidNip19("invalid utf-8 payload"))
}

fn utf8_entries(
    tlv: &[Vec<Vec<u8>>],
    tag: usize,
    _entity: &'static str,
) -> Result<Vec<String>, SecpError> {
    tlv[tag]
        .iter()
        .map(|value| {
            String::from_utf8(value.clone())
                .map_err(|_| SecpError::InvalidNip19("invalid utf-8 payload"))
        })
        .collect()
}

fn missing_required_field(entity: &'static str, tag: usize) -> &'static str {
    match (entity, tag) {
        ("nprofile", 0) => "missing TLV 0 for nprofile",
        ("nevent", 0) => "missing TLV 0 for nevent",
        ("naddr", 0) => "missing TLV 0 for naddr",
        ("naddr", 2) => "missing TLV 2 for naddr",
        ("naddr", 3) => "missing TLV 3 for naddr",
        ("nrelay", 0) => "missing TLV 0 for nrelay",
        _ => "missing required tlv field",
    }
}

fn expected_length(entity: &'static str, tag: usize, len: usize) -> &'static str {
    match (entity, tag, len) {
        ("nprofile", 0, 32) => "TLV 0 should be 32 bytes",
        ("nevent", 0, 32) => "TLV 0 should be 32 bytes",
        ("nevent", 2, 32) => "TLV 2 should be 32 bytes",
        ("nevent", 3, 4) => "TLV 3 should be 4 bytes",
        ("naddr", 2, 32) => "TLV 2 should be 32 bytes",
        ("naddr", 3, 4) => "TLV 3 should be 4 bytes",
        _ => "invalid tlv length",
    }
}
