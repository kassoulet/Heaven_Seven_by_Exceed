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

// ---- the blend-mode library (0x40d7b0..0x40d8bf): each takes (a = eax, b = ebx) as packed ARGB bytes and returns the blended value ----
fn bytes4(x: u32) -> [u32; 4] { [x & 0xff, (x >> 8) & 0xff, (x >> 16) & 0xff, x >> 24] }
fn from4(b: [u32; 4]) -> u32 { b[0] | b[1] << 8 | b[2] << 16 | b[3] << 24 }
/// 0x40d7b0 add: paddusb
pub fn blend_add(a: u32, b: u32) -> u32 { let (x, y) = (bytes4(a), bytes4(b)); from4([0, 1, 2, 3].map(|i| (x[i] + y[i]).min(255))) }
/// 0x40d7bd subtract: psubusb (b - a, clamped at 0)
pub fn blend_sub(a: u32, b: u32) -> u32 { let (x, y) = (bytes4(a), bytes4(b)); from4([0, 1, 2, 3].map(|i| y[i].saturating_sub(x[i]))) }
/// 0x40d7ca difference: |a - b| per byte (signed-compare max/min after a -0x80 bias)
pub fn blend_diff(a: u32, b: u32) -> u32 { let (x, y) = (bytes4(a), bytes4(b)); from4([0, 1, 2, 3].map(|i| (x[i].max(y[i]) - x[i].min(y[i])) & 0xff)) }
/// 0x40d811: (255-a) * b >> 8 on the three colour bytes (alpha byte: a's alpha * b's alpha >> 8 as well? the xor leaves byte 3 of a unchanged)
pub fn blend_mulinv(a: u32, b: u32) -> u32 { let a = a ^ 0x00ff_ffff; let (x, y) = (bytes4(a), bytes4(b)); from4([0, 1, 2, 3].map(|i| ((x[i] * y[i]) >> 8).min(255))) }
/// 0x40d833 screen: ~((~a * ~b) >> 8) on the colour bytes
pub fn blend_screen(a: u32, b: u32) -> u32 { let (a2, b2) = (a ^ 0x00ff_ffff, b ^ 0x00ff_ffff); let (x, y) = (bytes4(a2), bytes4(b2)); from4([0, 1, 2, 3].map(|i| ((x[i] * y[i]) >> 8).min(255))) ^ 0x00ff_ffff }
/// 0x40d866 multiply: a * b >> 8 per byte
pub fn blend_mul(a: u32, b: u32) -> u32 { let (x, y) = (bytes4(a), bytes4(b)); from4([0, 1, 2, 3].map(|i| ((x[i] * y[i]) >> 8).min(255))) }
/// 0x40d883 overlay: with t = 2a (16-bit): m = 0xff where a >= 128; out = (((t & 0xff) ^ m) * (b ^ m) >> 8) ^ m, packuswb
pub fn blend_overlay(a: u32, b: u32) -> u32 {
    let (x, y) = (bytes4(a), bytes4(b));
    from4([0, 1, 2, 3].map(|i| { let t = (x[i] * 2) & 0xffff; let m = (t >> 8) * 0xff & 0xffff; let p = (((t & 0xff) ^ m) * (y[i] ^ m)) & 0xffff; (((p >> 8) ^ m) & 0xffff).min(255) }))
}

// ---- plane-level operations ----
pub struct Xs(pub u32);
impl Xs { pub fn next(&mut self) -> u32 { let mut x = self.0; x ^= x << 13; x ^= x >> 17; x ^= x << 5; self.0 = x; x } }
pub fn fnv1a(b: &[u8]) -> u64 { let mut h: u64 = 0xcbf29ce484222325; for &c in b { h = (h ^ c as u64).wrapping_mul(0x100000001b3); } h }

