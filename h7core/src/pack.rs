//! The operator script's packed data formats, ported 1:1 from the original (0x407fc0, 0x407fd0, 0x4080fd).
//! Everything the demo stores - texture-op parameters, camera/object values - goes through these three readers.

/// 0x407fc0: variable-length integer. Low bit 0: one byte, value = b>>1. Low bit 1: a 16-bit little-endian signed word, value = w>>1 (arithmetic).
pub fn read_int(d: &[u8], p: &mut usize) -> i32 {
    let b = d[*p]; *p += 1;
    if b & 1 == 0 { (b >> 1) as i32 }
    else { let w = i16::from_le_bytes([d[*p - 1], d[*p]]); *p += 1; (w as i32) >> 1 }
}

/// 0x407fd0: packed float, three forms selected by the first byte (the engine's own 16/24-bit float formats):
///  - bit0 set: 16-bit word w: e = (w>>10) + 0xe0 (6-bit field; the `and al,0x1f` keeps 5 bits of it in al and leaves the carry into ah, which lands on the sign bit),
///    IEEE exponent = (e & 0x1f) + 0x7a, sign = bit 15, mantissa = (w & 0x3fe) << 13 (9 bits)
///  - first byte 0: the value 0.0 (1 byte)
///  - else a 24-bit value dw (3 bytes, bit 0 clear): e = ((dw & 0xff0000) >> 18) + 0xe0, exponent/sign as above (sign = bit 23), mantissa = (dw & 0x3fffe) << 5 (17 bits)
pub fn read_float(d: &[u8], p: &mut usize) -> f32 {
    let (e, mant);
    if d[*p] & 1 != 0 {
        let w = u16::from_le_bytes([d[*p], d[*p + 1]]) as u32;
        e = (w >> 10) + 0xe0; mant = (w & 0x3fe) << 13;
        *p += 2;
    } else if d[*p] == 0 {
        *p += 1; return 0.0;
    } else {
        let dw = d[*p] as u32 | (d[*p + 1] as u32) << 8 | (d[*p + 2] as u32) << 16;
        e = ((dw & 0xff0000) >> 18) + 0xe0; mant = (dw & 0x3fffe) << 5;
        *p += 3;
    }
    let sign = (e >> 8) & 1;
    f32::from_bits(sign << 31 | (((e & 0x1f) + 0x7a) << 23) | mant)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// re/pack_vectors.txt: 600 cases produced by running the ORIGINAL code in Unicorn (re/pack_vectors.py): "fn bytes eax consumed"
    #[test] fn matches_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/pack_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let d: Vec<u8> = (0..f[1].len() / 2).map(|i| u8::from_str_radix(&f[1][2 * i..2 * i + 2], 16).unwrap()).collect();
            let want = u32::from_str_radix(f[2], 16).unwrap(); let used: usize = f[3].parse().unwrap();
            let mut p = 0;
            let got = match f[0] { "int" => read_int(&d, &mut p) as u32, _ => read_float(&d, &mut p).to_bits() };
            assert_eq!((got, p), (want, used), "{line}"); n += 1;
        }
        assert_eq!(n, 600);
    }
}
