//! The image-program interpreter (0x40d43f), ported: five 256x256 ARGB planes (plane 0 = the work plane), a byte program, handler per code. Verified with whole-program vectors (re/prog_vectors.txt: the original run in Unicorn, output plane hashed).
use crate::imgops::*;

pub const N: usize = 65536;
/// 16-bit lanes of a pixel (punpcklbw with zero) and back (packuswb: signed 16-bit -> 0..255)
fn up(p: u32) -> [i16; 4] { [0, 1, 2, 3].map(|i| ((p >> (8 * i)) & 0xff) as i16) }
fn pk16(l: [i16; 4]) -> u32 { l.iter().enumerate().fold(0, |a, (i, &v)| a | (v.clamp(0, 255) as u32) << (8 * i)) }
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
    /// pixel = (round(s1*s2*255) + 0x100) >> 1 (grey). fsin itself is not rounded to 24 bits (f64 here), the following fmul is.
    fn sines(&mut self, a: &[u8]) {
        let n = a[0] as usize; self.select(a[1]);
        let a2 = a[2] as f32 + a[2] as f32; let b = a[3] as f32 * 0.0078125f32; let c2 = a[4] as f32 + a[4] as f32; let d = a[5] as f32 * 0.0078125f32;
        // fsin is not rounded to the 24-bit precision control (only +,-,*,/,sqrt are): keep the f64 value, round at the next multiply
        let sin32 = |t: f32| -> f64 { ((std::f64::consts::PI * t as f64) as f32 as f64).sin() };
        for i in 0..N {
            let (x, y) = ((i & 0xff) as f32, ((i >> 8) & 0xff) as f32);
            let s1 = sin32(((x * 0.00390625f32) * a2) + b); let s2 = sin32(((y * 0.00390625f32) * c2) + d);
            let v = (((s1 * s2) as f32) * 255.0f32).round_ties_even() as i32; let e = ((v + 0x100) as u32) >> 1;
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
    /// code 0x15 (0x40dbf0): radial sine ring. Args: n, blend code, radius a. Pixel = 0 outside the circle d2 >= a^2, else round(255 * sin((a - sqrt(d2)) * (pi/2 / a))), d from the centre (128,128).
    fn ring(&mut self, a: &[u8]) {
        let n = a[0] as usize; self.select(a[1]); let r = a[2] as f32; let k = (std::f32::consts::FRAC_PI_2) / r; let r2 = r * r;
        for i in 0..N {
            let (dx, dy) = ((i & 0xff) as f32 - 128.0, ((i >> 8) & 0xff) as f32 - 128.0); let d2 = dx * dx + dy * dy;
            let e = if d2 < r2 { let t = (r - d2.sqrt()) * k; ((((t as f64).sin() * 255.0f64) as f32).round_ties_even() as i32) as u32 } else { 0 };
            self.planes[0][i] = e.wrapping_mul(0x10101);
        }
        self.combine_into(n);
    }

    // ---- filter handlers (0x20..0x51): source plane(s) -> work plane (plane 0), then combined into plane n; plane ids = al<<18 (0x40d48a), only 0..4 exist here ----
    /// flat byte read over the five contiguous planes (plane id << 18 + byte offset), like `movzx eax, byte [esi+ebx*4]` with a lane offset
    fn byte_at(&self, abs: usize) -> u32 { (self.planes[abs >> 18][(abs & 0x3ffff) >> 2] >> (8 * (abs & 3))) & 0xff }
    /// 0x20 (0x40dc7b): args n, blend, m, ox, oy: copy plane m into the work plane shifted by (ox, oy) with 8-bit wrap
    fn shift_copy(&mut self, a: &[u8]) {
        let (n, m) = (a[0] as usize, a[2] as usize); self.select(a[1]);
        for i in 0..N { let (x, y) = ((i & 0xff) as u8, (i >> 8) as u8); let d = ((y.wrapping_add(a[4]) as usize) << 8) | x.wrapping_add(a[3]) as usize; self.planes[0][d] = self.planes[m][i]; }
        self.combine_into(n);
    }
    /// 0x21 (0x40dcba): args n, blend, P, A, B, C, D, E, F (x87 f32): warp of plane P: ecx = round(4096*C*sin(pi*((y/256)*2A + B/128))), edx = round(4096*F*sin(pi*((x/256)*2D + E/128)));
    /// pixel = sample(P, (x + (ecx>>12), y + (edx>>12)), fractions ecx&0xfff, edx&0xfff)
    fn warp(&mut self, a: &[u8]) {
        let (n, p) = (a[0] as usize, a[2] as usize); self.select(a[1]);
        let (a2, b, c, d2, e, f) = (a[3] as f32 + a[3] as f32, a[4] as f32 * 0.0078125f32, a[5] as f32, a[6] as f32 + a[6] as f32, a[7] as f32 * 0.0078125f32, a[8] as f32);
        // fsin is not rounded to the 24-bit precision control (only +,-,*,/,sqrt are): keep the f64 value, round at the next multiply
        let sin32 = |t: f32| -> f64 { ((std::f64::consts::PI * t as f64) as f32 as f64).sin() };
        for i in 0..N {
            let (x, y) = ((i & 0xff) as u8, (i >> 8) as u8);
            let ecx = ((sin32(((y as f32 * 0.00390625f32) * a2) + b) * c as f64) as f32 * 4096.0f32).round_ties_even() as i32 as u32;
            let edx = ((sin32(((x as f32 * 0.00390625f32) * d2) + e) * f as f64) as f32 * 4096.0f32).round_ties_even() as i32 as u32;
            let (bl, bh) = (((ecx >> 12) as u8).wrapping_add(x), ((edx >> 12) as u8).wrapping_add(y));
            let v = sample(&self.planes[p], ((bh as u16) << 8) | bl as u16, 0x101, (ecx & 0xfff) as u16, (edx & 0xfff) as u16); self.planes[0][i] = v;
        }
        self.combine_into(n);
    }
    /// 0x24 (0x40dda7): args n, blend, P1, P2, off, k1, k2: displacement map: c = byte `off` of P2's pixel; pixel = sample(P1, (x + (c*k1>>8), y + (k2*c>>8)), fractions (c*k1<<4)&0xfff, (k2*c<<4)&0xfff)
    fn displace(&mut self, a: &[u8]) {
        let (n, p1, p2, off, k1, k2) = (a[0] as usize, a[2] as usize, a[3] as usize, a[4] as usize, a[5] as u32, a[6] as u32); self.select(a[1]);
        for i in 0..N {
            let (x, y) = ((i & 0xff) as u8, (i >> 8) as u8); let c = self.byte_at((p2 << 18) + off + 4 * i);
            let (ecx, edx) = (c * k1, k2 * c);
            let (bl, bh) = (x.wrapping_add((ecx >> 8) as u8), y.wrapping_add((edx >> 8) as u8));
            let v = sample(&self.planes[p1], ((bh as u16) << 8) | bl as u16, 0x101, ((ecx << 4) & 0xfff) as u16, ((edx << 4) & 0xfff) as u16); self.planes[0][i] = v;
        }
        self.combine_into(n);
    }
    /// 0x31 (0x40de29): args n, blend, P, k: overlay (0x40d883) of the grey k over plane P
    fn overlay_k(&mut self, a: &[u8]) {
        let (n, p) = (a[0] as usize, a[2] as usize); self.select(a[1]); let g = a[3] as u32 * 0x10101;
        for i in 0..N { self.planes[0][i] = blend_overlay(g, self.planes[p][i]); }
        self.combine_into(n);
    }
    /// 0x32 (0x40de6b): args n, blend, P, k: contrast around 128 per 16-bit lane: clamp(((v-128)*k >> 7) + 128), alpha lane included
    fn contrast(&mut self, a: &[u8]) {
        let (n, p, k) = (a[0] as usize, a[2] as usize, a[3] as i16); self.select(a[1]);
        for i in 0..N { self.planes[0][i] = pk16(up(self.planes[p][i]).map(|v| (v.wrapping_sub(128).wrapping_mul(k) >> 7).wrapping_add(128))); }
        self.combine_into(n);
    }
    /// 0x33 (0x40dece): args n, blend, P, off, k: overlay of the grey k over the grey made from byte `off` of each pixel of P
    fn overlay_lane(&mut self, a: &[u8]) {
        let (n, p, off) = (a[0] as usize, a[2] as usize, a[3] as usize); self.select(a[1]); let g = a[4] as u32 * 0x10101;
        for i in 0..N { let b = self.byte_at((p << 18) + off + 4 * i) * 0x10101; self.planes[0][i] = blend_overlay(g, b); }
        self.combine_into(n);
    }
    /// 0x35 (0x40df1d): args n, blend, P, lane, lo (dword), hi (dword): auto-levels of P into the work plane, then gradient map of byte `lane` of each work pixel: ((v*0x10101 <<4 per lane) * ((hi-lo)<<4) >> 16) + lo
    fn gradmap(&mut self, a: &[u8]) {
        let (n, p, lane) = (a[0] as usize, a[2] as usize, a[3] as usize); self.select(a[1]);
        let lo = up(u32::from_le_bytes([a[4], a[5], a[6], a[7]])); let hi = up(u32::from_le_bytes([a[8], a[9], a[10], a[11]]));
        self.planes[0] = auto_levels(&self.planes[p]);
        let d = [0, 1, 2, 3].map(|c| hi[c].wrapping_sub(lo[c]) << 4);
        for i in 0..N {
            let u = up(self.byte_at(4 * i + lane) * 0x10101);
            self.planes[0][i] = pk16([0, 1, 2, 3].map(|c| ((((u[c] << 4) as i32 * d[c] as i32) >> 16) as i16).wrapping_add(lo[c])));
        }
        self.combine_into(n);
    }
    /// 0x36 (0x40df9a): args n, blend, P, k: diagonal emboss: s = (a-f)+(b-e)+(c-d) over the 3x3 ring, pmulhw(s<<3, k<<7), packsswb, + 0x808080 per 16-bit lane
    fn emboss(&mut self, a: &[u8]) {
        let (n, p, k) = (a[0] as usize, a[2] as usize, a[3] as i16); self.select(a[1]);
        for i in 0..N {
            let (x, y) = ((i & 0xff) as u8, (i >> 8) as u8); let at = |dx: u8, dy: u8| up(self.planes[p][(((y.wrapping_add(dy)) as usize) << 8) | x.wrapping_add(dx) as usize]);
            let (pa, pb, pc, pd, pe, pf) = (at(1, 255), at(1, 0), at(1, 1), at(255, 1), at(255, 0), at(255, 255));
            let mut by = [0u8; 4];
            for c in 0..4 { let s = pa[c].wrapping_sub(pf[c]).wrapping_add(pb[c].wrapping_sub(pe[c])).wrapping_add(pc[c].wrapping_sub(pd[c])); let s = ((s as u16) << 3) as i16;
                by[c] = ((s as i32 * ((k << 7) as i32)) >> 16).clamp(-128, 127) as i8 as u8; }
            let (l0, l1) = ((by[0] as u16 | (by[1] as u16) << 8).wrapping_add(0x8080), (by[2] as u16 | (by[3] as u16) << 8).wrapping_add(0x0080));
            self.planes[0][i] = l0 as u32 | (l1 as u32) << 16;
        }
        self.combine_into(n);
    }
    /// 0x37 (0x40e04a): args n, blend, P, dh, ds: HSV shift: H += dh (byte), S = S*ds >> 8
    fn hsv_shift(&mut self, a: &[u8]) {
        let (n, p, dh, ds) = (a[0] as usize, a[2] as usize, a[3], a[4] as u32); self.select(a[1]);
        for i in 0..N {
            let x = rgb2hsv(self.planes[p][i]); let s = (((x >> 8) & 0xff) * ds) >> 8;
            self.planes[0][i] = hsv2rgb((x & 0xffff_0000) | (s & 0xff) << 8 | (x as u8).wrapping_add(dh) as u32);
        }
        self.combine_into(n);
    }
    /// 0x40 (0x40e0a7): args n, blend, P, k: 3x3 blur, centre weight k, ring weight 7 each (16-bit lane sum), pmulhw with 0x10000/(k+0x38)
    fn blur(&mut self, a: &[u8]) {
        let (n, p, k) = (a[0] as usize, a[2] as usize, a[3] as u32); self.select(a[1]); let rc = (0x10000 / (k + 0x38)) as u16 as i16;
        for i in 0..N {
            let (x, y) = ((i & 0xff) as u8, (i >> 8) as u8); let at = |dx: u8, dy: u8| up(self.planes[p][(((y.wrapping_add(dy)) as usize) << 8) | x.wrapping_add(dx) as usize]);
            let c = at(0, 0); let ring = [at(1, 0), at(1, 1), at(0, 1), at(255, 1), at(255, 0), at(255, 255), at(0, 255), at(1, 255)];
            self.planes[0][i] = pk16([0, 1, 2, 3].map(|l| { let mut s = (c[l] as u16).wrapping_mul(k as u16); for r in &ring { s = s.wrapping_add((r[l] as u16).wrapping_mul(7)); } ((s as i16 as i32 * rc as i32) >> 16) as i16 }));
        }
        self.combine_into(n);
    }
    /// 0x41 (0x40e198): args n, blend, P1, P2, off, k: vertical streak: len = (byte off of P2 * k) >> 9; pixel = (sum of P1 at y, y-1 .. y-len) * (0xffff/(len+1)) >> 16 (pmulhw); len 0 copies the pixel
    fn streak(&mut self, a: &[u8]) {
        let (n, p1, p2, off, k) = (a[0] as usize, a[2] as usize, a[3] as usize, a[4] as usize, a[5] as u32); self.select(a[1]);
        for i in 0..N {
            let (x, y) = ((i & 0xff) as u8, (i >> 8) as u8); let len = (self.byte_at((p2 << 18) + off + 4 * i) * k) >> 9;
            if len == 0 { self.planes[0][i] = self.planes[p1][i]; continue; }
            let mut s = up(self.planes[p1][i]); let mut yy = y;
            for _ in 0..len { yy = yy.wrapping_sub(1); let t = up(self.planes[p1][((yy as usize) << 8) | x as usize]); for c in 0..4 { s[c] = s[c].wrapping_add(t[c]); } }
            let rc = (0xffff / (len + 1)) as u16 as i16;
            self.planes[0][i] = pk16(s.map(|v| ((v as i32 * rc as i32) >> 16) as i16));
        }
        self.combine_into(n);
    }
    /// 0x50 (0x40e224): args n, blend, P1, P2, k1, k2: out = (P1*k1 >> 8) + (P2*k2 >> 8) per 16-bit lane, packuswb
    fn mix(&mut self, a: &[u8]) {
        let (n, p1, p2, k1, k2) = (a[0] as usize, a[2] as usize, a[3] as usize, a[4] as u16, a[5] as u16); self.select(a[1]);
        for i in 0..N { let (u, v) = (up(self.planes[p1][i]), up(self.planes[p2][i])); self.planes[0][i] = pk16([0, 1, 2, 3].map(|c| ((u[c] as u16).wrapping_mul(k1) >> 8).wrapping_add((v[c] as u16).wrapping_mul(k2) >> 8) as i16)); }
        self.combine_into(n);
    }
    /// 0x51 (0x40e2a3): args n, blend, P1, P2, P3, off: crossfade P1/P2 by the mask byte `off` of P3: (P1*(255-m) >> 8) + (P2*m >> 8)
    fn mask_mix(&mut self, a: &[u8]) {
        let (n, p1, p2, p3, off) = (a[0] as usize, a[2] as usize, a[3] as usize, a[4] as usize, a[5] as usize); self.select(a[1]);
        for i in 0..N {
            let m = self.byte_at((p3 << 18) + off + 4 * i) as u16; let (u, v) = (up(self.planes[p1][i]), up(self.planes[p2][i]));
            self.planes[0][i] = pk16([0, 1, 2, 3].map(|c| ((u[c] as u16).wrapping_mul(m ^ 0xff) >> 8).wrapping_add((v[c] as u16).wrapping_mul(m) >> 8) as i16));
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
                0x15 => { self.ring(&prog[p..p + 3]); p += 3; }
                0x20 => { self.shift_copy(&prog[p..]); p += 5; }
                0x21 => { self.warp(&prog[p..]); p += 9; }
                0x24 => { self.displace(&prog[p..]); p += 7; }
                0x31 => { self.overlay_k(&prog[p..]); p += 4; }
                0x32 => { self.contrast(&prog[p..]); p += 4; }
                0x33 => { self.overlay_lane(&prog[p..]); p += 5; }
                0x35 => { self.gradmap(&prog[p..]); p += 12; }
                0x36 => { self.emboss(&prog[p..]); p += 4; }
                0x37 => { self.hsv_shift(&prog[p..]); p += 5; }
                0x40 => { self.blur(&prog[p..]); p += 4; }
                0x41 => { self.streak(&prog[p..]); p += 6; }
                0x50 => { self.mix(&prog[p..]); p += 6; }
                0x51 => { self.mask_mix(&prog[p..]); p += 6; }
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
        let mut n = 0; let mut bad: Vec<String> = vec![];
        for line in include_str!("../../re/prog_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); let prog = hex(f[0]);
            if prog.iter().any(|&b| !matches!(b, 0x10 | 1..=4 | 0x00..=0xff)) { continue; }
            let mut vm = Vm::new(); let out = vm.run(&prog);
            let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_le_bytes()).collect();
            if fnv1a(&bytes) != f[1].parse::<u64>().unwrap() { bad.push(f[0].to_string()); } n += 1;
        }
        assert!(bad.is_empty(), "{} of {n} programs differ: {:?}", bad.len(), bad);
        assert!(n >= 30);
    }
}
#[cfg(test)]
mod dump {
    use super::*;
    #[test] #[ignore] fn dump_program() {
        let h = std::env::var("H7PROG").unwrap(); let prog: Vec<u8> = (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect();
        let out = Vm::new().run(&prog); std::fs::write(std::env::var("H7OUT").unwrap(), out.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    }
}
