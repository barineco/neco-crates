use neco_cbor::CborValue;
use neco_cid::Cid;

use crate::error::CarError;

pub fn write_v1(roots: &[Cid], blocks: &[(Cid, &[u8])]) -> Result<Vec<u8>, CarError> {
    let root_values: Vec<CborValue> = roots
        .iter()
        .map(|cid| {
            let cid_bytes = cid.to_bytes();
            let mut payload = Vec::with_capacity(1 + cid_bytes.len());
            payload.push(0x00);
            payload.extend_from_slice(&cid_bytes);
            CborValue::Tag(42, Box::new(CborValue::Bytes(payload)))
        })
        .collect();

    let header = CborValue::Map(vec![
        (
            CborValue::Text("roots".into()),
            CborValue::Array(root_values),
        ),
        (CborValue::Text("version".into()), CborValue::Unsigned(1)),
    ]);

    let header_bytes = neco_cbor::encode_dag(&header).map_err(CarError::HeaderEncode)?;

    let mut output = Vec::new();
    encode_varint(header_bytes.len() as u64, &mut output);
    output.extend_from_slice(&header_bytes);

    for (cid, data) in blocks {
        let cid_bytes = cid.to_bytes();
        let section_len = cid_bytes.len() + data.len();
        encode_varint(section_len as u64, &mut output);
        output.extend_from_slice(&cid_bytes);
        output.extend_from_slice(data);
    }

    Ok(output)
}

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let lower = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(lower);
            return;
        }
        out.push(lower | 0x80);
    }
}
