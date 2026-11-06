//! The 3D pass (class 13 draw 0x40a680): evaluation handlers of the 3D subtree, camera/object lists and the span rasteriser, literal ports on the flat memory image.
//! x87 sequences are generated from the disassembly (re/x87gen.py) and run on `X87` (24-bit precision control: every op rounded to f32).
use crate::scene::*;
pub use crate::engine::*;

/// 0x40d35d: ebx.. = inverse of the 3x3 matrix at eax (adjugate * 1/det), written to edx
pub fn mat_inv(m: &mut Mem, eax: u32, edx: u32) {
    let mut x = X87::new();
    x.push(m.rf(eax.wrapping_add(0x10)));  // 40d35d fld    DWORD PTR [eax+0x10]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x20)));  // 40d360 fmul   DWORD PTR [eax+0x20]
    x.push(m.rf(eax.wrapping_add(0x1c)));  // 40d363 fld    DWORD PTR [eax+0x1c]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x14)));  // 40d366 fmul   DWORD PTR [eax+0x14]
    x.push(m.rf(eax.wrapping_add(0x1c)));  // 40d369 fld    DWORD PTR [eax+0x1c]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x8)));  // 40d36c fmul   DWORD PTR [eax+0x8]
    x.push(m.rf(eax.wrapping_add(0x4)));  // 40d36f fld    DWORD PTR [eax+0x4]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x20)));  // 40d372 fmul   DWORD PTR [eax+0x20]
    x.fxch(2);  // 40d375 fxch   st(2)
    x.arith(Op::Sub, 3, 0, true);  // 40d377 fsubp  st(3),st
    x.arith(Op::SubR, 1, 0, true);  // 40d379 fsubrp st(1),st
    x.push(m.rf(eax.wrapping_add(0x4)));  // 40d37b fld    DWORD PTR [eax+0x4]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x14)));  // 40d37e fmul   DWORD PTR [eax+0x14]
    x.push(m.rf(eax.wrapping_add(0x10)));  // 40d381 fld    DWORD PTR [eax+0x10]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x8)));  // 40d384 fmul   DWORD PTR [eax+0x8]
    x.arith(Op::Sub, 1, 0, true);  // 40d387 fsubp  st(1),st
    x.fldst(2);  // 40d389 fld    st(2)
    x.arith_m(Op::Mul, m.rf(eax));  // 40d38b fmul   DWORD PTR [eax]
    x.fldst(2);  // 40d38d fld    st(2)
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0xc)));  // 40d38f fmul   DWORD PTR [eax+0xc]
    x.fldst(2);  // 40d392 fld    st(2)
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x18)));  // 40d394 fmul   DWORD PTR [eax+0x18]
    x.fxch(2);  // 40d397 fxch   st(2)
    x.arith(Op::Add, 1, 0, true);  // 40d399 faddp  st(1),st
    x.arith(Op::Add, 1, 0, true);  // 40d39b faddp  st(1),st
    x.fld1();  // 40d39d fld1
    x.arith(Op::DivR, 1, 0, true);  // 40d39f fdivrp st(1),st
    x.arith(Op::Mul, 3, 0, false);  // 40d3a1 fmul   st(3),st
    x.arith(Op::Mul, 2, 0, false);  // 40d3a3 fmul   st(2),st
    x.arith(Op::Mul, 1, 0, false);  // 40d3a5 fmul   st(1),st
    x.fxch(3);  // 40d3a7 fxch   st(3)
    m.wf(edx, x.pop());  // 40d3a9 fstp   DWORD PTR [edx]
    m.wf(edx.wrapping_add(0x8), x.pop());  // 40d3ab fstp   DWORD PTR [edx+0x8]
    m.wf(edx.wrapping_add(0x4), x.pop());  // 40d3ae fstp   DWORD PTR [edx+0x4]
    x.push(m.rf(eax.wrapping_add(0x18)));  // 40d3b1 fld    DWORD PTR [eax+0x18]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x14)));  // 40d3b4 fmul   DWORD PTR [eax+0x14]
    x.push(m.rf(eax.wrapping_add(0xc)));  // 40d3b7 fld    DWORD PTR [eax+0xc]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x20)));  // 40d3ba fmul   DWORD PTR [eax+0x20]
    x.push(m.rf(eax.wrapping_add(0xc)));  // 40d3bd fld    DWORD PTR [eax+0xc]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x1c)));  // 40d3c0 fmul   DWORD PTR [eax+0x1c]
    x.push(m.rf(eax.wrapping_add(0x18)));  // 40d3c3 fld    DWORD PTR [eax+0x18]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x10)));  // 40d3c6 fmul   DWORD PTR [eax+0x10]
    x.fxch(2);  // 40d3c9 fxch   st(2)
    x.arith(Op::Sub, 3, 0, true);  // 40d3cb fsubp  st(3),st
    x.arith(Op::SubR, 1, 0, true);  // 40d3cd fsubrp st(1),st
    x.fxch(1);  // 40d3cf fxch   st(1)
    x.arith(Op::Mul, 0, 2, false);  // 40d3d1 fmul   st,st(2)
    x.fxch(1);  // 40d3d3 fxch   st(1)
    x.arith(Op::Mul, 0, 2, false);  // 40d3d5 fmul   st,st(2)
    x.fxch(1);  // 40d3d7 fxch   st(1)
    m.wf(edx.wrapping_add(0xc), x.pop());  // 40d3d9 fstp   DWORD PTR [edx+0xc]
    m.wf(edx.wrapping_add(0x18), x.pop());  // 40d3dc fstp   DWORD PTR [edx+0x18]
    x.push(m.rf(eax));  // 40d3df fld    DWORD PTR [eax]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x20)));  // 40d3e1 fmul   DWORD PTR [eax+0x20]
    x.push(m.rf(eax.wrapping_add(0x18)));  // 40d3e4 fld    DWORD PTR [eax+0x18]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x8)));  // 40d3e7 fmul   DWORD PTR [eax+0x8]
    x.push(m.rf(eax.wrapping_add(0x18)));  // 40d3ea fld    DWORD PTR [eax+0x18]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x4)));  // 40d3ed fmul   DWORD PTR [eax+0x4]
    x.push(m.rf(eax));  // 40d3f0 fld    DWORD PTR [eax]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x1c)));  // 40d3f2 fmul   DWORD PTR [eax+0x1c]
    x.fxch(2);  // 40d3f5 fxch   st(2)
    x.arith(Op::Sub, 3, 0, true);  // 40d3f7 fsubp  st(3),st
    x.arith(Op::SubR, 1, 0, true);  // 40d3f9 fsubrp st(1),st
    x.fxch(1);  // 40d3fb fxch   st(1)
    x.arith(Op::Mul, 0, 2, false);  // 40d3fd fmul   st,st(2)
    x.fxch(1);  // 40d3ff fxch   st(1)
    x.arith(Op::Mul, 0, 2, false);  // 40d401 fmul   st,st(2)
    x.fxch(1);  // 40d403 fxch   st(1)
    m.wf(edx.wrapping_add(0x10), x.pop());  // 40d405 fstp   DWORD PTR [edx+0x10]
    m.wf(edx.wrapping_add(0x1c), x.pop());  // 40d408 fstp   DWORD PTR [edx+0x1c]
    x.push(m.rf(eax.wrapping_add(0xc)));  // 40d40b fld    DWORD PTR [eax+0xc]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x8)));  // 40d40e fmul   DWORD PTR [eax+0x8]
    x.push(m.rf(eax));  // 40d411 fld    DWORD PTR [eax]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x14)));  // 40d413 fmul   DWORD PTR [eax+0x14]
    x.push(m.rf(eax));  // 40d416 fld    DWORD PTR [eax]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x10)));  // 40d418 fmul   DWORD PTR [eax+0x10]
    x.push(m.rf(eax.wrapping_add(0xc)));  // 40d41b fld    DWORD PTR [eax+0xc]
    x.arith_m(Op::Mul, m.rf(eax.wrapping_add(0x4)));  // 40d41e fmul   DWORD PTR [eax+0x4]
    x.fxch(2);  // 40d421 fxch   st(2)
    x.arith(Op::Sub, 3, 0, true);  // 40d423 fsubp  st(3),st
    x.arith(Op::SubR, 1, 0, true);  // 40d425 fsubrp st(1),st
    x.fxch(1);  // 40d427 fxch   st(1)
    x.arith(Op::Mul, 0, 2, false);  // 40d429 fmul   st,st(2)
    x.fxch(1);  // 40d42b fxch   st(1)
    x.arith(Op::Mul, 2, 0, true);  // 40d42d fmulp  st(2),st
    m.wf(edx.wrapping_add(0x14), x.pop());  // 40d42f fstp   DWORD PTR [edx+0x14]
    m.wf(edx.wrapping_add(0x20), x.pop());  // 40d432 fstp   DWORD PTR [edx+0x20]
}

