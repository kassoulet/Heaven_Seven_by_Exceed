//! Simple operator handlers of the 0x408xxx page, ported 1:1 (verified against the original, re/handler_vectors.txt).
//! An operator instance is a byte block `st` (the original's edx): +0x00/+0x04 counts, +0x20/+0x24 timing words, +0x28.. value slots (f32/u32), +0x2c.. the working values.
//! `out` is the destination (the original's ebx), `d`/`p` the script stream (edi).
use crate::pack::{read_float, read_int};

fn u32_at(b: &[u8], o: usize) -> u32 { u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) }
fn f32_at(b: &[u8], o: usize) -> f32 { f32::from_bits(u32_at(b, o)) }
fn put_u32(b: &mut [u8], o: usize, v: u32) { b[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
/// x87 fistp dword with the default control word: round to nearest even, out of range / NaN -> 0x80000000 (the "integer indefinite").
fn fist32(x: f32) -> i32 { if x.is_finite() && x.abs() < 2147483648.0 { x.round_ties_even() as i32 } else { i32::MIN } }
fn fist16(x: f32) -> i16 { if x.is_finite() && x.abs() < 32768.0 { x.round_ties_even() as i16 } else { i16::MIN } }

/// 0x408040: out[0] = 1 - bit6 of (t1 - t0)  ("is the interval shorter than 64 ticks")
pub fn cmp40(st: &[u8], out: &mut [u8]) { let d = u32_at(st, 0x24).wrapping_sub(u32_at(st, 0x20)); out[0] = 1 - ((d >> 6) & 1) as u8; }
/// 0x408051: out = (int) value
pub fn f2i51(st: &[u8], out: &mut [u8]) { put_u32(out, 0, fist32(f32_at(st, 0x2c)) as u32); }
/// 0x408057: value = next stream byte
pub fn byte2f57(st: &mut [u8], d: &[u8], p: &mut usize) { let b = d[*p]; *p += 1; put_u32(st, 0x2c, (b as f32).to_bits()); }
/// 0x408064: out[0] = low byte of (int) value
pub fn f2b64(st: &[u8], out: &mut [u8]) { out[0] = fist32(f32_at(st, 0x2c)) as u8; }
/// 0x40806f: value = packed integer from the stream
pub fn int2f6f(st: &mut [u8], d: &[u8], p: &mut usize) { let v = read_int(d, p); put_u32(st, 0x2c, (v as f32).to_bits()); }
/// 0x40807d: four values (+0x2c..+0x38) -> four clamped 0..255 words (colour), out[0..8]
pub fn quad4f7d(st: &[u8], out: &mut [u8]) {
    for i in 0..4 {
        let w = fist16(f32_at(st, 0x2c + 4 * i));
        let w = if w < 0 { 0 } else if w > 0xff { 0xff } else { w };
        out[2 * i..2 * i + 2].copy_from_slice(&(w as u16).to_le_bytes());
    }
}
/// 0x4081b0: copy the n=[+4] value slots (+0x2c..) to out
pub fn copy1b0(st: &[u8], out: &mut [u8]) { let n = u32_at(st, 4) as usize; for i in 1..=n { let v = u32_at(st, 0x28 + 4 * i); put_u32(out, 4 * (i - 1), v); } }
/// 0x4081c1: fill the n=[+0] value slots with packed floats from the stream, *last slot first*
pub fn fill1c1(st: &mut [u8], d: &[u8], p: &mut usize) { let n = u32_at(st, 0) as usize; for i in (1..=n).rev() { let v = read_float(d, p); put_u32(st, 0x28 + 4 * i, v.to_bits()); } }

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
    #[test] fn matches_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/handler_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let (mut st, sm, want_st, want_out) = (hex(f[1]), hex(f[2]), hex(f[3]), hex(f[4]));
            let want_used: usize = f[5].parse().unwrap();
            let mut out = vec![0xaau8; 16]; let mut p = 0usize;
            match f[0] {
                "cmp40" => cmp40(&st, &mut out), "f2i51" => f2i51(&st, &mut out), "byte2f57" => byte2f57(&mut st, &sm, &mut p),
                "f2b64" => f2b64(&st, &mut out), "int2f6f" => int2f6f(&mut st, &sm, &mut p), "quad4f7d" => quad4f7d(&st, &mut out),
                "copy1b0" => copy1b0(&st, &mut out), "fill1c1" => fill1c1(&mut st, &sm, &mut p), x => panic!("{x}") }
            assert_eq!(st, want_st, "state {line}"); assert_eq!(out, want_out, "out {line}"); assert_eq!(p, want_used, "used {line}"); n += 1;
        }
        assert_eq!(n, 1400);
    }
}
