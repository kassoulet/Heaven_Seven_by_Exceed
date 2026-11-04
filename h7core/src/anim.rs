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

/// A tiny x87 register stack (single precision, see the header): index 0 is ST(0). Only what the evaluator needs, one method per instruction form.
struct X(Vec<f32>);
impl X {
    fn new() -> X { X(Vec::new()) }
    fn push(&mut self, v: f32) { self.0.push(v) }
    fn pop(&mut self) -> f32 { self.0.pop().unwrap() }
    fn st(&self, i: usize) -> f32 { self.0[self.0.len() - 1 - i] }
    fn set(&mut self, i: usize, v: f32) { let n = self.0.len(); self.0[n - 1 - i] = v }
    fn fxch(&mut self, i: usize) { let (a, b) = (self.st(0), self.st(i)); self.set(0, b); self.set(i, a) }
    fn fld_st(&mut self, i: usize) { let v = self.st(i); self.push(v) }
}

/// The animation channel evaluator 0x407e05 for pre-decoded keys (the original decodes keys lazily from the script stream through [edx+0x14]; here `m` holds all key blocks
/// starting with a pad block at `first`, as the original's pool does). `cur` = offset of the current segment's key (0 = none), `coef` = the cached cubic coefficients.
pub struct Channel { pub n: usize, pub len: u32, pub first: usize, pub cur: usize, pub coef: [f32; 16] }

impl Channel {
    pub fn eval(&mut self, m: &[u8], t: i32) -> [f32; 4] {
        let mut t = if t > 0 { t } else { 1 };
        if t >= self.len as i32 { t = (t as u32 % self.len) as i32; }
        let ti = |o: usize| u32_at(m, o) as i32;
        let mut esi = self.cur;
        let mut advance = false;
        if esi == 0 || t <= ti(esi) { esi = self.first; advance = true; }
        else if t >= ti(esi + 0x40) { advance = true; }
        if advance {
            loop {
                esi += 0x40; self.cur = esi;
                if t < ti(esi + 0x40) { break; }
            }
            if self.n != 0 { self.build_coefficients(m, esi); }
        }
        let mut res = [0f32; 4];
        if self.n == 0 { return res; }
        let u = if t - ti(esi) < 0 { 0 } else { t - ti(esi) };
        let dt = ti(esi + 0x40) - ti(esi);
        let mut x = X::new();
        x.push(if dt > 0 { u as f32 / dt as f32 } else { 1.0 });                 // s
        x.push(f32_at(m, esi + 0x24)); x.push(f32_at(m, esi + 0x60));           // a, b
        x.fld_st(0); { let v = x.st(0) + x.st(2); x.set(0, v) }                   // a+b
        let ab = x.st(0);
        if (ab.to_bits() << 1) == 0 { x.pop(); x.pop(); x.pop(); }               // 0x407fa5: linear
        else {
            x.push(1.0);
            if (ab.to_bits() << 1) > 0x7f000000 {
                let v = x.st(0) / x.st(2); x.set(2, v);                           // fdivr st2,st0
                x.fxch(1);
                let v = x.st(2) * x.st(0); x.set(2, v);                           // fmul st2,st0
                let v = x.st(3) * x.st(0); x.set(3, v); x.pop();                  // fmulp st3
                x.fxch(3);
            } else {
                x.fld_st(0); { let v = x.st(0) + x.st(0); x.set(0, v) }          // 2
                let v = x.st(0) - x.st(2); x.set(2, v); x.pop();                  // fsubrp st2
                let v = x.st(0) / x.st(1); x.set(1, v); x.pop();                  // fdivrp st1
                x.fxch(3);
            }
            let s_mem = x.st(0);                                                  // fst [esp]
            x.fxch(2);
            let st4 = x.st(0);                                                    // fst [esp+4]
            if (s_mem.to_bits() as i32) < (st4.to_bits() as i32) {
                let v = x.st(3) / x.st(0); x.set(3, v); x.pop();                  // fdivp st3
                x.pop();                                                          // fstp st0
                let v = x.st(0) * x.st(0); x.set(0, v);                           // fmul st0
                let v = x.st(1) * x.st(0); x.set(1, v); x.pop();                  // fmulp st1
            } else {
                x.push(1.0);
                let v = x.st(0) - x.st(3); x.set(3, v); x.pop();                  // fsubrp st3
                x.fxch(1);
                let b_mem = x.st(0);                                              // fst [esp+8]
                x.fxch(2);
                let s_mem2 = x.st(0);                                             // fst [esp+4]
                if (b_mem.to_bits() as i32) < (s_mem2.to_bits() as i32) {
                    x.push(s_mem); { let v = x.st(0) + s_mem; x.set(0, v) }       // fld [esp]; fadd [esp]
                    x.fxch(3); x.pop(); x.pop();
                    let v = x.st(1) - x.st(0); x.set(1, v); x.pop();              // fsubp st1
                    let v = x.st(1) * x.st(0); x.set(1, v); x.pop();              // fmulp st1
                } else {
                    { let v = x.st(0) * x.st(0); x.set(0, v) }                    // fmul st0
                    x.fxch(2);
                    let v = x.st(3) / x.st(0); x.set(3, v); x.pop();              // fdivp st3
                    x.pop();                                                      // fstp st0
                    let v = x.st(1) * x.st(0); x.set(1, v); x.pop();              // fmulp st1
                    x.push(1.0);
                    let v = x.st(0) - x.st(1); x.set(1, v); x.pop();              // fsubrp st1
                }
            }
        }
        let y = x.st(0);
        for k in 0..self.n { let c = &self.coef[4 * k..4 * k + 4]; res[k] = (((c[3] * y + c[2]) * y + c[1]) * y) + c[0]; }
        res
    }

