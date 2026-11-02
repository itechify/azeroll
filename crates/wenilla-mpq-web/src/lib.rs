//! Browser MPQ adapter. The service worker reads only indexes and the requested file's blob.
use std::io::Read;
use wasm_bindgen::prelude::*;
// Share the engine's Storm primitives byte for byte.
#[path = "../../benilla-mpq/src/crypto.rs"]
mod crypto;
use crypto::{decrypt_block, hash_string, hash_type};

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect()
}
fn decrypt(bytes: &mut [u8], key: u32) {
    let mut w = words(bytes);
    decrypt_block(&mut w, key);
    for (b, v) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(w) {
        b.copy_from_slice(&v.to_le_bytes());
    }
}
fn err(message: impl ToString) -> JsValue {
    JsValue::from_str(&message.to_string())
}
#[wasm_bindgen]
pub struct MpqIndex {
    hashes: Vec<u32>,
    blocks: Vec<u32>,
}
#[wasm_bindgen]
impl MpqIndex {
    #[wasm_bindgen(constructor)]
    pub fn new(mut hashes: Vec<u8>, mut blocks: Vec<u8>) -> Result<MpqIndex, JsValue> {
        if hashes.is_empty() || !hashes.len().is_multiple_of(16) || !blocks.len().is_multiple_of(16)
        {
            return Err(err("Invalid MPQ index"));
        }
        decrypt(
            &mut hashes,
            hash_string("(hash table)", hash_type::FILE_KEY),
        );
        decrypt(
            &mut blocks,
            hash_string("(block table)", hash_type::FILE_KEY),
        );
        Ok(Self {
            hashes: words(&hashes),
            blocks: words(&blocks),
        })
    }
    pub fn lookup(&self, name: &str) -> Vec<u32> {
        let n = self.hashes.len() / 4;
        let start = hash_string(name, hash_type::TABLE_OFFSET) as usize & (n - 1);
        let a = hash_string(name, hash_type::NAME_A);
        let b = hash_string(name, hash_type::NAME_B);
        for i in 0..n {
            let h = &self.hashes[((start + i) % n) * 4..][..4];
            if h[3] == u32::MAX {
                break;
            }
            if h[0] == a && h[1] == b {
                if let Some(block) = self
                    .blocks
                    .get(h[3] as usize * 4..)
                    .and_then(|b| b.get(..4))
                {
                    return block.to_vec();
                }
            }
        }
        vec![]
    }
}
fn codec(mask: u8, bytes: &[u8], want: usize) -> Result<Vec<u8>, JsValue> {
    let mut out = Vec::new();
    match mask {
        2 => {
            flate2::read::ZlibDecoder::new(bytes)
                .take(want as u64 + 1)
                .read_to_end(&mut out)
                .map_err(err)?;
        }
        16 => {
            bzip2::read::BzDecoder::new(bytes)
                .take(want as u64 + 1)
                .read_to_end(&mut out)
                .map_err(err)?;
        }
        8 => {
            explode::ExplodeReader::new(bytes)
                .take(want as u64 + 1)
                .read_to_end(&mut out)
                .map_err(err)?;
        }
        _ => return Err(err(format!("Unsupported MPQ compression mask {mask:#x}"))),
    }
    if out.len() != want {
        return Err(err("MPQ decoded size mismatch"));
    }
    Ok(out)
}
#[wasm_bindgen]
pub fn extract(
    name: &str,
    mut raw: Vec<u8>,
    pos: u32,
    size: usize,
    flags: u32,
    sector_size: usize,
) -> Result<Vec<u8>, JsValue> {
    if size > 128 * 1024 * 1024 || sector_size == 0 || sector_size > 1024 * 1024 {
        return Err(err("MPQ file exceeds safety limit"));
    }
    let mut key = hash_string(
        name.rsplit(['\\', '/']).next().unwrap_or(name),
        hash_type::FILE_KEY,
    );
    if flags & 0x20000 != 0 {
        key = key.wrapping_add(pos) ^ size as u32;
    }
    let encrypted = flags & 0x10000 != 0;
    let compressed = flags & 0x300 != 0;
    let decode = |mut b: Vec<u8>, want: usize, k: u32| -> Result<Vec<u8>, JsValue> {
        if encrypted {
            decrypt(&mut b, k);
        }
        if !compressed || b.len() == want {
            if b.len() != want {
                return Err(err("Truncated MPQ file"));
            }
            return Ok(b);
        }
        if flags & 0x200 != 0 {
            let (&mask, data) = b.split_first().ok_or_else(|| err("Empty MPQ sector"))?;
            codec(mask, data, want)
        } else {
            codec(8, &b, want)
        }
    };
    if flags & 0x1000000 != 0 {
        return decode(raw, size, key);
    }
    if !compressed {
        if encrypted {
            let mut out = Vec::with_capacity(size);
            for (i, b) in raw.chunks(sector_size).enumerate() {
                out.extend(decode(b.to_vec(), b.len(), key.wrapping_add(i as u32))?);
            }
            return Ok(out);
        }
        raw.truncate(size);
        if raw.len() != size {
            return Err(err("Truncated MPQ file"));
        }
        return Ok(raw);
    }
    let count = size.div_ceil(sector_size);
    let table_len = (count + 1) * 4;
    let table = raw
        .get_mut(..table_len)
        .ok_or_else(|| err("Truncated MPQ sector table"))?;
    if encrypted {
        decrypt(table, key.wrapping_sub(1));
    }
    let offsets = words(table);
    let mut out = Vec::with_capacity(size);
    for i in 0..count {
        let start = offsets[i] as usize;
        let end = offsets[i + 1] as usize;
        if start < table_len || end < start {
            return Err(err("Invalid MPQ sector offsets"));
        }
        let b = raw
            .get(start..end)
            .ok_or_else(|| err("MPQ sector outside file"))?;
        out.extend(decode(
            b.to_vec(),
            (size - i * sector_size).min(sector_size),
            key.wrapping_add(i as u32),
        )?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn codecs_and_encrypted_sectors() {
        let plain = b"AIAIAIAIAIAIA";
        assert_eq!(
            codec(8, &[0, 4, 0x82, 0x24, 0x25, 0x8f, 0x80, 0x7f], 13).unwrap(),
            plain
        );
        let data = vec![b'A'; 8192];
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&data).unwrap();
        let z = z.finish().unwrap();
        assert_eq!(codec(2, &z, data.len()).unwrap(), data);
        let mut b = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
        b.write_all(&data).unwrap();
        assert_eq!(codec(16, &b.finish().unwrap(), data.len()).unwrap(), data);
        // A two-sector encrypted file with fixed key and CRC table padding.
        let name = "World\\test.adt";
        let key = hash_string("test.adt", hash_type::FILE_KEY).wrapping_add(512) ^ 16384;
        let mut raw = vec![0u8; 20];
        let mut offsets = vec![20, 20 + (z.len() + 1) as u32, 20 + 2 * (z.len() + 1) as u32];
        crypto::encrypt_block(&mut offsets, key.wrapping_sub(1));
        for (b, v) in raw.as_chunks_mut::<4>().0.iter_mut().zip(offsets) {
            b.copy_from_slice(&v.to_le_bytes());
        }
        for i in 0..2 {
            let mut sector = vec![2];
            sector.extend(&z);
            // Use 8192-byte sectors, so each sector's expected size is 8192.
            let mut w = words(&sector);
            crypto::encrypt_block(&mut w, key + i);
            for (b, v) in sector.as_chunks_mut::<4>().0.iter_mut().zip(w) {
                b.copy_from_slice(&v.to_le_bytes());
            }
            raw.extend(sector);
        }
        assert_eq!(
            extract(name, raw, 512, 16384, 0x80030200, 8192).unwrap(),
            data.repeat(2)
        );
        // Separate single-unit encrypted stored-file path.
        let key = hash_string("test.adt", hash_type::FILE_KEY).wrapping_add(512) ^ 8192;
        let mut stored = words(&data);
        crypto::encrypt_block(&mut stored, key);
        let raw: Vec<u8> = stored.into_iter().flat_map(u32::to_le_bytes).collect();
        assert_eq!(
            extract(name, raw, 512, 8192, 0x81030000, 4096).unwrap(),
            data
        );
    }
}
