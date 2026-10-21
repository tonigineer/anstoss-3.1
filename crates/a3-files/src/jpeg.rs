//! The game's JPEGs as its own decoder shows them: baseline Huffman
//! JPEGs whose colour planes are repeated over the pixels they cover rather
//! than smoothed between them (what `djpeg -nosmooth` gives) — which shows
//! on sharp colour edges, as the calculations' backgrounds have them.
//! Anything else (progressive, arithmetic, twelve bits) is left to another
//! decoder.

use image::{Rgb, RgbImage};

const SOI: u8 = 0xd8;
const EOI: u8 = 0xd9;
const SOS: u8 = 0xda;
const DQT: u8 = 0xdb;
const DHT: u8 = 0xc4;
const DRI: u8 = 0xdd;
/// Baseline and extended sequential Huffman frames.
const SOF0: u8 = 0xc0;
const SOF1: u8 = 0xc1;
const RST0: u8 = 0xd0;
const RST7: u8 = 0xd7;

/// The order the coefficients come in.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

struct Component {
    id: u8,
    h: usize,
    v: usize,
    table: usize,
    /// Its samples, a whole number of blocks wide and high.
    width: usize,
    height: usize,
    samples: Vec<u8>,
    dc_table: usize,
    ac_table: usize,
    predictor: i32,
}

#[derive(Clone, Default)]
struct Huffman {
    /// By code length: the codes' first value and index, and the values.
    max_code: [i32; 18],
    val_offset: [i32; 18],
    values: Vec<u8>,
}

impl Huffman {
    fn new(counts: &[u8; 16], values: Vec<u8>) -> Self {
        let mut h = Huffman { values, ..Huffman::default() };
        let (mut code, mut k) = (0i32, 0i32);
        for len in 1..=16 {
            let n = i32::from(counts[len - 1]);
            h.val_offset[len] = k - code;
            code += n;
            k += n;
            h.max_code[len] = if n > 0 { code - 1 } else { -1 };
            code <<= 1;
        }
        h.max_code[17] = i32::MAX;
        h
    }
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    acc: u32,
    count: u32,
}

impl Bits<'_> {
    fn bit(&mut self) -> Option<u32> {
        if self.count == 0 {
            let byte = *self.data.get(self.pos)?;
            if byte == 0xff {
                let next = *self.data.get(self.pos + 1)?;
                if next != 0 {
                    // A marker: pad with ones, as decoders do.
                    self.acc = 0xff;
                    self.count = 8;
                } else {
                    self.pos += 2;
                    self.acc = 0xff;
                    self.count = 8;
                }
            } else {
                self.pos += 1;
                self.acc = u32::from(byte);
                self.count = 8;
            }
        }
        self.count -= 1;
        Some((self.acc >> self.count) & 1)
    }

    fn bits(&mut self, n: u32) -> Option<i32> {
        let mut v = 0;
        for _ in 0..n {
            v = (v << 1) | self.bit()?;
        }
        Some(v as i32)
    }

    fn decode(&mut self, h: &Huffman) -> Option<u8> {
        let mut code = 0i32;
        for len in 1..=16 {
            code = (code << 1) | self.bit()? as i32;
            if code <= h.max_code[len] {
                return h.values.get((code + h.val_offset[len]) as usize).copied();
            }
        }
        None
    }

    /// A restart marker: the bits left over dropped, the marker passed.
    fn restart(&mut self) {
        self.count = 0;
        while self.pos + 1 < self.data.len() {
            if self.data[self.pos] == 0xff && (RST0..=RST7).contains(&self.data[self.pos + 1]) {
                self.pos += 2;
                return;
            }
            self.pos += 1;
        }
    }
}

fn extend(v: i32, bits: u32) -> i32 {
    if bits == 0 {
        0
    } else if v < 1 << (bits - 1) {
        v - (1 << bits) + 1
    } else {
        v
    }
}

