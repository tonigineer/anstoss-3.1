//! Reader for Ascaron's `.cpr` archives (magic `BAUM`), which hold the
//! ANSTOSS 3 graphics and music.
//!
//! An archive is a chain of blocks. Each starts with a 16-byte header — magic,
//! entry count, the size reserved for the header and index, the size of it in
//! use — and its index, one entry per file: offset, size, flags, name length,
//! name. The files' data follows the reserved space, and the next block
//! follows the data. Names are NUL-padded to a multiple of four and XOR-ed
//! with the repeating key `EF CD AB 89`.
//!
//! Graphics are stored as plain JPEG/BMP/TGA. Larger files, like the music,
//! are packed (see `unpack`).

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const NAME_KEY: [u8; 4] = [0xef, 0xcd, 0xab, 0x89];
const HEADER_LEN: usize = 16;
/// Offset of entries that are listed in the index but have no data.
const MISSING: u32 = u32::MAX;
/// A packed file comes in chunks that each unpack to this many bytes.
const CHUNK: usize = 0x20000;
/// Bytes of a stored chunk are XOR-ed with this, unless the chunk is raw.
const STORE_KEY: u8 = 0x35;

pub struct Archive {
    file: File,
    entries: HashMap<String, (u32, u32)>,
}

impl Archive {
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let length = file.metadata()?.len();
        let mut entries = HashMap::new();
        let mut block = 0u64;
        while block + HEADER_LEN as u64 <= length {
            file.seek(SeekFrom::Start(block))?;
            let mut head = [0; HEADER_LEN];
            file.read_exact(&mut head)?;
            if &head[..4] != b"BAUM" {
                if block == 0 {
                    return Err(invalid("not a BAUM archive"));
                }
                break;
            }
            let count = u32_at(&head, 4)? as usize;
            let reserved = u64::from(u32_at(&head, 8)?);
            let used = (u32_at(&head, 12)? as usize)
                .checked_sub(HEADER_LEN)
                .ok_or_else(|| invalid("index ends inside the header"))?;
            let mut index = vec![0; used];
            file.read_exact(&mut index)?;
            let data = read_index(&index, count, &mut entries)?;
            if reserved < HEADER_LEN as u64 {
                return Err(invalid("block smaller than its header"));
            }
            block += reserved + data;
        }
        Ok(Self { file, entries })
    }

    /// Reads one file, unpacked. Lookup ignores case and the `/` vs `\`
    /// separator, as the game itself does.
    pub fn read(&mut self, name: &str) -> io::Result<Vec<u8>> {
        let &(offset, size) = self.entries.get(&normalise(name)).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("{name} is not in the archive"))
        })?;
        self.file.seek(SeekFrom::Start(offset.into()))?;
        let mut data = vec![0; size as usize];
        self.file.read_exact(&mut data)?;
        match unpack(&data) {
            Some(unpacked) => unpacked,
            None => Ok(data),
        }
    }
}

/// Adds a block's index entries; returns the size of the block's data.
fn read_index(
    index: &[u8],
    count: usize,
    entries: &mut HashMap<String, (u32, u32)>,
) -> io::Result<u64> {
    let mut pos = 0;
    let mut data = 0;
    for _ in 0..count {
        let (offset, size) = (u32_at(index, pos)?, u32_at(index, pos + 4)?);
        let name_len = u32_at(index, pos + 12)? as usize;
        let raw = index
            .get(pos + 16..pos + 16 + name_len)
            .ok_or_else(|| invalid("index entry runs past the index"))?;
        pos += 16 + name_len;
        // Bytes above 0x7f are cp1252 umlauts; Latin-1 is close enough for lookups.
        let name: String = raw
            .iter()
            .zip(NAME_KEY.iter().cycle())
            .map(|(b, k)| b ^ k)
            .take_while(|&b| b != 0)
            .map(char::from)
            .collect();
        if offset != MISSING {
            entries.insert(normalise(&name), (offset, size));
            data += u64::from(size);
        }
    }
    Ok(data)
}

