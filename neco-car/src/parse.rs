use neco_cbor::CborValue;
use neco_cid::Cid;

use crate::error::CarError;
use crate::types::{CarEntry, CarV1};

pub fn parse_v1(input: &[u8]) -> Result<CarV1, CarError> {
    let (header_len, mut offset) = decode_varint(input)?;
    let header_len = header_len as usize;

    let header_end = offset
        .checked_add(header_len)
        .filter(|&end| end <= input.len())
        .ok_or(CarError::UnexpectedEnd)?;

    let header_bytes = &input[offset..header_end];
    offset = header_end;

    let header = neco_cbor::decode_dag(header_bytes).map_err(|e| {
        let kind = e.kind().clone();
        CarError::InvalidHeader(kind)
    })?;

    let CborValue::Map(entries) = &header else {
        return Err(CarError::HeaderNotMap);
    };

    let version = find_field(entries, "version")
        .ok_or(CarError::MissingHeaderField("version"))?
        .as_unsigned()
        .ok_or(CarError::MissingHeaderField("version"))?;

    if version != 1 {
        return Err(CarError::UnsupportedVersion(version));
    }

    let roots_value = find_field(entries, "roots").ok_or(CarError::MissingHeaderField("roots"))?;

    let roots_array = roots_value.as_array().ok_or(CarError::RootsNotArray)?;

    let mut roots = Vec::with_capacity(roots_array.len());
    for item in roots_array {
        let cid = extract_cid_link(item)?;
        roots.push(cid);
    }

    let mut blocks = Vec::new();
    while offset < input.len() {
        let (section_len, varint_size) = decode_varint(&input[offset..])?;
        offset += varint_size;

        if section_len == 0 {
            return Err(CarError::EmptySection);
        }

        let section_len = section_len as usize;
        let section_end = offset
            .checked_add(section_len)
            .filter(|&end| end <= input.len())
            .ok_or(CarError::UnexpectedEnd)?;

        let section = &input[offset..section_end];

        let (cid, cid_len) = Cid::from_bytes(section).map_err(CarError::InvalidBlockCid)?;

        // Defensive guard: Cid::from_bytes reports consuming more than available.
        // In practice, Cid::from_bytes would fail first with InvalidBlockCid.
        if cid_len > section_len {
            return Err(CarError::BlockLengthMismatch);
        }

        let data = section[cid_len..].to_vec();
        blocks.push(CarEntry::new(cid, data));

        offset = section_end;
    }

    Ok(CarV1::new(roots, blocks))
}

fn find_field<'a>(entries: &'a [(CborValue, CborValue)], key: &str) -> Option<&'a CborValue> {
    entries.iter().find_map(|(k, v)| match k {
        CborValue::Text(text) if text == key => Some(v),
        _ => None,
    })
}

fn extract_cid_link(value: &CborValue) -> Result<Cid, CarError> {
    let (tag, inner) = value.as_tag().ok_or(CarError::InvalidCidLink)?;
    if tag != 42 {
        return Err(CarError::InvalidCidLink);
    }
    let bytes = inner.as_bytes().ok_or(CarError::InvalidCidLink)?;
    if bytes.first().copied() != Some(0x00) {
        return Err(CarError::InvalidCidLink);
    }
    let (cid, _consumed) = Cid::from_bytes(&bytes[1..]).map_err(CarError::InvalidRootCid)?;
    Ok(cid)
}

fn decode_varint(input: &[u8]) -> Result<(u64, usize), CarError> {
    let mut value = 0u64;
    let mut shift = 0u32;

    for (index, &byte) in input.iter().enumerate() {
        let chunk = u64::from(byte & 0x7f);
        value |= chunk << shift;
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
        shift += 7;
        if shift >= 64 {
            return Err(CarError::VarintOverflow);
        }
    }

    Err(CarError::UnexpectedEnd)
}
