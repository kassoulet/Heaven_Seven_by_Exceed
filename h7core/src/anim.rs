//! Animation channels (the engine's spline/timeline system). Keys are 0x40-byte blocks: +0 time (i32), +4.. component values (f32, 1-based), +0x28 tension, +0x2c bias, +0x30 continuity.
//! Everything runs with the x87 in single precision (fldcw 0x7f), so plain f32 arithmetic in the same operation order is bit-exact.

fn u32_at(b: &[u8], o: usize) -> u32 { u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) }
fn f32_at(b: &[u8], o: usize) -> f32 { f32::from_bits(u32_at(b, o)) }

/// 0x407d84: Kochanek-Bartels style tangent at key `cur` (byte offset into `m`, with the previous key at cur-0x40 and the next at cur+0x40).
/// `flag` is cx (0x407d84 swaps the sign of the bias and the neighbour order when it is zero: incoming vs outgoing tangent). Result: out[1..=n].
pub fn tangent(m: &[u8], cur: usize, n: usize, flag: u16, out: &mut [f32]) {
    let bias = f32_at(m, cur + 0x2c);
    let (t1, t2) = (u32_at(m, cur) as i32 as f32, u32_at(m, cur + 0x40) as i32 as f32);
    let dt = t2 - t1;
    let dtp = t1 - u32_at(m, cur - 0x40) as i32 as f32;
    let (r, w) = if flag == 0 { (dtp / (dt + dtp), bias) } else { (dt / (dtp + dt), -bias) };
    let x = (r - 0.5) * (1.0 - w.abs()) + 0.5;
    let c = f32_at(m, cur + 0x30);
    let w1 = (1.0 - w) * (1.0 + c);
    let w2 = (1.0 + w) * (c - 1.0);
    let w3 = (1.0 - f32_at(m, cur + 0x28)) * x;
    let mut tmp = [0f32; 8];
    for i in 1..=n {
        let a = f32_at(m, cur + 4 * i) - f32_at(m, cur - 0x40 + 4 * i);
        out[i] = a * w1;
        tmp[i] = (f32_at(m, cur + 0x40 + 4 * i) - f32_at(m, cur + 4 * i)) * w2;
    }
    for i in 1..=n { out[i] = (out[i] - tmp[i]) * w3; }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
    #[test] fn tangent_matches_the_original() {
        let mut cnt = 0;
        for line in include_str!("../../re/tangent_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let (n, cx): (usize, u16) = (f[0].parse().unwrap(), f[1].parse().unwrap());
            let keys = hex(f[2]); let want = hex(f[3]);
            let mut out = [0f32; 8]; tangent(&keys, 0x40, n, cx, &mut out);
            let got: Vec<u8> = out[1..=n].iter().flat_map(|v| v.to_bits().to_le_bytes()).collect();
            assert_eq!(got, want, "{line}"); cnt += 1;
        }
        assert_eq!(cnt, 400);
    }
}
