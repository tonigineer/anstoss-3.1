//! Tables the game keeps inside `anstoss3.exe` rather than in its data
//! files, read from the installed executable by their addresses.

use std::io;
use std::path::Path;

use crate::cpr::{invalid, u32_at};

pub struct Exe {
    data: Vec<u8>,
    base: u32,
    /// Each section's address, size in memory, and where and how much of it
    /// the file holds.
    sections: Vec<(u32, u32, usize, usize)>,
}

impl Exe {
    pub fn open(path: &Path) -> io::Result<Self> {
        let data = std::fs::read(path)?;
        let header = u32_at(&data, 0x3c)? as usize;
        if data.get(header..header + 4) != Some(b"PE\0\0") {
            return Err(invalid("not a Windows executable"));
        }
        let u16_at = |pos: usize| data.get(pos..pos + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let count = u16_at(header + 6).ok_or_else(|| invalid("truncated header"))?;
        let optional = u16_at(header + 20).ok_or_else(|| invalid("truncated header"))?;
        let base = u32_at(&data, header + 24 + 28)?;
        let first = header + 24 + usize::from(optional);
        let sections = (0..usize::from(count))
            .map(|i| {
                let at = first + 40 * i;
                Ok((
                    base + u32_at(&data, at + 12)?,
                    u32_at(&data, at + 8)?,
                    u32_at(&data, at + 20)? as usize,
                    u32_at(&data, at + 16)? as usize,
                ))
            })
            .collect::<io::Result<_>>()?;
        Ok(Self { data, base, sections })
    }

    /// `len` bytes at the address `at`, as the file holds them.
    pub fn bytes(&self, at: u32, len: usize) -> io::Result<&[u8]> {
        let missing = || invalid(&format!("{at:#x} is not in the executable's file"));
        let &(start, _, raw, raw_len) = self
            .sections
            .iter()
            .find(|&&(start, size, _, raw_len)| {
                at >= start && at - start < size.max(raw_len as u32) && at >= self.base
            })
            .ok_or_else(missing)?;
        let offset = (at - start) as usize;
        if offset + len > raw_len {
            return Err(missing());
        }
        self.data.get(raw + offset..raw + offset + len).ok_or_else(missing)
    }

    /// The text a pointer points at; none where the pointer leads outside
    /// the file, as to memory the game fills in at run time.
    pub fn text_at(&self, pointer: u32) -> Option<String> {
        let mut text = Vec::new();
        for at in pointer.. {
            match self.bytes(at, 1).ok()?[0] {
                0 => break,
                byte => text.push(byte),
            }
        }
        Some(crate::sav::cp1252(&text))
    }

    pub fn u16s(&self, at: u32, count: usize) -> io::Result<Vec<u16>> {
        let bytes = self.bytes(at, 2 * count)?;
        Ok(bytes.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).collect())
    }

    pub fn u32s(&self, at: u32, count: usize) -> io::Result<Vec<u32>> {
        let bytes = self.bytes(at, 4 * count)?;
        Ok(bytes.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b)).collect())
    }
}
