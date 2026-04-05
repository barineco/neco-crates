use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{CborValue, DecodeError, DecodeErrorKind};

const MAX_DEPTH: usize = 128;

pub fn decode(input: &[u8]) -> Result<CborValue, DecodeError> {
    let mut decoder = Decoder::new(input, false);
    let value = decoder.decode_value()?;
    if decoder.position < input.len() {
        return Err(decoder.error(DecodeErrorKind::TrailingContent));
    }
    Ok(value)
}

pub fn decode_dag(input: &[u8]) -> Result<CborValue, DecodeError> {
    let mut decoder = Decoder::new(input, true);
    let value = decoder.decode_value()?;
    if decoder.position < input.len() {
        return Err(decoder.error(DecodeErrorKind::TrailingContent));
    }
    Ok(value)
}

pub fn decode_one(input: &[u8]) -> Result<(CborValue, usize), DecodeError> {
    let mut decoder = Decoder::new(input, false);
    let value = decoder.decode_value()?;
    Ok((value, decoder.position))
}

struct Decoder<'a> {
    input: &'a [u8],
    position: usize,
    depth: usize,
    dag_mode: bool,
}

impl<'a> Decoder<'a> {
    fn new(input: &'a [u8], dag_mode: bool) -> Self {
        Self {
            input,
            position: 0,
            depth: 0,
            dag_mode,
        }
    }

    fn decode_value(&mut self) -> Result<CborValue, DecodeError> {
        let initial = self.read_byte()?;
        let major = initial >> 5;
        let additional = initial & 0x1F;

        match major {
            0 => Ok(CborValue::Unsigned(
                self.read_argument(major, additional, true)?,
            )),
            1 => self.decode_negative(additional),
            2 => self.decode_bytes(additional),
            3 => self.decode_text(additional),
            4 => self.decode_array(additional),
            5 => self.decode_map(additional),
            6 => self.decode_tag(additional),
            7 => self.decode_simple(additional),
            _ => Err(self.error(DecodeErrorKind::InvalidMajorType(major))),
        }
    }

    fn decode_negative(&mut self, additional: u8) -> Result<CborValue, DecodeError> {
        let encoded = self.read_argument(1, additional, true)?;
        if encoded > i64::MAX as u64 {
            return Err(self.error(DecodeErrorKind::IntegerOverflow));
        }
        Ok(CborValue::Negative(-1 - encoded as i64))
    }

    fn decode_bytes(&mut self, additional: u8) -> Result<CborValue, DecodeError> {
        let length = self.read_argument(2, additional, false)? as usize;
        let bytes = self.read_slice(length)?;
        Ok(CborValue::Bytes(bytes.to_vec()))
    }

    fn decode_text(&mut self, additional: u8) -> Result<CborValue, DecodeError> {
        let length = self.read_argument(3, additional, false)? as usize;
        let bytes = self.read_slice(length)?;
        let text = core::str::from_utf8(bytes).map_err(|_| {
            self.error_at(
                self.position.saturating_sub(length),
                DecodeErrorKind::InvalidUtf8,
            )
        })?;
        Ok(CborValue::Text(String::from(text)))
    }

    fn decode_array(&mut self, additional: u8) -> Result<CborValue, DecodeError> {
        let length = self.read_argument(4, additional, false)? as usize;
        self.push_depth()?;
        let mut items = Vec::with_capacity(length);
        for _ in 0..length {
            items.push(self.decode_value()?);
        }
        self.depth -= 1;
        Ok(CborValue::Array(items))
    }

    fn decode_map(&mut self, additional: u8) -> Result<CborValue, DecodeError> {
        let length = self.read_argument(5, additional, false)? as usize;
        self.push_depth()?;
        let mut entries = Vec::with_capacity(length);
        let mut previous_key_bytes: Option<Vec<u8>> = None;

        for _ in 0..length {
            let key = self.decode_value()?;
            let current_key_bytes = if self.dag_mode {
                match &key {
                    CborValue::Text(text) => {
                        let mut encoded = Vec::new();
                        text_key_to_bytes(text, &mut encoded);
                        if let Some(previous) = previous_key_bytes.as_ref() {
                            if previous == &encoded {
                                return Err(self.error(DecodeErrorKind::DuplicateMapKey));
                            }
                            if previous > &encoded {
                                return Err(self.error(DecodeErrorKind::UnsortedMapKeys));
                            }
                        }
                        Some(encoded)
                    }
                    _ => return Err(self.error(DecodeErrorKind::NonTextMapKey)),
                }
            } else {
                None
            };

            let value = self.decode_value()?;
            if let Some(encoded) = current_key_bytes {
                previous_key_bytes = Some(encoded);
            }
            entries.push((key, value));
        }

        self.depth -= 1;
        Ok(CborValue::Map(entries))
    }

