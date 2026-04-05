use alloc::vec::Vec;

use crate::{CborValue, EncodeError};

pub fn encode(value: &CborValue) -> Result<Vec<u8>, EncodeError> {
    let mut output = Vec::new();
    encode_value(value, &mut output, false)?;
    Ok(output)
}

pub fn encode_dag(value: &CborValue) -> Result<Vec<u8>, EncodeError> {
    let mut output = Vec::new();
    encode_value(value, &mut output, true)?;
    Ok(output)
}

fn encode_value(
    value: &CborValue,
    output: &mut Vec<u8>,
    dag_mode: bool,
) -> Result<(), EncodeError> {
    match value {
        CborValue::Unsigned(number) => encode_head(0, *number, output),
        CborValue::Negative(number) => encode_negative(*number, output)?,
        CborValue::Bytes(bytes) => {
            encode_head(2, bytes.len() as u64, output);
            output.extend_from_slice(bytes);
        }
        CborValue::Text(text) => {
            encode_head(3, text.len() as u64, output);
            output.extend_from_slice(text.as_bytes());
        }
        CborValue::Array(items) => {
            encode_head(4, items.len() as u64, output);
            for item in items {
                encode_value(item, output, dag_mode)?;
            }
        }
        CborValue::Map(entries) => encode_map(entries, output, dag_mode)?,
        CborValue::Tag(tag, inner) => encode_tag(*tag, inner.as_ref(), output)?,
        CborValue::Bool(false) => output.push(0xF4),
        CborValue::Bool(true) => output.push(0xF5),
        CborValue::Null => output.push(0xF6),
    }
    Ok(())
}

fn encode_negative(value: i64, output: &mut Vec<u8>) -> Result<(), EncodeError> {
    if value >= 0 {
        return Err(EncodeError::InvalidNegativeValue(value));
    }

    let encoded = (-1_i128 - i128::from(value)) as u64;
    encode_head(1, encoded, output);
    Ok(())
}

fn encode_map(
    entries: &[(CborValue, CborValue)],
    output: &mut Vec<u8>,
    dag_mode: bool,
) -> Result<(), EncodeError> {
    encode_head(5, entries.len() as u64, output);

    if !dag_mode {
        for (key, value) in entries {
            encode_value(key, output, false)?;
            encode_value(value, output, false)?;
        }
        return Ok(());
    }

    let mut ordered = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let CborValue::Text(text) = key else {
            return Err(EncodeError::NonTextKeyInDagMode);
        };

        let mut encoded_key = Vec::new();
        encode_head(3, text.len() as u64, &mut encoded_key);
        encoded_key.extend_from_slice(text.as_bytes());
        ordered.push((encoded_key, value));
    }

    ordered.sort_by(|left, right| left.0.cmp(&right.0));
    for i in 1..ordered.len() {
        if ordered[i - 1].0 == ordered[i].0 {
            return Err(EncodeError::DuplicateKeyInDagMode);
        }
    }
    for (encoded_key, value) in ordered {
        output.extend_from_slice(&encoded_key);
        encode_value(value, output, true)?;
    }
    Ok(())
}

fn encode_tag(tag: u64, inner: &CborValue, output: &mut Vec<u8>) -> Result<(), EncodeError> {
    if tag != 42 {
        return Err(EncodeError::UnsupportedTag(tag));
    }

    let CborValue::Bytes(bytes) = inner else {
        return Err(EncodeError::InvalidTag42Payload);
    };
    if bytes.first().copied() != Some(0x00) {
        return Err(EncodeError::InvalidTag42Payload);
    }

    encode_head(6, tag, output);
    encode_head(2, bytes.len() as u64, output);
    output.extend_from_slice(bytes);
    Ok(())
}

fn encode_head(major: u8, value: u64, output: &mut Vec<u8>) {
    match value {
        0..=23 => output.push((major << 5) | value as u8),
        24..=0xFF => {
            output.push((major << 5) | 24);
            output.push(value as u8);
        }
        0x100..=0xFFFF => {
            output.push((major << 5) | 25);
            output.extend_from_slice(&(value as u16).to_be_bytes());
        }
        0x1_0000..=0xFFFF_FFFF => {
            output.push((major << 5) | 26);
            output.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            output.push((major << 5) | 27);
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
}
