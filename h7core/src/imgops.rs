//! The image-program helper library (0x40d4b1.., ARGB planes of 256x256 dwords), ported 1:1 and verified against the original (re/helper_vectors.txt).
use crate::mmx::*;

/// 0x40d4b1: bilinear sample of an ARGB plane. Position = (bl, bh) = (x, y) in 0..255 with 8-bit wrap; al/ah = the x/y offsets of the second taps; `fx`/`fy` = the 15-bit fractions (ecx/edx).
pub fn sample(plane: &[u32], ebx: u16, eax: u16, fx: u16, fy: u16) -> u32 {
    let (x, y) = ((ebx & 0xff) as u8, (ebx >> 8) as u8); let (dx, dy) = ((eax & 0xff) as u8, (eax >> 8) as u8);
    let at = |x: u8, y: u8| plane[((y as usize) << 8) | x as usize] as u64;
    let (x1, y1) = (x.wrapping_add(dx), y.wrapping_add(dy));
    let (mut m0, mut m1, mut m3, mut m2) = (at(x, y), at(x1, y), at(x1, y1), at(x, y1));
    let m5 = pk([fx; 4]); let m6 = pk([fy; 4]);
    m0 = unpcklbw(m0); m1 = unpcklbw(m1); m2 = unpcklbw(m2); m3 = unpcklbw(m3);
    m0 = psllw(m0, 4); m1 = psllw(m1, 4); m2 = psllw(m2, 4); m3 = psllw(m3, 4);
    m1 = psubw(m1, m0); m3 = psubw(m3, m2); m1 = pmulhw(m1, m5); m3 = pmulhw(m3, m5);
    m1 = psllw(m1, 4); m0 = paddw(m0, m1); m3 = psllw(m3, 4); m2 = paddw(m2, m3);
    m2 = psubw(m2, m0); m2 = pmulhw(m2, m6); m0 = psraw(m0, 4); m0 = paddw(m0, m2);
    packuswb_lo(m0) as u32
}

/// 0x40d536: RGB -> HSV, branch-free (sbb/and min and max). Input bytes b0 | b1<<8 | b2<<16, result H | S<<8 | V<<16 (V = max, S = 255*(max-min)/max, H = ((255*delta)/range + 0x200*k) * 0x2aaa >> 16 ... exactly as the original).
pub fn rgb2hsv(x: u32) -> u32 {
    let (b0, b1, b2) = ((x & 0xff) as i32, ((x >> 8) & 0xff) as i32, ((x >> 16) & 0xff) as i32);
    let mn = b0.min(b1).min(b2); let mx = b0.max(b1).max(b2);
    if mx == 0 { return 0; }
    let v = (mx as u32) << 16;
    let d = mx - mn; if d == 0 { return v; }
    let s = ((d * 255) as u32 / mx as u32) & 0xff;
    let (a, mut b, c) = (mx - b0, mx - b1, mx - b2);
    let (num, off);
    if mx == b0 { num = c - b; off = 0; } else if mx == b1 { num = a - c; off = 0x200; } else { b -= a; num = b; off = 0x400; }
    let q = (num * 255) / d;                                  // imul/idiv (signed, truncating)
    let h = (((q + off) as i16 as i32 * 0x2aaa) >> 16) as u8 as u32;
    v | (s << 8) | h
}
const T_P: [u32; 6] = [0x10000, 0x10000, 0x1, 0x1, 0x100, 0x100];
const T_Q: [u32; 6] = [0x0, 0x1, 0x0, 0x100, 0x0, 0x10000];
const T_T: [u32; 6] = [0x100, 0x0, 0x10000, 0x0, 0x1, 0x0];
const T_V: [u32; 6] = [0x1, 0x100, 0x100, 0x10000, 0x10000, 0x1];
/// 0x40d604: HSV -> RGB with the six-sextant table method: sector = (H*0x06060606) >> 32, f = low dword >> 24, p = (255-S)V>>8, q = (255-(S f>>8))V>>8, t = (255-(S (255-f)>>8))V>>8;
/// the four channel multipliers per sector come from the tables at 0x42a23c.. (which of p, q, t, V goes to which byte).
pub fn hsv2rgb(x: u32) -> u32 {
    let (h, s, v) = ((x & 0xff) as u64, ((x >> 8) & 0xff) as u32, ((x >> 16) & 0xff) as u32);
    let m = h * 0x0606_0606; let sector = (m >> 32) as usize; let f = ((m as u32) >> 24) & 0xff;
    let p = (((255 - s) * v) >> 8) & 0xff;
    let q = (((255 - ((s * f) >> 8) as u32 & 0xff) * v) >> 8) & 0xff;
    let t = (((255 - ((s * (255 - f)) >> 8) as u32 & 0xff) * v) >> 8) & 0xff;
    let sec = sector.min(5);
    (p.wrapping_mul(T_P[sec])) | (q.wrapping_mul(T_Q[sec])) | (t.wrapping_mul(T_T[sec])) | (v.wrapping_mul(T_V[sec]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn sample_matches_the_original() {
        let raw = include_bytes!("../../re/plane.bin"); let plane: Vec<u32> = raw.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let mut n = 0;
        for line in include_str!("../../re/helper_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); if f[0] != "sample" { continue; }
            let p = |i: usize| -> u64 { f[i].parse().unwrap() };
            let got = sample(&plane, p(1) as u16, p(2) as u16, p(3) as u16, p(4) as u16);
            assert_eq!(got as u64, p(5), "sample vector {n}"); n += 1;
        }
        assert_eq!(n, 300);
    }

    #[test] fn hsv_conversions_match_the_original() {
        let (mut a, mut b) = (0, 0);
        for line in include_str!("../../re/helper_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); let v: u32 = f.get(1).and_then(|x| x.parse().ok()).unwrap_or(0);
            match f[0] { "rgb2hsv" => { assert_eq!(rgb2hsv(v), f[2].parse::<u32>().unwrap(), "rgb2hsv {v:#x}"); a += 1; }
                         "hsv2rgb" => { assert_eq!(hsv2rgb(v), f[2].parse::<u32>().unwrap(), "hsv2rgb {v:#x}"); b += 1; } _ => {} }
        }
        assert_eq!((a, b), (400, 400));
    }
}