    fn decode_tag(&mut self, additional: u8) -> Result<CborValue, DecodeError> {
        let tag = self.read_argument(6, additional, false)?;
        if tag != 42 {
            return Err(self.error(DecodeErrorKind::UnsupportedTag(tag)));
        }

        let value = self.decode_value()?;
        let CborValue::Bytes(bytes) = &value else {
            return Err(self.error(DecodeErrorKind::UnsupportedTag(tag)));
        };
        if bytes.first().copied() != Some(0x00) {
            return Err(self.error(DecodeErrorKind::UnsupportedTag(tag)));
        }

        Ok(CborValue::Tag(tag, Box::new(value)))
    }

    fn decode_simple(&mut self, additional: u8) -> Result<CborValue, DecodeError> {
        match additional {
            20 => Ok(CborValue::Bool(false)),
            21 => Ok(CborValue::Bool(true)),
            22 => Ok(CborValue::Null),
            // CborValue has no float variant, so floats are unsupported in both modes.
            25..=27 => Err(self.error(DecodeErrorKind::FloatNotAllowed)),
            31 => Err(self.error(DecodeErrorKind::IndefiniteLength)),
            _ => Err(self.error(DecodeErrorKind::InvalidMajorType(7))),
        }
    }

    fn read_argument(
        &mut self,
        major: u8,
        additional: u8,
        canonical_integer: bool,
    ) -> Result<u64, DecodeError> {
        let value = match additional {
            0..=23 => u64::from(additional),
            24 => u64::from(self.read_byte()?),
            25 => u64::from(self.read_u16()?),
            26 => u64::from(self.read_u32()?),
            27 => self.read_u64()?,
            31 => return Err(self.error(DecodeErrorKind::IndefiniteLength)),
            _ => return Err(self.error(DecodeErrorKind::InvalidMajorType(major))),
        };

        if self.dag_mode && canonical_integer && !is_canonical_integer(additional, value) {
            return Err(self.error(DecodeErrorKind::NonCanonicalInteger));
        }
        Ok(value)
    }

    fn push_depth(&mut self) -> Result<(), DecodeError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error(DecodeErrorKind::NestingTooDeep));
        }
        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8, DecodeError> {
        match self.input.get(self.position).copied() {
            Some(byte) => {
                self.position += 1;
                Ok(byte)
            }
            None => Err(self.error(DecodeErrorKind::UnexpectedEnd)),
        }
    }

    fn read_u16(&mut self) -> Result<u16, DecodeError> {
        let bytes = self.read_slice(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, DecodeError> {
        let bytes = self.read_slice(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, DecodeError> {
        let bytes = self.read_slice(8)?;
        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_slice(&mut self, length: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| self.error(DecodeErrorKind::UnexpectedEnd))?;
        let slice = self
            .input
            .get(self.position..end)
            .ok_or_else(|| self.error(DecodeErrorKind::UnexpectedEnd))?;
        self.position = end;
        Ok(slice)
    }

    fn error(&self, kind: DecodeErrorKind) -> DecodeError {
        DecodeError::new(kind, self.position)
    }

    fn error_at(&self, position: usize, kind: DecodeErrorKind) -> DecodeError {
        DecodeError::new(kind, position)
    }
}

fn is_canonical_integer(additional: u8, value: u64) -> bool {
    match additional {
        0..=23 => true,
        24 => value >= 24,
        25 => value > 0xFF,
        26 => value > 0xFFFF,
        27 => value > 0xFFFF_FFFF,
        _ => false,
    }
}

fn text_key_to_bytes(text: &str, output: &mut Vec<u8>) {
    let length = text.len() as u64;
    match length {
        0..=23 => output.push(0x60 | length as u8),
        24..=0xFF => {
            output.push(0x78);
            output.push(length as u8);
        }
        0x100..=0xFFFF => {
            output.push(0x79);
            output.extend_from_slice(&(length as u16).to_be_bytes());
        }
        0x1_0000..=0xFFFF_FFFF => {
            output.push(0x7A);
            output.extend_from_slice(&(length as u32).to_be_bytes());
        }
        _ => {
            output.push(0x7B);
            output.extend_from_slice(&length.to_be_bytes());
        }
    }
    output.extend_from_slice(text.as_bytes());
}