/// The inverse DCT of a block, into its samples (level-shifted, clamped).
fn idct(coefficients: &[i32; 64], out: &mut [u8], stride: usize) {
    let mut tmp = [0f32; 64];
    let c = |u: usize| if u == 0 { std::f32::consts::FRAC_1_SQRT_2 } else { 1.0 };
    let mut cos = [[0f32; 8]; 8];
    for (x, row) in cos.iter_mut().enumerate() {
        for (u, v) in row.iter_mut().enumerate() {
            *v = c(u) * (((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI) / 16.0).cos();
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            tmp[y * 8 + x] = (0..8).map(|u| cos[x][u] * coefficients[y * 8 + u] as f32).sum();
        }
    }
    for x in 0..8 {
        for y in 0..8 {
            let v: f32 = (0..8).map(|v| cos[y][v] * tmp[v * 8 + x]).sum::<f32>() / 4.0;
            out[y * stride + x] = (v.round() as i32 + 128).clamp(0, 255) as u8;
        }
    }
}

/// Decodes a baseline JPEG; `None` for anything else.
pub fn decode(data: &[u8]) -> Option<RgbImage> {
    if data.get(..2)? != [0xff, SOI] {
        return None;
    }
    let mut pos = 2;
    let mut tables = [[0u16; 64]; 4];
    let mut dc = vec![Huffman::default(); 4];
    let mut ac = vec![Huffman::default(); 4];
    let mut components: Vec<Component> = Vec::new();
    let (mut width, mut height) = (0usize, 0usize);
    let mut restart_interval = 0usize;
    let (mut hmax, mut vmax) = (1, 1);
    loop {
        while *data.get(pos)? == 0xff && *data.get(pos + 1)? == 0xff {
            pos += 1;
        }
        if *data.get(pos)? != 0xff {
            return None;
        }
        let marker = *data.get(pos + 1)?;
        pos += 2;
        if marker == EOI {
            break;
        }
        let len = usize::from(u16::from_be_bytes([*data.get(pos)?, *data.get(pos + 1)?]));
        let segment = data.get(pos + 2..pos + len)?;
        match marker {
            DQT => {
                let mut s = segment;
                while !s.is_empty() {
                    let (precision, id) = (s[0] >> 4, usize::from(s[0] & 3));
                    s = &s[1..];
                    for k in 0..64 {
                        tables[id][ZIGZAG[k]] = if precision == 0 {
                            u16::from(*s.get(k)?)
                        } else {
                            u16::from_be_bytes([*s.get(2 * k)?, *s.get(2 * k + 1)?])
                        };
                    }
                    s = &s[if precision == 0 { 64 } else { 128 }..];
                }
            }
            DHT => {
                let mut s = segment;
                while !s.is_empty() {
                    let (class, id) = (s[0] >> 4, usize::from(s[0] & 3));
                    let counts: [u8; 16] = s.get(1..17)?.try_into().ok()?;
                    let n: usize = counts.iter().map(|&c| usize::from(c)).sum();
                    let values = s.get(17..17 + n)?.to_vec();
                    let table = Huffman::new(&counts, values);
                    if class == 0 {
                        dc[id] = table
                    } else {
                        ac[id] = table
                    }
                    s = &s[17 + n..];
                }
            }
            DRI => restart_interval = usize::from(u16::from_be_bytes([segment[0], segment[1]])),
            SOF0 | SOF1 => {
                if segment[0] != 8 {
                    return None;
                }
                height = usize::from(u16::from_be_bytes([segment[1], segment[2]]));
                width = usize::from(u16::from_be_bytes([segment[3], segment[4]]));
                let n = usize::from(segment[5]);
                for k in 0..n {
                    let c = segment.get(6 + 3 * k..9 + 3 * k)?;
                    let (h, v) = (usize::from(c[1] >> 4), usize::from(c[1] & 15));
                    components.push(Component {
                        id: c[0],
                        h,
                        v,
                        table: usize::from(c[2] & 3),
                        width: 0,
                        height: 0,
                        samples: Vec::new(),
                        dc_table: 0,
                        ac_table: 0,
                        predictor: 0,
                    });
                }
                hmax = components.iter().map(|c| c.h).max()?;
                vmax = components.iter().map(|c| c.v).max()?;
                let (mcus_x, mcus_y) = (width.div_ceil(8 * hmax), height.div_ceil(8 * vmax));
                for c in &mut components {
                    c.width = mcus_x * c.h * 8;
                    c.height = mcus_y * c.v * 8;
                    c.samples = vec![0; c.width * c.height];
                }
            }
            // Progressive, lossless, arithmetic: another decoder's.
            0xc2 | 0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => return None,
            SOS => {
                let n = usize::from(segment[0]);
                let mut scan = Vec::new();
                for k in 0..n {
                    let (id, t) = (segment[1 + 2 * k], segment[2 + 2 * k]);
                    let index = components.iter().position(|c| c.id == id)?;
                    components[index].dc_table = usize::from(t >> 4);
                    components[index].ac_table = usize::from(t & 15);
                    scan.push(index);
                }
                let start = pos + len;
                let used = decode_scan(
                    data.get(start..)?,
                    &mut components,
                    &scan,
                    (&tables, &dc, &ac),
                    (width, height, hmax, vmax),
                    restart_interval,
                )?;
                pos = start + used;
                continue;
            }
            _ => {}
        }
        pos += len;
    }
    let mut image = RgbImage::new(width as u32, height as u32);
    let sample =
        |c: &Component, x: usize, y: usize| c.samples[(y * c.v / vmax) * c.width + x * c.h / hmax];
    for y in 0..height {
        for x in 0..width {
            let pixel = match components.as_slice() {
                [luma] => {
                    let l = sample(luma, x, y);
                    [l, l, l]
                }
                [luma, cb, cr] => {
                    ycc_to_rgb(sample(luma, x, y), sample(cb, x, y), sample(cr, x, y))
                }
                _ => return None,
            };
            image.put_pixel(x as u32, y as u32, Rgb(pixel));
        }
    }
    Some(image)
}

/// A scan's entropy-coded data into the components' samples; how many
/// bytes it took, up to the next marker.
fn decode_scan(
    data: &[u8],
    components: &mut [Component],
    scan: &[usize],
    (tables, dc, ac): (&[[u16; 64]; 4], &[Huffman], &[Huffman]),
    (width, height, hmax, vmax): (usize, usize, usize, usize),
    restart_interval: usize,
) -> Option<usize> {
    let mut bits = Bits { data, pos: 0, acc: 0, count: 0 };
    for &i in scan {
        components[i].predictor = 0;
    }
    // One component alone runs through its own blocks; several by MCUs.
    let units: Vec<(usize, usize, usize)> = if let [i] = scan {
        let c = &components[*i];
        let (bw, bh) = ((width * c.h).div_ceil(8 * hmax), (height * c.v).div_ceil(8 * vmax));
        (0..bh).flat_map(|by| (0..bw).map(move |bx| (*i, bx, by))).collect()
    } else {
        let (mcus_x, mcus_y) = (width.div_ceil(8 * hmax), height.div_ceil(8 * vmax));
        let mut units = Vec::new();
        for my in 0..mcus_y {
            for mx in 0..mcus_x {
                for &i in scan {
                    let c = &components[i];
                    for v in 0..c.v {
                        for h in 0..c.h {
                            units.push((i, mx * c.h + h, my * c.v + v));
                        }
                    }
                }
            }
        }
        units
    };
    let per_unit = if let [_] = scan {
        1
    } else {
        scan.iter().map(|&i| components[i].h * components[i].v).sum()
    };
    let mut block = [0i32; 64];
    for (k, &(i, bx, by)) in units.iter().enumerate() {
        if restart_interval > 0 && k > 0 && k % (restart_interval * per_unit) == 0 {
            bits.restart();
            for &j in scan {
                components[j].predictor = 0;
            }
        }
        block.fill(0);
        let c = &mut components[i];
        let quant = &tables[c.table];
        let size = u32::from(bits.decode(&dc[c.dc_table])?);
        c.predictor += extend(bits.bits(size)?, size);
        block[0] = c.predictor * i32::from(quant[0]);
        let mut z = 1;
        while z < 64 {
            let rs = bits.decode(&ac[c.ac_table])?;
            let (run, size) = (usize::from(rs >> 4), u32::from(rs & 15));
            if size == 0 {
                if run == 15 {
                    z += 16;
                    continue;
                }
                break;
            }
            z += run;
            if z >= 64 {
                break;
            }
            let at = ZIGZAG[z];
            block[at] = extend(bits.bits(size)?, size) * i32::from(quant[at]);
            z += 1;
        }
        if bx * 8 < c.width && by * 8 < c.height {
            let stride = c.width;
            idct(&block, &mut c.samples[by * 8 * stride + bx * 8..], stride);
        }
    }
    // Up to the next marker.
    let mut pos = bits.pos;
    while pos + 1 < data.len()
        && !(data[pos] == 0xff && data[pos + 1] != 0 && !(RST0..=RST7).contains(&data[pos + 1]))
    {
        pos += 1;
    }
    Some(pos)
}

/// YCbCr to RGB as libjpeg reckons it, in sixteen-bit fixed point
/// (`jdcolor.c`).
fn ycc_to_rgb(y: u8, cb: u8, cr: u8) -> [u8; 3] {
    const SCALE: i32 = 16;
    const HALF: i32 = 1 << (SCALE - 1);
    let fix = |x: f64| (x * f64::from(1 << SCALE) + 0.5) as i32;
    let (y, cb, cr) = (i32::from(y), i32::from(cb) - 128, i32::from(cr) - 128);
    let r = y + ((fix(1.40200) * cr + HALF) >> SCALE);
    let g = y + ((-fix(0.34414) * cb - fix(0.71414) * cr + HALF) >> SCALE);
    let b = y + ((fix(1.77200) * cb + HALF) >> SCALE);
    [r, g, b].map(|v| v.clamp(0, 255) as u8)
}

#[cfg(test)]
mod tests {
    use image::codecs::jpeg::JpegEncoder;

    use super::*;

    #[test]
    fn a_baseline_picture_decodes_as_another_decoder_has_it() {
        // Grey steps: no colour edges, so smoothing the colour planes or
        // not makes no difference.
        let picture = RgbImage::from_fn(40, 24, |x, y| {
            let v = ((x / 8) * 40 + (y / 8) * 10) as u8;
            Rgb([v, v, v])
        });
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, 90).encode_image(&picture).unwrap();
        let ours = decode(&bytes).unwrap();
        let theirs = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(ours.dimensions(), theirs.dimensions());
        let worst = ours.as_raw().iter().zip(theirs.as_raw()).map(|(a, b)| a.abs_diff(*b)).max();
        assert!(worst.unwrap() <= 2, "{worst:?}");
    }

    #[test]
    fn only_a_jpeg_is_taken() {
        assert!(decode(b"GIF89a").is_none());
    }
}
