//! The scene-graph evaluator (0x409a50 / 0x409b72 / 0x409b98), the keyframe-spline parameter evaluator (0x407e05) and the 2D node handlers, ported as literal
//! transcriptions that run on a flat copy of the original's memory (image at 0x400000, heap at 0x20000000): node objects, parameter nodes and key lists keep
//! the original byte layout, handler pointers stored in the nodes are the ORIGINAL addresses and are dispatched by address (`call_handler`). An address that is
//! not ported yet is reported (not silently skipped), so the set of missing handlers is always explicit.
//! x87 code runs through `X87`: every arithmetic op is rounded to f32 (the original sets the 24-bit precision control), same operand order as the instructions.

use crate::blit::{blit_bilinear, blit_nearest, blit_rotated, blit_unscaled, Surf};
use crate::mmx::*;
use crate::text::{render_text, GlyphRec, Layer, Text};

pub const IMG_BASE: u32 = 0x40_0000;
pub const HEAP_BASE: u32 = 0x2000_0000;

pub struct Mem { pub img: Vec<u8>, pub heap: Vec<u8>, /** debug: every 8/16-bit write (address, size, value) */ pub log: Option<Vec<(u32, u8, u16)>> }
impl Mem {
    fn loc(&self, a: u32) -> (&Vec<u8>, usize) { if a >= HEAP_BASE { (&self.heap, (a - HEAP_BASE) as usize) } else { (&self.img, (a - IMG_BASE) as usize) } }
    fn loc_mut(&mut self, a: u32) -> (&mut Vec<u8>, usize) { if a >= HEAP_BASE { (&mut self.heap, (a - HEAP_BASE) as usize) } else { (&mut self.img, (a - IMG_BASE) as usize) } }
    pub fn r8(&self, a: u32) -> u8 { let (b, o) = self.loc(a); b[o] }
    pub fn r16(&self, a: u32) -> u16 { let (b, o) = self.loc(a); u16::from_le_bytes([b[o], b[o + 1]]) }
    pub fn r32(&self, a: u32) -> u32 { let (b, o) = self.loc(a); u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) }
    pub fn rf(&self, a: u32) -> f32 { f32::from_bits(self.r32(a)) }
    pub fn w8(&mut self, a: u32, v: u8) { if let Some(l) = &mut self.log { l.push((a, 1, v as u16)); } let (b, o) = self.loc_mut(a); b[o] = v; }
    pub fn w16(&mut self, a: u32, v: u16) { if let Some(l) = &mut self.log { l.push((a, 2, v)); } let (b, o) = self.loc_mut(a); b[o..o + 2].copy_from_slice(&v.to_le_bytes()); }
    pub fn w32(&mut self, a: u32, v: u32) { let (b, o) = self.loc_mut(a); b[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
    pub fn wf(&mut self, a: u32, v: f32) { self.w32(a, v.to_bits()); }
    pub fn bytes(&self, a: u32, n: usize) -> &[u8] { let (b, o) = self.loc(a); &b[o..o + n] }
    pub fn bytes_mut(&mut self, a: u32, n: usize) -> &mut [u8] { let (b, o) = self.loc_mut(a); &mut b[o..o + n] }
    /// snapshot file = image bytes followed by the heap bytes (written by re/h7emu.py, see re/snap_tool.py)
    pub fn from_snapshot(raw: &[u8], img_len: usize) -> Mem { Mem { img: raw[..img_len].to_vec(), heap: raw[img_len..].to_vec(), log: None } }
}

/// x87 register stack with 24-bit precision control: every arithmetic result is rounded to f32. `st(0)` is the top.
#[derive(Default)]
pub struct X87 { s: Vec<f32> }
#[derive(Clone, Copy)]
pub enum Op { Add, Sub, SubR, Mul, Div, DivR }
fn ap(op: Op, d: f32, s: f32) -> f32 { match op { Op::Add => d + s, Op::Sub => d - s, Op::SubR => s - d, Op::Mul => d * s, Op::Div => d / s, Op::DivR => s / d } }
pub fn fistp(x: f32) -> i32 { if x.is_finite() && x.abs() < 2147483648.0 { x.round_ties_even() as i32 } else { i32::MIN } }
impl X87 {
    pub fn new() -> X87 { X87 { s: Vec::with_capacity(8) } }
    fn ix(&self, i: usize) -> usize { self.s.len() - 1 - i }
    pub fn st(&self, i: usize) -> f32 { self.s[self.ix(i)] }
    pub fn push(&mut self, v: f32) { self.s.push(v); }
    pub fn pop(&mut self) -> f32 { self.s.pop().unwrap() }
    pub fn fld1(&mut self) { self.push(1.0); }
    pub fn fldst(&mut self, i: usize) { let v = self.st(i); self.push(v); }
    pub fn fild(&mut self, v: i32) { self.push(v as f32); }
    pub fn fxch(&mut self, i: usize) { let a = self.ix(0); let b = self.ix(i); self.s.swap(a, b); }
    pub fn fabs(&mut self) { let a = self.ix(0); self.s[a] = self.s[a].abs(); }
    pub fn fchs(&mut self) { let a = self.ix(0); self.s[a] = -self.s[a]; }
    /// fop st(d), st(s)  (d = destination; `Sub`: st(d) = st(d) - st(s); `SubR`: st(d) = st(s) - st(d)); `pop` for the ...p forms
    pub fn arith(&mut self, op: Op, d: usize, s: usize, pop: bool) { let r = ap(op, self.st(d), self.st(s)); let i = self.ix(d); self.s[i] = r; if pop { self.s.pop(); } }
    /// fop st, m32 (st0 = st0 op m32; `SubR`: m32 - st0)
    pub fn arith_m(&mut self, op: Op, m: f32) { let i = self.ix(0); self.s[i] = ap(op, self.s[i], m); }
}

fn cmp_i(a: i32, b: i32) -> std::cmp::Ordering { a.cmp(&b) }

/// 0x407e05: evaluate parameter node `p` at time `t` (ebx on entry). Writes the interpolated components to p+0x2c.., then calls the apply handler [p+0x10] (ebx = [p+0x18]).
pub fn eval_param(m: &mut Mem, p: u32, t: i32) {
    let mut ebx = t;
    let mut esi = m.r32(p + 0x24);
    if !(ebx > 0) { ebx = 1; }
    let period = m.r32(p + 0x1c) as i32;
    if !(ebx < period) { ebx = ((ebx as u32) % (period as u32)) as i32; }
    let n = m.r32(p + 4);
    // the original re-seeks from the first key when t <= the current key time (or no current key)
    let mut advance = false;
    if esi == 0 || !(ebx > m.r32(esi) as i32) { esi = m.r32(p + 0x20); advance = true; }
    else if ebx >= m.r32(esi + 0x40) as i32 { advance = true; }
    if advance {
        loop {
            // call [p+0x14]: the per-key callback (0x40695a = ret in every parameter of the scene)
            let cb = m.r32(p + 0x14);
            assert_eq!(cb, 0x40695a, "key callback {cb:#x} not ported");
            esi += 0x40; m.w32(p + 0x24, esi);
            if cmp_i(ebx, m.r32(esi + 0x40) as i32).is_ge() { continue; }
            break;
        }
        if n != 0 { hermite_setup(m, p, esi, n); }
    }
    if n != 0 {
        // 0x407ecc: local time u = (t - key.time) / (next.time - key.time)
        let mut d = ebx.wrapping_sub(m.r32(esi) as i32); if d < 0 { d = 0; }
        let dt = (m.r32(esi + 0x40) as i32).wrapping_sub(m.r32(esi) as i32);
        let mut x = X87::new();
        if dt > 0 { x.fild(d); x.arith_m(Op::Div, dt as f32); } else { x.fld1(); }   // fild; fidiv
        let g = ease(m, &mut x, esi);
        for i in 0..n {
            let c = p + 0x68 + 16 * i;
            let mut r = m.rf(c + 0xc);
            r *= g; r += m.rf(c + 8); r *= g; r += m.rf(c + 4); r *= g; r += m.rf(c);
            m.wf(p + 0x2c + 4 * i, r);
        }
    }
    let ebx = m.r32(p + 0x18);
    call_param_apply(m, p, ebx);
}

/// 0x407e42..0x407eb5: Hermite coefficients (value, tangent, c2, c3) of the segment [esi, esi+0x40] for every component
fn hermite_setup(m: &mut Mem, p: u32, esi: u32, n: u32) {
    let mut o0 = [0f32; 17]; let mut o1 = [0f32; 17];
    tangent(m, p, esi, true, &mut o0);
    tangent(m, p, esi + 0x40, false, &mut o1);
    for c in (1..=n).rev() {
        let cur = m.rf(esi + 4 * c); let nxt = m.rf(esi + 0x40 + 4 * c);
        let d = nxt - cur;                 // fld next; fsub cur
        let m0 = o0[c as usize]; let m1 = o1[c as usize];
        let two_m0 = m0 + m0;
        let two_d = d + d;
        let three_d = d * 3.0;
        let a = three_d - two_m0;          // 3D - 2*m0
        let b = m0 - two_d;                // m0 - 2D
        let c2 = a - m1;                   // 3D - 2m0 - m1
        let c3 = b + m1;                   // m0 + m1 - 2D
        let base = p + 0x68 + 16 * (c - 1);
        m.wf(base, cur); m.wf(base + 4, m0); m.wf(base + 8, c2); m.wf(base + 0xc, c3);
    }
}

/// 0x407d84 (+0x407d70): TCB tangent of the key `k` (`first` = ecx != 0: the incoming side), written to out[1..=n]
fn tangent(m: &Mem, p: u32, k: u32, first: bool, out: &mut [f32; 17]) {
    let n = m.r32(p + 4);
    let mut x = X87::new();
    x.push(m.rf(k + 0x2c));
    x.fild(m.r32(k) as i32);
    x.fild(m.r32(k + 0x40) as i32);
    x.arith(Op::Sub, 0, 1, false);
    x.fxch(1);
    let v = x.st(0) - (m.r32(k.wrapping_sub(0x40)) as i32) as f32; let i = x.s.len() - 1; x.s[i] = v;   // fisub [k-0x40]
    if first { x.fxch(1); x.fxch(2); x.fchs(); x.fxch(2); }
    x.arith(Op::Add, 1, 0, false);
    x.arith(Op::DivR, 1, 0, true);
    x.fldst(1); x.fabs(); x.fld1(); x.arith(Op::SubR, 1, 0, true);
    x.push(0.5);
    x.arith(Op::Sub, 2, 0, false);
    x.fxch(1);
    x.arith(Op::Mul, 2, 0, true);
    x.arith(Op::Add, 1, 0, true);
    // scale 1 = (1 - sA) * (1 + C)
    x.fld1(); x.arith(Op::Sub, 0, 2, false);
    x.fld1(); x.arith_m(Op::Add, m.rf(k + 0x30));
    x.arith(Op::Mul, 1, 0, true);
    let s1 = x.pop();
    let mut d1 = [0f32; 17];
    for c in 1..=n as usize { d1[c] = (m.rf(k + 4 * c as u32) - m.rf(k - 0x40 + 4 * c as u32)) * s1; }
    // scale 2 = (sA + 1) * (C - 1)
    x.fld1(); x.arith(Op::Add, 2, 0, true);
    x.fld1(); x.arith_m(Op::SubR, m.rf(k + 0x30));
    x.arith(Op::Mul, 2, 0, true);
    x.fxch(1);
    let s2 = x.pop();
    let mut d2 = [0f32; 17];
    for c in 1..=n as usize { d2[c] = (m.rf(k + 0x40 + 4 * c as u32) - m.rf(k + 4 * c as u32)) * s2; }
    // st0 = w; w*(1-B)
    x.fld1(); x.arith_m(Op::Sub, m.rf(k + 0x28));
    x.arith(Op::Mul, 1, 0, true);
    let s3 = x.pop();
    for c in 1..=n as usize { out[c] = (d1[c] - d2[c]) * s3; }
}

/// 0x407eeb..0x407f76: the ease-in/out curve of the segment; returns g(u) (u = st0 on entry)
fn ease(m: &Mem, x: &mut X87, esi: u32) -> f32 {
    x.push(m.rf(esi + 0x24));
    x.push(m.rf(esi + 0x60));
    x.fldst(0); x.arith(Op::Add, 0, 2, false);
    let s = x.st(0);
    if (s.to_bits() << 1) == 0 { x.pop(); x.pop(); x.pop(); return x.pop(); }
    let big = (s.to_bits() << 1) > 0x7f00_0000;
    x.fld1();
    if big {
        x.arith(Op::DivR, 2, 0, false); x.fxch(1); x.arith(Op::Mul, 2, 0, false); x.arith(Op::Mul, 3, 0, true); x.fxch(3);
    } else {
        x.fldst(0); x.arith(Op::Add, 0, 0, false); x.arith(Op::SubR, 2, 0, true); x.arith(Op::DivR, 1, 0, true); x.fxch(3);
    }
    // stack: [u, Y, Z, k]
    let u = x.st(0); x.fxch(2);                    // [Z, Y, u, k]
    let z = x.st(0);
    if (u.to_bits() as i32) < (z.to_bits() as i32) {
        x.arith(Op::Div, 3, 0, true); x.pop(); x.arith(Op::Mul, 0, 0, false); x.arith(Op::Mul, 1, 0, true);
    } else {
        x.fld1(); x.arith(Op::SubR, 3, 0, true);   // [Z, Y, 1-u, k]
        x.fxch(1);                                 // [Y, Z, 1-u, k]
        let y = x.st(0); x.fxch(2);                // [1-u, Z, Y, k]
        let omu = x.st(0);
        if !((y.to_bits() as i32) < (omu.to_bits() as i32)) {
            x.arith(Op::Mul, 0, 0, false); x.fxch(2); x.arith(Op::Div, 3, 0, true); x.pop(); x.arith(Op::Mul, 1, 0, true);
            x.fld1(); x.arith(Op::SubR, 1, 0, true);
        } else {
            x.push(u); x.arith_m(Op::Add, u); x.fxch(3); x.pop(); x.pop(); x.arith(Op::Sub, 1, 0, true); x.arith(Op::Mul, 1, 0, true);
        }
    }
    x.pop()
}

/// the apply handlers stored at [param+0x10]: copy the evaluated components to the destination [param+0x18]
fn call_param_apply(m: &mut Mem, p: u32, dst: u32) {
    let h = m.r32(p + 0x10);
    match h {
        0x408040 => { // time window: node flag byte = 1 - (bit 6 of the current key offset)
            let off = m.r32(p + 0x24).wrapping_sub(m.r32(p + 0x20));
            let v = 1u8.wrapping_sub(((off >> 6) & 1) as u8); m.w8(dst, v);
        }
        0x4081b0 => { let n = m.r32(p + 4); for i in (1..=n).rev() { let v = m.r32(p + 0x28 + 4 * i); m.w32(dst + 4 * i - 4, v); } }
        0x408051 => { let v = fistp(m.rf(p + 0x2c)); m.w32(dst, v as u32); }
        0x40807d => { // colour: four components -> 16-bit words clamped to 0..255
            let mut w = [0u16; 4];
            for i in (0..4).rev() { w[i] = fistp(m.rf(p + 0x2c + 4 * i as u32)) as i16 as u16; }
            for i in 0..4 { let v = w[i] as i16; let v = if v < 0 { 0 } else if v > 0xff { 0xff } else { v }; m.w16(dst + 2 * i as u32, v as u16); }
        }
        _ => panic!("param apply handler {h:#x} not ported"),
    }
}

/// The scene: memory plus the evaluator state. `ebp` = the context struct [0x42a3f8] (heap 0x20001000).
pub struct Scene { pub caller: u32, /** test mode: skip draws until this handler is met (0 = off) */ pub gate: u32, /** test mode: stop drawing when this handler is met (0 = off) */ pub stop: u32, pub m: Mem, pub ebp: u32, /** handlers met but not ported (skipped), in call order */ pub skipped: Vec<u32>, pub draw_log: Vec<(u32, u32)> }

impl Scene {
    pub fn new(m: Mem) -> Scene { let ebp = m.r32(0x42a3f8); Scene { caller: 0, gate: 0, stop: 0, m, ebp, skipped: vec![], draw_log: vec![] } }
    /// 0x409b72: evaluate the parameter list of `node`, then (when its enable flag byte is non-zero) call its handler
    pub fn eval_node(&mut self, node: u32, t: i32) {
        let mut p = self.m.r32(node + 0x14);
        if p != 0 { eval_param(&mut self.m, p, t); p = self.m.r32(p + 8); }
        if self.m.r8(node) == 0 { return; }
        while p != 0 { eval_param(&mut self.m, p, t); p = self.m.r32(p + 8); }
        let h = self.m.r32(node + 0x10);
        self.call_eval(h, node, t, 0);
    }
    /// 0x409b98: evaluate every child of `node`
    pub fn eval_children(&mut self, node: u32, t: i32) {
        let mut c = self.m.r32(node + 8);
        while c != 0 { self.eval_node(c, t); c = self.m.r32(c + 4); }
    }
    fn call_eval(&mut self, h: u32, esi: u32, t: i32, _edi: u32) {
        match h {
            0x409b98 => self.eval_children(esi, t),
            0x408255 => { self.eval_children(esi, t); let v = self.m.r32(esi + 0x18); self.eval_node(v, t); }   // the present (0x401565) is done by the caller
            0x40b231 => self.eval_viewport(esi, t),
            0x40ae69 => self.eval_sprite(esi, t),
            0x40a542 => { let a = self.m.r32(esi + 0x34) as i32; if a > 1 && a < 0x1ff { self.eval_sprite(esi, t); } }
            0x40b6ab => { if (self.m.r32(esi + 0x78) as i32) < 0x62 { self.eval_sprite(esi, t); } }
            0x4081e8 => { // class 49 (flip-book texture): frame = ((caller offset * 0x3d0 + t * 1000) >> 16) mod nframes; data = base + frame * stride
                let off = self.caller.wrapping_sub(self.ebp + 0x1e0);
                let v = (off.wrapping_mul(0x3d0).wrapping_add((t as u32).wrapping_mul(1000))) >> 16;
                let fr = v % self.m.r32(esi + 0x2c);
                let d = fr.wrapping_mul(self.m.r32(esi + 0x38)).wrapping_add(self.m.r32(esi + 0x34)); self.m.w32(esi + 0x28, d);
            }
            0x408bda => self.skipped.push(h),   // class 18 (3D object / camera matrices): with the 3D engine
            _ => panic!("node handler {h:#x} not ported"),
        }
    }
    /// 0x40ae69: class 31 family: insert into the draw list [ebp+0x48] (sorted by key +0x18, signed, descending), absolute position = own + parent, evaluate children
    fn eval_sprite(&mut self, esi: u32, t: i32) {
        let m = &mut self.m;
        let key = m.r32(esi + 0x18) as i32;
        let mut link = self.ebp + 0x48;
        let mut edx = m.r32(link);
        while edx != 0 && !((m.r32(edx + 0x18) as i32) < key) { link = edx + 0x1c; edx = m.r32(link); }
        m.w32(esi + 0x1c, edx); m.w32(link, esi);
        let par = m.r32(esi + 0xc);
        let (mut x, mut y) = (m.rf(esi + 0x20), m.rf(esi + 0x24));
        if m.r8(par + 1) & 1 != 0 { y += m.rf(par + 0x2c); x += m.rf(par + 0x28); }
        m.wf(esi + 0x2c, y); m.wf(esi + 0x28, x);
        self.eval_children(esi, t);
    }
    /// 0x40b231 (class 3 viewport): evaluate the children into a fresh draw list, then call every list node's draw handler [+0x30]
    fn eval_viewport(&mut self, esi: u32, t: i32) {
        let old = self.m.r32(self.ebp + 0x48); self.m.w32(self.ebp + 0x48, 0);
        self.eval_children(esi, t);
        let mut n = self.m.r32(self.ebp + 0x48); self.m.w32(self.ebp + 0x48, old);
        while n != 0 { let h = self.m.r32(n + 0x30); self.draw(h, n, t, esi); n = self.m.r32(n + 0x1c); }
    }
    /// 0x408cdd: the sprite rectangle in 16.16 layer pixels: +0x3c = (x + f34) * f30, +0x40 = (f38 - y) * f30, +0x44 = w * f30, +0x48 = h * f30 (x87 order, fistp)
    fn sprite_rect(&mut self, esi: u32, edi: u32) {
        let m = &mut self.m; let f30 = m.rf(edi + 0x30);
        let hh = m.rf(esi + 0x38) * f30; let ww = m.rf(esi + 0x34) * f30;
        m.w32(esi + 0x48, fistp(hh) as u32); m.w32(esi + 0x44, fistp(ww) as u32);
        let x = (m.rf(esi + 0x28) + m.rf(edi + 0x34)) * f30; let y = (-m.rf(esi + 0x2c) + m.rf(edi + 0x38)) * f30;
        m.w32(esi + 0x40, fistp(y) as u32); m.w32(esi + 0x3c, fistp(x) as u32);
    }
    /// 0x408d8c (class 5): evaluate the texture node [+0x58], then blit it onto the 8-bit layer with the blitter chosen by [+0x4c] (table 0x42a200: nearest, rotated, bilinear, unscaled)
    fn draw_sprite(&mut self, esi: u32, edi: u32, t: i32) {
        self.sprite_rect(esi, edi);
        let tex = self.m.r32(esi + 0x58);
        self.caller = esi; self.eval_node(tex, t);
        let mode = self.m.r32(esi + 0x4c);
        let rect = [self.m.r32(esi + 0x3c) as i32, self.m.r32(esi + 0x40) as i32, self.m.r32(esi + 0x44) as i32, self.m.r32(esi + 0x48) as i32];
        let p1 = self.m.r32(esi + 0x54) as i32; let ang = self.m.rf(esi + 0x5c);
        let (w, h, praw, dp) = (self.m.r32(tex + 0x18) as i32, self.m.r32(tex + 0x1c) as i32, self.m.r32(tex + 0x20) as i32, self.m.r32(tex + 0x28));
        let n = ((praw >> 2) * (h + 2)).max(64 * 256) as usize;
        let src = Surf { w, h, pitch: praw >> 2, data: self.m.bytes(dp, n).to_vec() };
        let mut dst = self.surf8(edi);
        match mode { 0 => blit_nearest(p1, rect, &src, &mut dst), 1 => blit_rotated(ang, p1, rect, &src, &mut dst), 2 => blit_bilinear(p1, rect, &src, &mut dst), 3 => blit_unscaled(p1, rect, &src, &mut dst), _ => panic!("sprite mode {mode}") }
        let dd = self.m.r32(edi + 0x28); let n = dst.data.len(); self.m.bytes_mut(dd, n).copy_from_slice(&dst.data);
    }
    /// 0x408fab (class 50): glow streak at the node position; A = [34]*[38]*65536, B = [38]*65536*(1-[34]) (x87, fistp)
    fn draw_streak(&mut self, esi: u32, edi: u32) {
        let m = &mut self.m;
        let s0 = m.rf(esi + 0x38) * 65536.0; let omk = 1.0 - m.rf(esi + 0x34); let k = m.rf(esi + 0x34) * s0; let b = s0 * omk;
        let (a_i, b_i) = (fistp(k), fistp(b));
        let f30 = m.rf(edi + 0x30);
        let x = (m.rf(esi + 0x28) + m.rf(edi + 0x34)) * f30; let y = (-m.rf(esi + 0x2c) + m.rf(edi + 0x38)) * f30;
        let (tab, stride) = (m.r32(0x42a3f0), m.r32(0x42a3ec));
        if std::env::var("H7DBG").is_ok() { eprintln!("streak A={a_i:#x} B={b_i:#x} pos=({:#x},{:#x}) tab={tab:#x} stride={stride:#x}", fistp(x), fistp(y)); }
        crate::streak::streak(m, tab, stride, a_i as u32, b_i as u32, (fistp(x), fistp(y)), edi);
    }
    pub fn draw(&mut self, h: u32, esi: u32, _t: i32, edi: u32) {
        self.draw_log.push((h, esi));
        if self.stop != 0 { if h == self.stop { self.stop = u32::MAX; } if self.stop == u32::MAX { return; } }
        if self.gate != 0 { if h != self.gate { return; } self.gate = 0; }      // test mode: start at this handler (the earlier layers come from the snapshot)
        match h {
            0x409cda => self.draw_text(esi, edi),
            0x409dec => self.draw_palette(esi, edi),
            0x408d8c => self.draw_sprite(esi, edi, _t),
            0x408fab => self.draw_streak(esi, edi),
            _ => self.skipped.push(h),   // 3D scene (0x40a680), glow/blur (0x408d8c, 0x408fab) ...: not ported yet
        }
    }
    fn surf8(&self, o: u32) -> Surf { let (w, h, praw, dp) = (self.m.r32(o + 0x18) as i32, self.m.r32(o + 0x1c) as i32, self.m.r32(o + 0x20) as i32, self.m.r32(o + 0x28));
        let pitch = praw >> 2; let n = (pitch * (h + 2)) as usize; Surf { w, h, pitch, data: if dp != 0 { self.m.bytes(dp, n).to_vec() } else { vec![] } } }
    /// 0x409cda: the text object `esi` onto the 8-bit layer of the viewport `edi` (text.rs holds the port; the glyph mips are the 0x600 records' three 0x200 surfaces)
    fn draw_text(&mut self, esi: u32, edi: u32) {
        let mut glyphs: Vec<GlyphRec> = vec![];
        let mut p = esi + 0x44;
        while self.m.r32(p) != 0 { let g = self.m.r32(p); glyphs.push(GlyphRec { mips: [self.surf8(g), self.surf8(g + 0x200), self.surf8(g + 0x400)] }); p += 4; }
        if glyphs.is_empty() { return; }
        let refs: Vec<&GlyphRec> = glyphs.iter().collect();
        let t = Text { x: self.m.rf(esi + 0x28), y: self.m.rf(esi + 0x2c), scale: self.m.rf(esi + 0x34), spacing: self.m.rf(esi + 0x38), alpha: self.m.r32(esi + 0x3c) as i32,
                       force: self.m.r8(esi + 0x40) & 1 != 0, flag_t: self.m.r32(self.ebp + 0x18) & 1 != 0 };
        let mut layer = Layer { surf: self.surf8(edi), f2c: self.m.rf(edi + 0x2c), f30: self.m.rf(edi + 0x30), f34: self.m.rf(edi + 0x34), f38: self.m.rf(edi + 0x38) };
        render_text(&t, &refs, &mut layer);
        let dp = self.m.r32(edi + 0x28); let n = layer.surf.data.len(); self.m.bytes_mut(dp, n).copy_from_slice(&layer.surf.data);
    }
    /// 0x409dec (class 14): build the 256-entry colour ramp (0x409dec..0x409ea4) from the two colours at +0x34/+0x3c, then 0x409f80: add ramp[layer byte] (saturating bytes)
    /// to the 32-bit buffer for every non-zero layer byte and clear the layer
    fn draw_palette(&mut self, esi: u32, edi: u32) {
        let m = &mut self.m;
        let tab = self.ebp + 0xc01e0;
        let a = (m.r32(esi + 0x34) as u64) | (m.r32(esi + 0x38) as u64) << 32;
        let b = (m.r32(esi + 0x3c) as u64) | (m.r32(esi + 0x40) as u64) << 32;
        let mut mm3 = psubw(b, a); let mut mm2 = psllw(a, 7);
        let exp = m.r32(esi + 0x4c) as i32;
        assert!(exp <= 0x10, "palette variant with exponent {exp} not ported");
        let mut edi_i: u32 = 0;
        let mut ecx = m.r32(esi + 0x44) & 0xffff;
        while ecx != 0 { if edi_i == 0x100 { break; } m.w32(tab + 4 * edi_i, 0); mm2 = paddw(mm2, mm3); edi_i += 1; ecx -= 1; }
        let mut ecx2 = 0x80i32 - m.r32(esi + 0x48) as i32 - edi_i as i32;
        if ecx2 > 0 && edi_i != 0x100 {
            let eax = 0x8000u32 / ecx2 as u32;
            let w = eax as u16 as u64; let mm5 = w | w << 16 | w << 32 | w << 48;
            let mut mm6 = 0u64;
            while ecx2 > 0 {
                let mut mm1 = mm2; let mut mm4 = mm6; let mut mm7 = mm6; let mut edx = exp as u32;
                loop { let c = edx & 1; edx >>= 1; if c == 0 { /* jae */ } else { mm4 = pmulhw(mm4, mm7); mm4 = paddw(mm4, mm4); }
                       if edx == 0 { break; } mm7 = pmulhw(mm7, mm7); mm7 = paddw(mm7, mm7); }
                mm1 = pmulhw(mm1, mm4); mm1 = psraw(mm1, 6); let v = packuswb_lo(mm1) as u32;
                if edi_i < 0x100 { m.w32(tab + 4 * edi_i, v); }
                edi_i += 1; mm2 = paddw(mm2, mm3); mm6 = paddw(mm6, mm5); ecx2 -= 1;
            }
        }
        while edi_i != 0x100 { let v = packuswb_lo(psraw(mm2, 7)) as u32; m.w32(tab + 4 * edi_i, v); mm2 = paddw(mm2, mm3); edi_i += 1; }
        // 0x409f80
        let (w, h, pitch, d32, d8) = (m.r32(edi + 0x18) as usize, m.r32(edi + 0x1c) as usize, m.r32(edi + 0x20) as usize, m.r32(edi + 0x24), m.r32(edi + 0x28));
        for y in 0..h {
            for g in 0..(w >> 2) {
                let lp = d8 + (y * (pitch >> 2) + g * 4) as u32;
                if m.r32(lp) == 0 { continue; }
                let fp = d32 + (y * pitch + g * 16) as u32;
                for px in 0..4u32 {
                    let idx = m.r8(lp + px) as u32; let e = m.r32(tab + 4 * idx);
                    let o = m.r32(fp + 4 * px);
                    let mut r = 0u32; for k in 0..4 { let s = ((o >> (8 * k)) & 0xff) + ((e >> (8 * k)) & 0xff); r |= s.min(255) << (8 * k); }
                    m.w32(fp + 4 * px, r);
                }
                m.w32(lp, 0);
            }
        }
    }
    /// 0x409a50: one frame evaluation from the current root [ebp+0x50]
    pub fn frame(&mut self, t: i32) { let r = self.m.r32(self.ebp + 0x50); if r != 0 { self.eval_node(r, t); } }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    pub const IMG_LEN: usize = 188416;
    pub fn dir() -> String { std::env::var("H7SNAP_DIR").unwrap_or("/tmp/claude-1000/w".into()) }
    /// (frame, t) of the evaluator calls (re/snap_tool.py writes times.txt)
    pub fn times() -> Vec<(u32, i32)> { std::fs::read_to_string(format!("{}/times.txt", dir())).unwrap_or_default().lines().filter_map(|l| { let mut i = l.split(' '); Some((i.next()?.parse().ok()?, i.next()?.parse().ok()?)) }).collect() }
    pub fn load(name: &str) -> Option<Mem> { std::fs::read(format!("{}/{name}.bin", dir())).ok().map(|r| Mem::from_snapshot(&r, IMG_LEN)) }
    fn diff(a: &Mem, b: &Mem, lo: u32, hi: u32) -> Vec<u32> { (lo..hi).filter(|&x| a.r8(x) != b.r8(x)).collect() }
    /// evaluator phase: run frame k from the state at the evaluator entry (snapK) and compare the heap area holding the nodes and parameter nodes with the state
    /// the original has when the first draw handler is called (psnapK)
    #[test] fn eval_phase_matches() {
        let mut ran = 0;
        for (k, t) in times() {
            let (Some(a), Some(b)) = (load(&format!("snap{k}")), load(&format!("psnap{k}"))) else { continue };
            let mut s = Scene::new(a); s.gate = u32::MAX;   // evaluation phase only: no draws
            s.frame(t);
            let ebp = s.ebp;
            let end = b.r32(ebp + 0x44);
            let d: Vec<u32> = diff(&s.m, &b, ebp + 0x1e0, end).into_iter().filter(|x| !(0x2006_1fe0..0x2006_21e0).contains(x)).collect();   // the class-18 object (3D matrices) is not ported
            let d2 = diff(&s.m, &b, 0x2024_0000, 0x2030_0000);
            println!("frame {k}: node-table diffs {} param/key diffs {} skipped {:?} draws {}", d.len(), d2.len(), s.skipped, s.draw_log.len());
            for x in d.iter().take(12) { println!("  node {:#x} (obj {:#x}+{:#x}) rust {:02x} orig {:02x}", x, (x - ebp - 0x1e0) / 0x200 * 0x200 + ebp + 0x1e0, (x - ebp - 0x1e0) % 0x200, s.m.r8(*x), b.r8(*x)); }
            for x in d2.iter().take(12) { println!("  heap {:#x} rust {:02x} orig {:02x}", x, s.m.r8(*x), b.r8(*x)); }
            assert!(d.is_empty() && d2.is_empty());
            ran += 1;
        }
        if ran == 0 { eprintln!("no snapshots in {} (see re/snap_tool.py)", dir()); }
    }
}

