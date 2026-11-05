//! The image-program interpreter (0x40d43f), ported: five 256x256 ARGB planes (plane 0 = the work plane), a byte program, handler per code. Verified with whole-program vectors (re/prog_vectors.txt: the original run in Unicorn, output plane hashed).
use crate::imgops::*;

pub const N: usize = 65536;
pub struct Vm { pub planes: Vec<Vec<u32>>, pub seed: u32, pub mask: u32, pub blend: fn(u32, u32) -> u32, carry: bool }

impl Vm {
    pub fn new() -> Vm { Vm { planes: vec![vec![0u32; N]; 5], seed: 0, mask: 0, blend: |a, _| a, carry: false } }
    /// 0x40d758: select the combine mode from one byte
    fn select(&mut self, code: u8) { self.mask = COMBINE_MASKS[(code & 0xf) as usize & 7]; self.blend = blend_by_code((code >> 4) as u32); }
    /// 0x40d77a: combine the work plane (0) into plane n with the selected mask/blend
    fn combine_into(&mut self, n: usize) {
        let (mask, f) = (self.mask, self.blend); let src = self.planes[0].clone();
        for i in 0..N { let (a, b) = (src[i], self.planes[n][i]); self.planes[n][i] = (f(a, b) & mask) | (b & !mask & 0x00ff_ffff); }
    }
    /// 0x40d498: ecx = ecx*0xfacedead; rcr ecx,3 (the carry in is the imul's CF = signed overflow); xchg cl,ch; inc ecx
    pub fn rnd(&mut self) -> u32 {
        let prod = (self.seed as i32 as i64) * (0xfaceDEADu32 as i32 as i64); let v = prod as i32 as u32; let cf = (prod != v as i32 as i64) as u64;
        let v33 = (cf << 32) | v as u64; let rot = ((v33 >> 3) | (v33 << 30)) & ((1u64 << 33) - 1);
        let mut e = rot as u32; e = (e & 0xffff_0000) | ((e & 0xff) << 8) | ((e >> 8) & 0xff); e = e.wrapping_add(1); self.seed = e; e
    }

    /// 0x40d9df: noise write: eax += ((ecx>>16) * amp) >> 17, then >>2, clamp 0..255, grey pixel
    fn put(&mut self, idx: usize, eax: u32, ecx: i32, amp: i32) {
        let mut c = ecx >> 16; c = c.wrapping_mul(amp); c >>= 17; let mut e = (eax as i32).wrapping_add(c); e >>= 2;
        if e < 0 { e = 0; } if e as u32 >= 0xff { e = 0xff; }
        self.planes[0][idx] = (e as u32).wrapping_mul(0x10101);
    }
    /// code 0x11 (0x40d8f7): diamond-square "plasma" on the work plane, combined into plane n. Args: n, blend code, seed (dword), amplitude (dword, halved every level).
    fn plasma(&mut self, a: &[u8]) -> Option<()> {
        let n = a[0] as usize; self.select(a[1]); self.seed = u32::from_le_bytes([a[2], a[3], a[4], a[5]]); let mut amp = i32::from_le_bytes([a[6], a[7], a[8], a[9]]);
        let r0 = self.rnd(); self.planes[0][0] = (r0 & 0xff).wrapping_mul(0x10101);
        let (mut bl, mut bh, mut dl, mut dh) = (0u8, 0u8, 0x80u8, 0u8);
        let px = |s: &Vm, bl: u8, bh: u8| (s.planes[0][((bh as usize) << 8) | bl as usize] & 0xff) as u32;
        loop {
            // phase 1 (squares)
            loop {
                let mut e = px(self, bl, bh);
                bl = bl.wrapping_add(dh); e += px(self, bl, bh); bh = bh.wrapping_add(dh); e += px(self, bl, bh); bl = bl.wrapping_sub(dh); e += px(self, bl, bh);
                bh = bh.wrapping_sub(dl); bl = bl.wrapping_add(dl);
                let r = self.rnd() as i32; self.put(((bh as usize) << 8) | bl as usize, e, r.wrapping_mul(3), amp);
                bh = bh.wrapping_sub(dl); bl = bl.wrapping_add(dl);
                if bl != 0 { continue; }
                bh = bh.wrapping_add(dh);
                if bh != 0 { continue; }
                break;
            }
            // phase 2 (diamonds)
            loop {
                let mut e = px(self, bl, bh);
                bh = bh.wrapping_add(dh); e += px(self, bl, bh);
                bh = bh.wrapping_sub(dl); bl = bl.wrapping_add(dl); e += px(self, bl, bh);
                bl = bl.wrapping_sub(dh); e += px(self, bl, bh);
                bl = bl.wrapping_add(dl);
                let r = self.rnd() as i32; self.put(((bh as usize) << 8) | bl as usize, e, r.wrapping_mul(2), amp);
                bh = bh.wrapping_sub(dl);
                let mut e = px(self, bl, bh);
                bl = bl.wrapping_add(dh); e += px(self, bl, bh);
                bl = bl.wrapping_sub(dl); bh = bh.wrapping_add(dl); e += px(self, bl, bh);
                bh = bh.wrapping_sub(dh); e += px(self, bl, bh);
                bh = bh.wrapping_add(dl);
                let r = self.rnd() as i32; self.put(((bh as usize) << 8) | bl as usize, e, r.wrapping_add(r), amp);
                bl = bl.wrapping_add(dl); if bl != 0 { continue; }
                bh = bh.wrapping_add(dh); if bh != 0 { continue; }
                break;
            }
            amp >>= 1; dh = dl; dl >>= 1; if dl == 0 { break; }
        }
        self.combine_into(n); Some(())
    }