/// A packed file, unpacked; `None` if `data` is not packed.
///
/// A packed file starts with its unpacked length and the number of chunks,
/// then the packed size of every chunk, then the chunks. A chunk starts with
/// a word: negative, it is stored — XOR-ed with 0x35 unless bit 30 is set —
/// and the word's negation (without bit 30) is its length; positive, it is
/// packed with LZ77 (see [`inflate`]) and the word is its unpacked length.
fn unpack(data: &[u8]) -> Option<io::Result<Vec<u8>>> {
    let (total, count) = (u32_at(data, 0).ok()? as usize, u32_at(data, 4).ok()? as usize);
    let table = data.get(8..8 + count.checked_mul(4)?)?;
    let sizes: Vec<usize> =
        table.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b) as usize).collect();
    // Only a packed file adds up: its chunks fill it exactly and unpack to
    // its length.
    let packed: usize = sizes.iter().sum();
    if count == 0 || 8 + table.len() + packed != data.len() || total.div_ceil(CHUNK) != count {
        return None;
    }
    let mut out = Vec::with_capacity(total);
    let mut pos = 8 + table.len();
    for size in sizes {
        let chunk = &data[pos..pos + size];
        pos += size;
        let Some(word) = chunk.get(..4).map(|w| i32::from_le_bytes(w.try_into().unwrap())) else {
            return Some(Err(invalid("empty chunk")));
        };
        let body = &chunk[4..];
        if word < 0 {
            let raw = word & 0x4000_0000 != 0;
            let len = (word.unsigned_abs() & 0x3fff_ffff) as usize;
            let Some(stored) = body.get(..len) else {
                return Some(Err(invalid("stored chunk shorter than it says")));
            };
            out.extend(stored.iter().map(|&b| if raw { b } else { b ^ STORE_KEY }));
        } else if let Err(err) = inflate(body, word as usize, &mut out) {
            return Some(Err(err));
        }
    }
    out.truncate(total);
    Some(Ok(out))
}

/// Unpacks `len` bytes of an LZ77 chunk onto `out`. The chunk is a stream of
/// bits, least significant first: a 0 bit and eight bits are a byte as it
/// is; a 1 bit starts a copy of earlier output. Three bits `n` say how many
/// bits, `n + 1`, the distance takes, offset by `2^(n+1) - 2`; the length
/// starts at 2 and grows by fields of 2, 3, 4 … bits as long as each field
/// is all ones.
fn inflate(chunk: &[u8], len: usize, out: &mut Vec<u8>) -> io::Result<()> {
    let start = out.len();
    let mut bits = Bits { data: chunk, pos: 0 };
    while out.len() - start < len {
        if bits.read(1)? == 0 {
            out.push(bits.read(8)? as u8);
            continue;
        }
        let width = bits.read(3)? + 1;
        let distance = bits.read(width)? as usize + (1 << width) - 2;
        let mut length = 2;
        let mut field = 2;
        loop {
            let value = bits.read(field)?;
            length += value as usize;
            if value != (1 << field) - 1 {
                break;
            }
            field += 1;
        }
        let from = out
            .len()
            .checked_sub(distance + 1)
            .filter(|&from| from >= start)
            .ok_or_else(|| invalid("copy from before the chunk"))?;
        for i in 0..length {
            let byte = out[from + i];
            out.push(byte);
        }
    }
    out.truncate(start + len);
    Ok(())
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn read(&mut self, count: u32) -> io::Result<u32> {
        let mut value = 0;
        for i in 0..count {
            let byte = self.data.get(self.pos / 8).ok_or_else(|| invalid("chunk ends early"))?;
            value |= u32::from(byte >> (self.pos % 8) & 1) << i;
            self.pos += 1;
        }
        Ok(value)
    }
}

fn normalise(name: &str) -> String {
    name.replace('\\', "/").to_lowercase()
}

pub fn u32_at(bytes: &[u8], pos: usize) -> io::Result<u32> {
    bytes
        .get(pos..pos + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| invalid("truncated file"))
}

pub fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes bits least significant first.
    fn bits(fields: &[(u32, u32)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut pos = 0;
        for &(value, count) in fields {
            for i in 0..count {
                if pos % 8 == 0 {
                    out.push(0);
                }
                *out.last_mut().unwrap() |= ((value >> i & 1) as u8) << (pos % 8);
                pos += 1;
            }
        }
        out
    }

    #[test]
    fn literals_and_copies_inflate() {
        // "ab", then a copy of distance 1 (width 1: 0 + 0) and length 2 + 3 + 1.
        let chunk = bits(&[
            (0, 1),
            (b'a'.into(), 8),
            (0, 1),
            (b'b'.into(), 8),
            (1, 1),
            (0, 3),
            (1, 1),
            (3, 2),
            (1, 3),
        ]);
        let mut out = Vec::new();
        inflate(&chunk, 8, &mut out).unwrap();
        assert_eq!(out, b"abababab");
    }

    #[test]
    fn a_packed_file_unpacks_stored_chunks() {
        let body: Vec<u8> = b"RIFF".iter().map(|b| b ^ STORE_KEY).collect();
        let mut data = Vec::new();
        data.extend(4u32.to_le_bytes());
        data.extend(1u32.to_le_bytes());
        data.extend(8u32.to_le_bytes());
        data.extend((-4i32 & !0x4000_0000).to_le_bytes());
        data.extend(body);
        assert_eq!(unpack(&data).unwrap().unwrap(), b"RIFF");
        // A JPEG is no packed file.
        assert!(unpack(&[0xff, 0xd8, 0xff, 0xe0, 0, 0x10, b'J', b'F', b'I', b'F']).is_none());
    }
}