#[cfg(test)]
mod draw_tests {
    use super::*;
    use super::tests::{dir, load};
    /// draw phase from the first text draw: the snapshot taken there holds the 3D + glow result in the viewport's 32-bit buffer; the Rust text draws + the second palette pass must
    /// reproduce the frame the original presents (320x176 dwords, plain copy by 0x40c76b)
    #[test] fn text_and_palette_pass_match_the_presented_frame() { check_from("tsnap", 0x409cda); }
    /// from the class-5 sprite draw: sprite blitter + glow streak + palette + text must give the presented frame
    #[test] fn sprite_streak_palette_text_match_the_presented_frame() { check_from("bsnap", 0x408d8c); }
    /// layer after the class-5 sprite draw = the original's layer when the class-50 draw starts
    #[test] fn sprite_layer_matches() {
        for (k, t) in super::tests::times() {
            let (Some(a), Some(b)) = (load(&format!("bsnap{k}")), load(&format!("hs408fab_{k}"))) else { continue };
            let mut s = Scene::new(a); s.gate = 0x408d8c; s.stop = 0x408fab; s.frame(t);
            let vp = { let r = s.m.r32(s.ebp + 0x50); s.m.r32(r + 0x18) };
            let (n, d8) = ((s.m.r32(vp + 0x20) >> 2) * s.m.r32(vp + 0x1c), s.m.r32(vp + 0x28));
            let diff = (0..n).filter(|&i| s.m.r8(d8 + i) != b.r8(d8 + i)).count();
            println!("frame {k}: sprite layer differs in {diff} of {n} bytes");
            assert_eq!(diff, 0);
        }
    }
    /// layer after the streak = the layer when the palette pass starts
    #[test] fn streak_layer_matches() {
        for (k, t) in super::tests::times() {
            let (Some(a), Some(b)) = (load(&format!("hs408fab_{k}")), load(&format!("hs409dec_{k}"))) else { continue };
            let mut s = Scene::new(a); s.gate = 0x408fab; s.stop = 0x409dec; s.m.log = Some(vec![]); s.frame(t);
            if let Ok(w) = std::fs::read_to_string(format!("{}/streak_writes_{k}.txt", dir())) {   // the original's write trace (re/streak_trace.py): first divergence
                let orig: Vec<(u32, u32, u32)> = w.lines().map(|l| { let v: Vec<u32> = l.split(' ').map(|x| x.parse().unwrap()).collect(); (v[0], v[1], v[2]) }).collect();
                let d8 = { let r = s.m.r32(s.ebp + 0x50); let vp = s.m.r32(r + 0x18); s.m.r32(vp + 0x28) };
                let mine: Vec<(u32, u32, u32)> = s.m.log.as_ref().unwrap().iter().filter(|e| e.0 >= d8 && e.0 < d8 + 56320 + 64).map(|e| (e.0 - d8, e.1 as u32, e.2 as u32)).collect();
                println!("  write traces: original {} rust {}", orig.len(), mine.len());
                if let Some(i) = (0..orig.len().min(mine.len())).find(|&i| orig[i] != mine[i]) { println!("  first divergence at write #{i}: original {:?} rust {:?} (prev {:?})", orig[i], mine[i], if i > 0 { orig[i - 1] } else { (0, 0, 0) }); }
            }
            let vp = { let r = s.m.r32(s.ebp + 0x50); s.m.r32(r + 0x18) };
            let (n, d8) = ((s.m.r32(vp + 0x20) >> 2) * s.m.r32(vp + 0x1c), s.m.r32(vp + 0x28));
            let diff = (0..n).filter(|&i| s.m.r8(d8 + i) != b.r8(d8 + i)).count();
            println!("frame {k}: streak layer differs in {diff} of {n} bytes");
            let pw = (s.m.r32(vp + 0x20) >> 2) as u32;
            let bad: Vec<u32> = (0..n).filter(|&i| s.m.r8(d8 + i) != b.r8(d8 + i)).collect();
            let bef = load(&format!("hs408fab_{k}")).unwrap();
            let rc = (0..n).filter(|&i| s.m.r8(d8 + i) != bef.r8(d8 + i)).count(); let oc = (0..n).filter(|&i| b.r8(d8 + i) != bef.r8(d8 + i)).count();
            println!("  rust changed {rc} bytes, original changed {oc}");
            for yy in 14..22u32 { let mut a = String::new(); let mut c = String::new(); for xx in 150..164u32 { a += &format!("{:02x} ", s.m.r8(d8 + yy * pw + xx)); c += &format!("{:02x} ", b.r8(d8 + yy * pw + xx)); } println!("  y{yy} rust {a}| orig {c}"); }
            for by in 0..11u32 { let mut line = String::new(); for bx in 0..20u32 { let mut c = 0; for yy in 0..16 { for xx in 0..16 { let i = (by * 16 + yy) * pw + bx * 16 + xx; if s.m.r8(d8 + i) != b.r8(d8 + i) { c += 1; } } } line.push(if c == 0 { '.' } else if c < 64 { 'o' } else { '#' }); } println!("  {line}"); }
            if let (Some(f), Some(l)) = (bad.first(), bad.last()) { println!("  first bad ({},{}) last ({},{})", f % pw, f / pw, l % pw, l / pw); }
            for &i in bad.iter().take(8) { println!("  ({},{}) rust {:02x} orig {:02x} before {:02x}", i % pw, i / pw, s.m.r8(d8 + i), b.r8(d8 + i), load(&format!("hs408fab_{k}")).unwrap().r8(d8 + i)); }
            assert_eq!(diff, 0);
        }
    }
    /// 32-bit buffer after the palette pass (= the buffer at the first text draw)
    #[test] fn palette_buffer_matches() {
        let mut bad = 0;
        for (k, t) in super::tests::times() {
            let (Some(a), Some(b)) = (load(&format!("bsnap{k}")), load(&format!("tsnap{k}"))) else { continue };
            let mut s = Scene::new(a); s.gate = 0x408d8c; s.stop = 0x409cda; s.frame(t);
            let vp = { let r = s.m.r32(s.ebp + 0x50); s.m.r32(r + 0x18) };
            let (n, d32, d8) = (320 * 176 * 4, s.m.r32(vp + 0x24), s.m.r32(vp + 0x28));
            let diff = (0..n).filter(|&i| s.m.r8(d32 + i) != b.r8(d32 + i)).count();
            let diff8 = (0..320 * 178).filter(|&i| s.m.r8(d8 + i) != b.r8(d8 + i)).count();
            println!("frame {k}: buffer differs in {diff} bytes, layer in {diff8}");
            if diff > 0 { let first = (0..n).find(|&i| s.m.r8(d32 + i) != b.r8(d32 + i)).unwrap(); println!("   first at px ({},{})", (first / 4) % 320, first / 4 / 320); }
            bad += diff + diff8;
        }
        assert_eq!(bad, 0);
    }
    fn check_from(prefix: &str, gate: u32) {
        let mut bad = 0;
        for (k, t) in super::tests::times() {
            let Some(a) = load(&format!("{prefix}{k}")) else { continue };
            let Ok(fb) = std::fs::read(format!("{}/fb{k}.bin", dir())) else { continue };
            let mut s = Scene::new(a); s.gate = gate;
            s.frame(t);
            let root = s.m.r32(s.ebp + 0x50); let vp = s.m.r32(root + 0x18);
            let (w, h, d32) = (s.m.r32(vp + 0x18) as usize, s.m.r32(vp + 0x1c) as usize, s.m.r32(vp + 0x24));
            let mine = s.m.bytes(d32, w * h * 4).to_vec();
            let before = s.m.bytes(d32, 0).len(); let _ = before;
            let diff = (0..w * h * 4).filter(|&i| mine[i] != fb[i]).count();
            let mae: f64 = (0..w * h * 4).map(|i| (mine[i] as f64 - fb[i] as f64).abs()).sum::<f64>() / (w * h * 4) as f64;
            println!("frame {k}: {w}x{h} differing bytes {diff} of {} (mean abs error {mae:.4}); bars (rows 176..240) black: {}", w * h * 4, fb[w * h * 4..].iter().all(|&b| b == 0));
            if diff > 0 { let ds: Vec<usize> = (0..w * h * 4).filter(|&i| mine[i] != fb[i]).collect(); for &i in ds.iter().take(10) { println!("   px ({},{}) ch{} rust {} orig {}", (i / 4) % w, (i / 4) / w, i % 4, mine[i], fb[i]); } }
            bad += diff;
        }
        assert_eq!(bad, 0);
    }
}