    /// code 0x12 (0x40da07): value noise. Args: n, blend code, seed (dword), k. A grid with spacing 1<<k gets random low bytes, then every pixel is the bilinear sample (0x40d4b1, fractions
    /// advancing by 0x1000>>k per pixel) between the four surrounding grid points - computed in place on the work plane, row-major, exactly in the original's loop order.
    fn noise(&mut self, a: &[u8]) {
        let n = a[0] as usize; self.select(a[1]); self.seed = u32::from_le_bytes([a[2], a[3], a[4], a[5]]); let k = a[6] as u32;
        let dl = (1u32 << k) as u8; let step = (0x1000u32 >> k) as u16;
        let (mut bl, mut bh) = (0u8, 0u8);
        loop { let r = self.rnd(); let i = ((bh as usize) << 8) | bl as usize; self.planes[0][i] = (self.planes[0][i] & !0xff) | (r & 0xff);
               bl = bl.wrapping_add(dl); if bl != 0 { continue; } bh = bh.wrapping_add(dl); if bh != 0 { continue; } break; }
        let mut out = 0usize; let ea = (dl as u16) | (dl as u16) << 8;
        let mut cy = 0u8;
        loop {                                                            // cell rows (bh)
            let mut fy = 0u16;
            for _ in 0..dl {                                              // scanlines inside the cell row
                let mut cx = 0u8;
                loop {                                                    // cells along the row (bl)
                    let mut fx = 0u16;
                    for _ in 0..dl { let v = sample(&self.planes[0], ((cy as u16) << 8) | cx as u16, ea, fx, fy) & 0xff; self.planes[0][out] = v * 0x10101; out = (out + 1) & 0xffff; fx += step; }
                    cx = cx.wrapping_add(dl); if cx == 0 { break; }
                }
                fy += step;
            }
            cy = cy.wrapping_add(dl); if cy == 0 { break; }
        }
        self.combine_into(n);
    }