    fn build_coefficients(&mut self, m: &[u8], esi: usize) {
        let n = self.n;
        let (mut o1, mut o2) = ([0f32; 8], [0f32; 8]);
        tangent(m, esi, n, 1, &mut o1);
        tangent(m, esi + 0x40, n, 0, &mut o2);
        for k in 1..=n {
            let p0 = f32_at(m, esi + 4 * k); let d = f32_at(m, esi + 0x40 + 4 * k) - p0;
            let (t0, t1) = (o1[k], o2[k]);
            let d3 = d * 3.0; let t02 = t0 + t0; let d2 = d + d;
            let a1 = d3 - t02; let b1 = t0 - d2;
            self.coef[4 * (k - 1)] = p0; self.coef[4 * (k - 1) + 1] = t0;
            self.coef[4 * (k - 1) + 2] = a1 - t1; self.coef[4 * (k - 1) + 3] = b1 + t1;
        }
    }
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

    #[test] fn eval_matches_the_original() {
        let mut cnt = 0;
        for line in include_str!("../../re/eval_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let (n, len, t, cur): (usize, u32, i32, usize) = (f[0].parse().unwrap(), f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap());
            let keys = hex(f[4]); let want = hex(f[5]); let want_cur: usize = f[6].parse().unwrap(); let want_coef = hex(f[7]);
            let mut ch = Channel { n, len, first: 0, cur: cur * 0x40, coef: [0.0; 16] };
            let r = ch.eval(&keys, t);
            let got: Vec<u8> = r[..n].iter().flat_map(|v| v.to_bits().to_le_bytes()).collect();
            let gc: Vec<u8> = ch.coef[..4 * n].iter().flat_map(|v| v.to_bits().to_le_bytes()).collect();
            assert_eq!(got, want, "value {line}"); assert_eq!(ch.cur / 0x40, want_cur, "cur {line}"); assert_eq!(gc, want_coef, "coef {line}"); cnt += 1;
        }
        assert_eq!(cnt, 500);
    }
}