/// 0x40d684 auto-levels of a 256x256 ARGB plane, per colour channel: out = ((v - min) * (0x7fff / (max - min))) >> 7 with 16-bit lane arithmetic (the alpha lane's multiplier is 0: alpha becomes 0).
pub fn auto_levels(src: &[u32]) -> Vec<u32> {
    let mut mx = [0i16; 4]; let mut mn = [0x7fffi16; 4];
    for &p in src { for c in 0..4 { let v = ((p >> (8 * c)) & 0xff) as i16; mx[c] = if v > mx[c] { v } else if mx[c] > v { mx[c] } else { 0 }; /* pcmpgtw masks: a tie gives 0 (original quirk) */ if v <= mn[c] { mn[c] = v; } } }
    let range: [u32; 4] = [0, 1, 2, 3].map(|c| (mx[c] - mn[c]).clamp(0, 255) as u32);   // psubw + packuswb (unsigned saturation)
    let recip: [u16; 4] = [0, 1, 2].map(|c| if range[c] == 0 { 0x7fff } else { (0x7fff / range[c]) as u16 }).into_iter().chain([0]).collect::<Vec<_>>().try_into().unwrap();
    src.iter().map(|&p| { let mut o = 0u32; for c in 0..4 { let v = (((p >> (8 * c)) & 0xff) as i16).wrapping_sub(mn[c]) as u16; let r = (v.wrapping_mul(recip[c]) >> 7).min(255); o |= (r as u32) << (8 * c); } o }).collect()
}
pub const COMBINE_MASKS: [u32; 8] = [0x0, 0xff, 0xff00, 0xffff, 0xff0000, 0xff00ff, 0xffff00, 0xffffff];
pub fn blend_by_code(n: u32) -> fn(u32, u32) -> u32 { [|a, _| a, blend_add, blend_sub, blend_diff, blend_mulinv, blend_screen, blend_mul, blend_overlay][(n & 7) as usize] }
/// 0x40d758 + 0x40d77a: combine `src` into `dst` with one stream byte: low nibble = channel mask (table 0x42a374), high nibble = blend (table 0x42a394);
/// out = (blend(src, dst) & mask) | (dst & ~mask & 0xffffff)   (alpha is dropped)
pub fn combine(src: &[u32], dst: &mut [u32], code: u8) {
    let mask = COMBINE_MASKS[(code & 0xf) as usize & 7]; let f = blend_by_code((code >> 4) as u32);
    for i in 0..dst.len() { let (a, b) = (src[i], dst[i]); dst[i] = (f(a, b) & mask) | (b & !mask & 0x00ff_ffff); }
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

    #[test] fn blends_match_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/helper_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); if !f[0].starts_with("blend_") { continue; }
            let p = |i: usize| -> u32 { f[i].parse().unwrap() }; let (a, b, want) = (p(1), p(2), p(3));
            let got = match f[0] { "blend_add" => blend_add(a, b), "blend_sub" => blend_sub(a, b), "blend_diff" => blend_diff(a, b), "blend_mulinv" => blend_mulinv(a, b),
                "blend_screen" => blend_screen(a, b), "blend_mul" => blend_mul(a, b), "blend_overlay" => blend_overlay(a, b), x => panic!("{x}") };
            assert_eq!(got, want, "{} {a:#x} {b:#x}", f[0]); n += 1;
        }
        assert_eq!(n, 1050);
    }

    fn plane(seed: u32, lo: [u32; 3], span: [u32; 3]) -> Vec<u32> {
        let mut g = Xs(seed); (0..65536).map(|_| { let x = g.next(); (lo[0] + (x & 0xff) % span[0]) | (lo[1] + ((x >> 8) & 0xff) % span[1]) << 8 | (lo[2] + ((x >> 16) & 0xff) % span[2]) << 16 | ((x >> 24) & 0xff) << 24 }).collect()
    }
    fn bytes(p: &[u32]) -> Vec<u8> { p.iter().flat_map(|v| v.to_le_bytes()).collect() }
    #[test] fn plane_ops_match_the_original() {
        let (mut a, mut c) = (0, 0);
        for line in include_str!("../../re/plane_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); let p = |i: usize| -> u64 { f[i].parse().unwrap() };
            match f[0] {
                "autolevels" => { let s = plane(p(1) as u32, [p(2) as u32, p(3) as u32, p(4) as u32], [p(5) as u32, p(6) as u32, p(7) as u32]); assert_eq!(fnv1a(&bytes(&auto_levels(&s))), p(8), "autolevels {line}"); a += 1; }
                "combine" => { let seed = p(1) as u32; let s = plane(seed, [0; 3], [256; 3]); let mut d = plane(seed + 1, [0; 3], [256; 3]); combine(&s, &mut d, p(2) as u8); assert_eq!(fnv1a(&bytes(&d)), p(3), "combine {line}"); c += 1; }
                _ => {} }
        }
        assert_eq!((a, c), (5, 40));
    }
}
