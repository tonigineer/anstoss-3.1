//! Microsoft's IMA ADPCM, the sound of the intro video and of the music:
//! stereo blocks, each with a header per channel — its first sample and step
//! index — then groups of four bytes per channel, eight samples each.

const CHANNELS: usize = 2;

const INDEX_TABLE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];
const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

struct Channel {
    predictor: i32,
    index: i32,
}

impl Channel {
    fn next(&mut self, nibble: u8) -> i16 {
        let step = STEP_TABLE[self.index as usize];
        let mut diff = step >> 3;
        if nibble & 1 != 0 {
            diff += step >> 2;
        }
        if nibble & 2 != 0 {
            diff += step >> 1;
        }
        if nibble & 4 != 0 {
            diff += step;
        }
        if nibble & 8 != 0 {
            diff = -diff;
        }
        self.predictor = (self.predictor + diff).clamp(i16::MIN.into(), i16::MAX.into());
        self.index = (self.index + INDEX_TABLE[nibble as usize]).clamp(0, 88);
        self.predictor as i16
    }
}

/// Decodes stereo IMA ADPCM in blocks of `block_len` bytes into interleaved
/// 16-bit samples.
pub fn decode(adpcm: &[u8], block_len: usize) -> Vec<i16> {
    let mut pcm = Vec::with_capacity(adpcm.len() * 2);
    for block in adpcm.chunks_exact(block_len) {
        decode_block(block, &mut pcm);
    }
    pcm
}

/// Decodes one block into interleaved stereo samples: the header sample of
/// each channel, then groups of 4 bytes (8 samples) per channel, low nibble
/// first.
fn decode_block(block: &[u8], out: &mut Vec<i16>) {
    let mut channels: Vec<Channel> = (0..CHANNELS)
        .map(|c| Channel {
            predictor: i16::from_le_bytes([block[4 * c], block[4 * c + 1]]).into(),
            index: i32::from(block[4 * c + 2]).min(88),
        })
        .collect();
    out.extend(channels.iter().map(|c| c.predictor as i16));

    let mut decoded = [[0i16; 8]; CHANNELS];
    for group in block[4 * CHANNELS..].as_chunks::<{ 4 * CHANNELS }>().0 {
        for (c, bytes) in group.as_chunks::<4>().0.iter().enumerate() {
            for (j, byte) in bytes.iter().enumerate() {
                decoded[c][2 * j] = channels[c].next(byte & 0x0f);
                decoded[c][2 * j + 1] = channels[c].next(byte >> 4);
            }
        }
        for s in 0..8 {
            out.extend(decoded.iter().map(|channel| channel[s]));
        }
    }
}

/// A 16-bit stereo PCM WAV file in memory.
pub fn wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let block_align = (CHANNELS * 2) as u16;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&(CHANNELS as u16).to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * u32::from(block_align)).to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
    out
}