impl Scene {
    /// 0x40a680 (class 13 draw): time remap, hand the viewport to the 3D subtree root [+0x3c] and evaluate it (0x409b72)
    pub fn draw_3d(&mut self, esi: u32, edi: u32, t: i32) {
        let eax = (self.m.r32(esi + 0x34) << 6) as i32;
        let ebx = t.wrapping_sub(eax);
        let mut x = X87::new(); x.fild(ebx); x.arith_m(Op::Mul, self.m.rf(esi + 0x38));
        let ebx = fistp(x.pop());
        let node = self.m.r32(esi + 0x3c);
        self.m.w32(node + 0x3c, edi);
        self.vp3d = edi;
        self.eval_node(node, ebx);
    }
    /// node handlers of the 3D subtree: 0x40a6a5 (class 16 root), 0x408c22 (19), 0x4083f6 (22), 0x407278 (26), 0x40695b (32)
    pub(crate) fn eval_3d(&mut self, h: u32, esi: u32, t: i32) {
        match h {
            0x40a6a5 => {
                let mut ebx = t;
                let eax = (self.m.r32(esi + 0x24) << 6) as i32;
                if eax != 0 {
                    loop { ebx = ebx.wrapping_add(eax); if ebx >= 0 { break; } }
                    while !(ebx < eax) { ebx = ebx.wrapping_sub(eax); }
                }
                self.eval_children(esi, ebx);
                if self.stage3d >= 2 { self.camera_lists(esi, ebx); }
                if self.stage3d >= 3 { self.raster(esi, ebx); }
            }
            0x408c22 => self.eval_19(esi, t),
            0x4083f6 => {   // class 22 (light): 1/[+0xb4] -> +0xb8, flag 2, then the class-18 transform
                let m = &mut self.m;
                let mut x = X87::new(); x.push(m.rf(esi + 0xb4)); x.fld1(); x.arith(Op::DivR, 1, 0, true);
                m.wf(esi + 0xb8, x.pop());
                let v = m.r32(esi + 0xbc) | 2; m.w32(esi + 0xbc, v);
                self.call_eval(0x408bda, esi, t, 0);
            }
            0x407278 => {   // class 26
                self.eval_19(esi, t);
                let m = &mut self.m;
                let mut x = X87::new();
                x.fld1(); x.push(m.rf(esi + 0x168)); x.fldst(0); x.arith(Op::Div, 2, 0, true);
                x.arith(Op::Mul, 0, 0, false); x.fxch(1);
                m.wf(esi + 0x170, x.top());
                x.push(m.rf(esi + 0xf8)); x.arith(Op::Mul, 0, 1, false); m.wf(esi + 0x174, x.pop());
                x.fxch(1); m.wf(esi + 0x178, x.top());
                x.push(m.rf(esi + 0x16c)); x.arith(Op::Mul, 0, 0, false); m.wf(esi + 0x184, x.top());
                x.arith(Op::Add, 1, 0, true); x.sqrt(); m.wf(esi + 0x164, x.pop());
                let c = f32::from_bits(0x3f22f983);
                x.arith_m(Op::Mul, m.rf(esi + 0xe4)); m.wf(esi + 0x180, x.pop());
                x.push(c); x.arith_m(Op::Mul, m.rf(esi + 0xe0)); m.wf(esi + 0x17c, x.pop());
            }
            0x40695b => { let v = self.m.r32(esi + 0x1a8); self.m.w32(esi + 0x164, v); self.eval_19(esi, t); }
            _ => unreachable!(),
        }
    }
    /// 0x408c22 (class 19): class-18 transform, inverse matrix -> +0x130, packed words, colour/size words, shader record -> +0x160
    fn eval_19(&mut self, esi: u32, t: i32) {
        self.call_eval(0x408bda, esi, t, 0);
        let m = &mut self.m;
        mat_inv(m, esi + 0x1c, esi + 0x130);
        let eax = m.r32(esi + 0x114); let mut edx = m.r32(esi + 0x118).wrapping_add(eax);
        edx = edx.wrapping_add(eax >> 16);
        m.w16(esi + 0x11a, edx as u16);
        let a = m.r32(esi + 0x154).wrapping_add(0x4000000); let b = m.r32(esi + 0x158).wrapping_add(0x4000000);
        m.w32(esi + 0xe8, a); m.w32(esi + 0xec, b);
        let mut a = m.r32(esi + 0xfc); if a & 0xf8000000 != 0 { a = a.wrapping_sub(0x3800000); }
        m.w32(esi + 0x15c, a);
        let e = m.r32(esi + 0xdc);
        if e == 0 { let v = m.r32(esi + 0x18) & !1; m.w32(esi + 0x18, v); } else { let v = m.r32(e + 0x24); m.w32(esi + 0x160, v); }
    }
    pub(crate) fn raster(&mut self, _esi: u32, _t: i32) { unimplemented!() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::{load, times};
    pub const KS: [u32; 5] = [1, 2, 31, 100, 240];
    /// (scene at the first draw handler, esi = the class-13 node, edi = viewport)
    pub fn start(k: u32) -> (Scene, u32, u32) {
        let s = Scene::new(load(&format!("psnap{k}")).unwrap());
        let root = s.m.r32(s.ebp + 0x50); let vp = s.m.r32(root + 0x18);
        fn walk(m: &Mem, mut n: u32, out: &mut Vec<u32>) { while n != 0 { if m.r32(n + 0x30) == 0x40a680 { out.push(n); } let c = m.r32(n + 8); if c != 0 { walk(m, c, out); } n = m.r32(n + 4); } }
        let mut v = vec![]; walk(&s.m, vp, &mut v);
        (s, v[0], vp)
    }
    pub fn tk(k: u32) -> i32 { times().iter().find(|e| e.0 == k).unwrap().1 }
    /// compare against a stage dump of the original; returns the differing addresses (image + heap)
    pub fn diff_stage(s: &Scene, k: u32, tag: &str) -> Vec<u32> {
        let Some(o) = load(&format!("s3d_{k}_{tag}")) else { panic!("missing dump s3d_{k}_{tag}") };
        let mut d = vec![];
        for (i, (a, b)) in s.m.img.iter().zip(&o.img).enumerate() { if a != b { d.push(0x40_0000 + i as u32); } }
        for (i, (a, b)) in s.m.heap.iter().zip(&o.heap).enumerate() { if a != b { d.push(0x2000_0000 + i as u32); } }
        d
    }
    pub fn report(s: &Scene, k: u32, tag: &str, d: &[u32]) {
        println!("frame {k}: {} differing bytes", d.len());
        let mut last = 0; let mut shown = 0;
        for &a in d.iter().take(60) { println!("   {a:#x} rust {:#04x} orig {:#04x}", s.m.r8(a), load(&format!("s3d_{k}_{tag}")).unwrap().r8(a)); }
        for &a in d { if a > last + 16 || last == 0 { if shown < 12 { println!("   first diff of a run at {a:#x}"); } shown += 1; } last = a; }
    }
    #[test] fn lists_3d_matches() {
        for k in KS {
            let (mut s, node, vp) = start(k);
            if load(&format!("s3d_{k}_40a6c6")).is_none() { continue; }
            s.stage3d = 2; s.draw_3d(node, vp, tk(k));
            let d = diff_stage(&s, k, "40a6c6"); report(&s, k, "40a6c6", &d); assert!(d.is_empty());
        }
    }
    #[test] fn eval_3d_matches() {
        for k in KS {
            let (mut s, node, vp) = start(k);
            if load(&format!("s3d_{k}_40a6c1")).is_none() { continue; }
            s.stage3d = 1; s.draw_3d(node, vp, tk(k));
            let d = diff_stage(&s, k, "40a6c1"); report(&s, k, "40a6c1", &d); assert!(d.is_empty());
        }
    }
}