    /// code 0x13 (0x40daac): sine plasma. Args: n, blend code, a, b, c, d: v = sin(pi*(x/256*2a/... ) ...) - exactly: s1 = sin(pi * ((x*(1/256))*(2a) + b/128)), s2 = sin(pi * ((y*(1/256))*(2c) + d/128)),
    /// pixel = (round(s1*s2*255) + 0x100) >> 1 (grey). x87 fsin in single-precision mode = the correctly rounded f32 of sin(f64) here.
    fn sines(&mut self, a: &[u8]) {
        let n = a[0] as usize; self.select(a[1]);
        let a2 = a[2] as f32 + a[2] as f32; let b = a[3] as f32 * 0.0078125f32; let c2 = a[4] as f32 + a[4] as f32; let d = a[5] as f32 * 0.0078125f32;
        let sin32 = |t: f32| -> f32 { ((std::f64::consts::PI * t as f64) as f32 as f64).sin() as f32 };
        for i in 0..N {
            let (x, y) = ((i & 0xff) as f32, ((i >> 8) & 0xff) as f32);
            let s1 = sin32(((x * 0.00390625f32) * a2) + b); let s2 = sin32(((y * 0.00390625f32) * c2) + d);
            let v = ((s1 * s2) * 255.0f32).round_ties_even() as i32; let e = ((v + 0x100) as u32) >> 1;
            self.planes[0][i] = e.wrapping_mul(0x10101);
        }
        self.combine_into(n);
    }

    /// code 0x14 (0x40db51): random dark blobs on white. Args: n, blend code, seed (dword), count, k. The work plane is set to white; for each blob a random centre (cl, ch) is drawn and every pixel
    /// darkens by (255 - min(255, (round(dist) * k) >> 4)) with dist the wrapped-around euclidean distance (x87 fsqrt/fistp), saturating at 0.
    fn blobs(&mut self, a: &[u8]) {
        let n = a[0] as usize; self.select(a[1]); self.seed = u32::from_le_bytes([a[2], a[3], a[4], a[5]]); let (count, k) = (a[6] as u32, a[7] as u32);
        self.planes[0].iter_mut().for_each(|x| *x = 0x00ff_ffff);
        for _ in 0..count {
            let c = self.rnd(); let (cx, cy) = ((c & 0xff) as u8, ((c >> 8) & 0xff) as u8);
            for i in 0..N {
                let (bl, bh) = ((i & 0xff) as u8, ((i >> 8) & 0xff) as u8);
                let dx = bl.wrapping_sub(cx) as i8 as i32 as f32; let dy = bh.wrapping_sub(cy) as i8 as i32 as f32;
                let d = ((dx * dx + dy * dy).sqrt()).round_ties_even() as i32 as u32;
                let mut e = d.wrapping_mul(k); if e >= 0xfff { e = 0xfff; } let e = ((e >> 4) as u8) ^ 0xff;
                let cur = (self.planes[0][i] & 0xff) as u8; let v = cur.saturating_sub(e) as u32;
                self.planes[0][i] = v * 0x10101;
            }
        }
        self.combine_into(n);
    }
    /// run a program; returns the output plane converted like 0x40d479 (bswap, shr 8) when a result code (1..4) is reached
    pub fn run(&mut self, prog: &[u8]) -> Vec<u32> {
        let mut p = 0;
        loop {
            let code = prog[p]; p += 1;
            match code {
                1..=4 => return self.planes[code as usize].iter().map(|&v| v.swap_bytes() >> 8).collect(),
                0x10 => { // constant fill of the work plane, combined into plane n: args n, blend code, value
                    let (n, bc, v) = (prog[p] as usize, prog[p + 1], prog[p + 2]); p += 3;
                    self.select(bc); let g = (v as u32) * 0x10101; self.planes[0].iter_mut().for_each(|x| *x = g); self.combine_into(n);
                }
                0x11 => self.plasma(&prog[p..p + 10]).map(|()| p += 10).unwrap_or(()),
                0x12 => { self.noise(&prog[p..p + 7]); p += 7; }
                0x13 => { self.sines(&prog[p..p + 6]); p += 6; }
                0x14 => { self.blobs(&prog[p..p + 8]); p += 8; }
                c => panic!("image-program code {c:#x} not ported yet"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
    #[test] fn programs_match_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/prog_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); let prog = hex(f[0]);
            if prog.iter().any(|&b| !matches!(b, 0x10 | 1..=4 | 0x00..=0xff)) { continue; }
            let mut vm = Vm::new(); let out = vm.run(&prog);
            let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_le_bytes()).collect();
            assert_eq!(fnv1a(&bytes), f[1].parse::<u64>().unwrap(), "program {}", f[0]); n += 1;
        }
        assert!(n >= 30);
    }
}
