//! The scene-graph evaluator (0x409a50 / 0x409b72 / 0x409b98), the keyframe-spline parameter evaluator (0x407e05) and the 2D node handlers, ported as literal
//! transcriptions that run on a flat copy of the original's memory (image at 0x400000, heap at 0x20000000): node objects, parameter nodes and key lists keep
//! the original byte layout, handler pointers stored in the nodes are the ORIGINAL addresses and are dispatched by address (`call_handler`). An address that is
//! not ported yet is reported (not silently skipped), so the set of missing handlers is always explicit.
//! x87 code runs through `X87`: every arithmetic op is rounded to f32 (the original sets the 24-bit precision control), same operand order as the instructions.

pub const IMG_BASE: u32 = 0x40_0000;
pub const HEAP_BASE: u32 = 0x2000_0000;

pub struct Mem { pub img: Vec<u8>, pub heap: Vec<u8> }
impl Mem {
    fn loc(&self, a: u32) -> (&Vec<u8>, usize) { if a >= HEAP_BASE { (&self.heap, (a - HEAP_BASE) as usize) } else { (&self.img, (a - IMG_BASE) as usize) } }
    fn loc_mut(&mut self, a: u32) -> (&mut Vec<u8>, usize) { if a >= HEAP_BASE { (&mut self.heap, (a - HEAP_BASE) as usize) } else { (&mut self.img, (a - IMG_BASE) as usize) } }
    pub fn r8(&self, a: u32) -> u8 { let (b, o) = self.loc(a); b[o] }
    pub fn r16(&self, a: u32) -> u16 { let (b, o) = self.loc(a); u16::from_le_bytes([b[o], b[o + 1]]) }
    pub fn r32(&self, a: u32) -> u32 { let (b, o) = self.loc(a); u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) }
    pub fn rf(&self, a: u32) -> f32 { f32::from_bits(self.r32(a)) }
    pub fn w8(&mut self, a: u32, v: u8) { let (b, o) = self.loc_mut(a); b[o] = v; }
    pub fn w16(&mut self, a: u32, v: u16) { let (b, o) = self.loc_mut(a); b[o..o + 2].copy_from_slice(&v.to_le_bytes()); }
    pub fn w32(&mut self, a: u32, v: u32) { let (b, o) = self.loc_mut(a); b[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
    pub fn wf(&mut self, a: u32, v: f32) { self.w32(a, v.to_bits()); }
    pub fn bytes(&self, a: u32, n: usize) -> &[u8] { let (b, o) = self.loc(a); &b[o..o + n] }
    pub fn bytes_mut(&mut self, a: u32, n: usize) -> &mut [u8] { let (b, o) = self.loc_mut(a); &mut b[o..o + n] }
    /// snapshot file = image bytes followed by the heap bytes (written by re/h7emu.py, see re/snap_tool.py)
    pub fn from_snapshot(raw: &[u8], img_len: usize) -> Mem { Mem { img: raw[..img_len].to_vec(), heap: raw[img_len..].to_vec() } }
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
pub struct Scene { pub m: Mem, pub ebp: u32, /** handlers met but not ported (skipped), in call order */ pub skipped: Vec<u32>, pub draw_log: Vec<(u32, u32)> }

impl Scene {
    pub fn new(m: Mem) -> Scene { let ebp = m.r32(0x42a3f8); Scene { m, ebp, skipped: vec![], draw_log: vec![] } }
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
    pub fn draw(&mut self, h: u32, esi: u32, _t: i32, edi: u32) { self.draw_log.push((h, esi)); let _ = edi; }
    /// 0x409a50: one frame evaluation from the current root [ebp+0x50]
    pub fn frame(&mut self, t: i32) { let r = self.m.r32(self.ebp + 0x50); if r != 0 { self.eval_node(r, t); } }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    pub const IMG_LEN: usize = 188416;
    pub fn dir() -> String { std::env::var("H7SNAP_DIR").unwrap_or("/tmp/claude-1000/w".into()) }
    pub fn load(name: &str) -> Option<Mem> { std::fs::read(format!("{}/{name}.bin", dir())).ok().map(|r| Mem::from_snapshot(&r, IMG_LEN)) }
    fn diff(a: &Mem, b: &Mem, lo: u32, hi: u32) -> Vec<u32> { (lo..hi).filter(|&x| a.r8(x) != b.r8(x)).collect() }
    /// evaluator phase: run frame k from the state at the evaluator entry (snapK) and compare the heap area holding the nodes and parameter nodes with the state
    /// the original has when the first draw handler is called (psnapK)
    #[test] fn eval_phase_matches() {
        let times = [(1u32, 0i32), (2, 81), (3, 163), (31, 2457)];
        let mut ran = 0;
        for (k, t) in times {
            let (Some(a), Some(b)) = (load(&format!("snap{k}")), load(&format!("psnap{k}"))) else { continue };
            let mut s = Scene::new(a);
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
