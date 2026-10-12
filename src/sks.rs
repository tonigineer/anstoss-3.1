//! Reader for Ascaron's `.sks` video format (magic `SK10`), used for the
//! ANSTOSS 3 intro and cut-scenes.
//!
//! Layout: a 32-byte header — magic, width, height, fps, frame count, a zero,
//! a flags word, the sample rate — then one `(offset, size)` pair per frame.
//! Every frame is a baseline JPEG, followed directly by its share of the
//! soundtrack: 44.1 kHz stereo IMA ADPCM in Microsoft's block layout, 136 bytes
//! per block. Concatenated, those slices form one continuous ADPCM stream.

use std::io;
use std::ops::Range;
use std::path::Path;

use crate::adpcm::{decode, wav};
use crate::cpr::{invalid, u32_at};

const HEADER_LEN: usize = 0x20;
/// Two 4-byte channel headers plus 128 bytes of nibbles.
const BLOCK_LEN: usize = 136;

pub struct Sks {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub sample_rate: u32,
    data: Vec<u8>,
    frames: Vec<Range<usize>>,
    audio: Vec<Range<usize>>,
}

impl Sks {
    pub fn open(path: &Path) -> io::Result<Self> {
        let data = std::fs::read(path)?;
        if !data.starts_with(b"SK10") {
            return Err(invalid("not an SK10 video"));
        }
        let field = |i: usize| u32_at(&data, 4 + 4 * i);
        let (width, height, fps, count) = (field(0)?, field(1)?, field(2)?, field(3)? as usize);
        let sample_rate = field(6)?;
        if fps == 0 || count == 0 {
            return Err(invalid("video has no frames"));
        }

        let mut frames = Vec::with_capacity(count);
        for i in 0..count {
            let entry = HEADER_LEN + 8 * i;
            let (offset, size) =
                (u32_at(&data, entry)? as usize, u32_at(&data, entry + 4)? as usize);
            if offset + size > data.len() {
                return Err(invalid("frame runs past the end of the file"));
            }
            frames.push(offset..offset + size);
        }
        // Audio fills the gap between one frame's JPEG and the next frame.
        let audio = frames
            .iter()
            .enumerate()
            .map(|(i, f)| f.end..frames.get(i + 1).map_or(data.len(), |next| next.start))
            .collect();

        Ok(Self { width, height, fps, sample_rate, data, frames, audio })
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn frame_jpeg(&self, i: usize) -> &[u8] {
        &self.data[self.frames[i].clone()]
    }

    /// The whole soundtrack, decoded to a 16-bit PCM WAV file in memory.
    pub fn soundtrack_wav(&self) -> Vec<u8> {
        let adpcm: Vec<u8> =
            self.audio.iter().flat_map(|r| &self.data[r.clone()]).copied().collect();
        wav(&decode(&adpcm, BLOCK_LEN), self.sample_rate)
    }
}
