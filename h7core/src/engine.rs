//! 3D pass stage 2: camera set-up, object lists and the per-line object marking (0x40af49 and the 0x4086xx..0x4089xx helpers), plus the object
//! vtable entries these call ([obj+0xa0] emit, [obj+0xb0] distance, [obj+0xb4] line test). Literal ports on the flat memory image; x87 sequences are
//! generated from the disassembly (re/x87gen.py) and run on `X87`. Registers of the original are plain local variables with the same names.
use crate::scene::*;
use crate::three_d::mat_inv;

pub const SCRATCH: u32 = STACK_BASE + 0x2000;   // stands in for the original's stack frames (esp)

/// 0x40182d: world point on the x87 stack (st0 = z, st1 = y, st2 = x) minus the object position, times the matrix at [esi+0x130..0x150]; three results on the stack
pub fn xform(m: &Mem, x: &mut X87, esi: u32) {
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x48)));  // 40182d fsub   DWORD PTR [esi+0x48]
x.fxch(1);  // 401830 fxch   st(1)
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x44)));  // 401832 fsub   DWORD PTR [esi+0x44]
x.fxch(2);  // 401835 fxch   st(2)
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x40)));  // 401837 fsub   DWORD PTR [esi+0x40]
x.push(m.rf(esi.wrapping_add(0x130)));  // 40183a fld    DWORD PTR [esi+0x130]
x.arith(Op::Mul, 0, 1, false);  // 401840 fmul   st,st(1)
x.push(m.rf(esi.wrapping_add(0x13c)));  // 401842 fld    DWORD PTR [esi+0x13c]
x.arith(Op::Mul, 0, 4, false);  // 401848 fmul   st,st(4)
x.push(m.rf(esi.wrapping_add(0x148)));  // 40184a fld    DWORD PTR [esi+0x148]
x.arith(Op::Mul, 0, 4, false);  // 401850 fmul   st,st(4)
x.fxch(2);  // 401852 fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 401854 faddp  st(1),st
x.push(m.rf(esi.wrapping_add(0x138)));  // 401856 fld    DWORD PTR [esi+0x138]
x.arith(Op::Mul, 0, 3, false);  // 40185c fmul   st,st(3)
x.push(m.rf(esi.wrapping_add(0x144)));  // 40185e fld    DWORD PTR [esi+0x144]
x.arith(Op::Mul, 0, 6, false);  // 401864 fmul   st,st(6)
x.fxch(2);  // 401866 fxch   st(2)
x.arith(Op::Add, 3, 0, true);  // 401868 faddp  st(3),st
x.push(m.rf(esi.wrapping_add(0x150)));  // 40186a fld    DWORD PTR [esi+0x150]
x.arith(Op::Mul, 0, 5, false);  // 401870 fmul   st,st(5)
x.fxch(2);  // 401872 fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 401874 faddp  st(1),st
x.push(m.rf(esi.wrapping_add(0x134)));  // 401876 fld    DWORD PTR [esi+0x134]
x.arith(Op::Mul, 4, 0, true);  // 40187c fmulp  st(4),st
x.push(m.rf(esi.wrapping_add(0x140)));  // 40187e fld    DWORD PTR [esi+0x140]
x.arith(Op::Mul, 6, 0, true);  // 401884 fmulp  st(6),st
x.arith(Op::Add, 1, 0, true);  // 401886 faddp  st(1),st
x.push(m.rf(esi.wrapping_add(0x14c)));  // 401888 fld    DWORD PTR [esi+0x14c]
x.arith(Op::Mul, 4, 0, true);  // 40188e fmulp  st(4),st
x.fxch(2);  // 401890 fxch   st(2)
x.arith(Op::Add, 3, 0, true);  // 401892 faddp  st(3),st
x.fxch(3);  // 401894 fxch   st(3)
x.arith(Op::Add, 2, 0, true);  // 401896 faddp  st(2),st
}
/// 0x40183a: the same without the three subtractions
pub fn xform_nosub(m: &Mem, x: &mut X87, esi: u32) {
x.push(m.rf(esi.wrapping_add(0x130)));  // 40183a fld    DWORD PTR [esi+0x130]
x.arith(Op::Mul, 0, 1, false);  // 401840 fmul   st,st(1)
x.push(m.rf(esi.wrapping_add(0x13c)));  // 401842 fld    DWORD PTR [esi+0x13c]
x.arith(Op::Mul, 0, 4, false);  // 401848 fmul   st,st(4)
x.push(m.rf(esi.wrapping_add(0x148)));  // 40184a fld    DWORD PTR [esi+0x148]
x.arith(Op::Mul, 0, 4, false);  // 401850 fmul   st,st(4)
x.fxch(2);  // 401852 fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 401854 faddp  st(1),st
x.push(m.rf(esi.wrapping_add(0x138)));  // 401856 fld    DWORD PTR [esi+0x138]
x.arith(Op::Mul, 0, 3, false);  // 40185c fmul   st,st(3)
x.push(m.rf(esi.wrapping_add(0x144)));  // 40185e fld    DWORD PTR [esi+0x144]
x.arith(Op::Mul, 0, 6, false);  // 401864 fmul   st,st(6)
x.fxch(2);  // 401866 fxch   st(2)
x.arith(Op::Add, 3, 0, true);  // 401868 faddp  st(3),st
x.push(m.rf(esi.wrapping_add(0x150)));  // 40186a fld    DWORD PTR [esi+0x150]
x.arith(Op::Mul, 0, 5, false);  // 401870 fmul   st,st(5)
x.fxch(2);  // 401872 fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 401874 faddp  st(1),st
x.push(m.rf(esi.wrapping_add(0x134)));  // 401876 fld    DWORD PTR [esi+0x134]
x.arith(Op::Mul, 4, 0, true);  // 40187c fmulp  st(4),st
x.push(m.rf(esi.wrapping_add(0x140)));  // 40187e fld    DWORD PTR [esi+0x140]
x.arith(Op::Mul, 6, 0, true);  // 401884 fmulp  st(6),st
x.arith(Op::Add, 1, 0, true);  // 401886 faddp  st(1),st
x.push(m.rf(esi.wrapping_add(0x14c)));  // 401888 fld    DWORD PTR [esi+0x14c]
x.arith(Op::Mul, 4, 0, true);  // 40188e fmulp  st(4),st
x.fxch(2);  // 401890 fxch   st(2)
x.arith(Op::Add, 3, 0, true);  // 401892 faddp  st(3),st
x.fxch(3);  // 401894 fxch   st(3)
x.arith(Op::Add, 2, 0, true);  // 401896 faddp  st(2),st
}
/// 0x401899: rotation by the matrix at [esi+0x1c..0x40]
pub fn rot(m: &Mem, x: &mut X87, esi: u32) {
x.push(m.rf(esi.wrapping_add(0x1c)));  // 401899 fld    DWORD PTR [esi+0x1c]
x.arith(Op::Mul, 0, 1, false);  // 40189c fmul   st,st(1)
x.push(m.rf(esi.wrapping_add(0x28)));  // 40189e fld    DWORD PTR [esi+0x28]
x.arith(Op::Mul, 0, 4, false);  // 4018a1 fmul   st,st(4)
x.push(m.rf(esi.wrapping_add(0x34)));  // 4018a3 fld    DWORD PTR [esi+0x34]
x.arith(Op::Mul, 0, 4, false);  // 4018a6 fmul   st,st(4)
x.fxch(2);  // 4018a8 fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 4018aa faddp  st(1),st
x.push(m.rf(esi.wrapping_add(0x24)));  // 4018ac fld    DWORD PTR [esi+0x24]
x.arith(Op::Mul, 0, 3, false);  // 4018af fmul   st,st(3)
x.push(m.rf(esi.wrapping_add(0x30)));  // 4018b1 fld    DWORD PTR [esi+0x30]
x.arith(Op::Mul, 0, 6, false);  // 4018b4 fmul   st,st(6)
x.fxch(2);  // 4018b6 fxch   st(2)
x.arith(Op::Add, 3, 0, true);  // 4018b8 faddp  st(3),st
x.push(m.rf(esi.wrapping_add(0x3c)));  // 4018ba fld    DWORD PTR [esi+0x3c]
x.arith(Op::Mul, 0, 5, false);  // 4018bd fmul   st,st(5)
x.fxch(2);  // 4018bf fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 4018c1 faddp  st(1),st
x.push(m.rf(esi.wrapping_add(0x20)));  // 4018c3 fld    DWORD PTR [esi+0x20]
x.arith(Op::Mul, 4, 0, true);  // 4018c6 fmulp  st(4),st
x.push(m.rf(esi.wrapping_add(0x2c)));  // 4018c8 fld    DWORD PTR [esi+0x2c]
x.arith(Op::Mul, 6, 0, true);  // 4018cb fmulp  st(6),st
x.arith(Op::Add, 1, 0, true);  // 4018cd faddp  st(1),st
x.push(m.rf(esi.wrapping_add(0x38)));  // 4018cf fld    DWORD PTR [esi+0x38]
x.arith(Op::Mul, 4, 0, true);  // 4018d2 fmulp  st(4),st
x.fxch(2);  // 4018d4 fxch   st(2)
x.arith(Op::Add, 3, 0, true);  // 4018d6 faddp  st(3),st
x.fxch(3);  // 4018d8 fxch   st(3)
x.arith(Op::Add, 2, 0, true);  // 4018da faddp  st(2),st
}
/// 0x4018dd: ray point: [edi+0x20..0x28] = origin [edi..8] + direction [edi+0xc..0x14] * t [edi+0x18]
pub fn ray_point(m: &mut Mem, edi: u32) {
    let mut x = X87::new();
x.push(m.rf(edi.wrapping_add(0xc)));  // 4018dd fld    DWORD PTR [edi+0xc]
x.push(m.rf(edi.wrapping_add(0x10)));  // 4018e0 fld    DWORD PTR [edi+0x10]
x.push(m.rf(edi.wrapping_add(0x14)));  // 4018e3 fld    DWORD PTR [edi+0x14]
x.push(m.rf(edi.wrapping_add(0x18)));  // 4018e6 fld    DWORD PTR [edi+0x18]
x.arith(Op::Mul, 1, 0, false);  // 4018e9 fmul   st(1),st
x.arith(Op::Mul, 2, 0, false);  // 4018eb fmul   st(2),st
x.arith(Op::Mul, 3, 0, true);  // 4018ed fmulp  st(3),st
x.push(m.rf(edi));  // 4018ef fld    DWORD PTR [edi]
x.push(m.rf(edi.wrapping_add(0x4)));  // 4018f1 fld    DWORD PTR [edi+0x4]
x.push(m.rf(edi.wrapping_add(0x8)));  // 4018f4 fld    DWORD PTR [edi+0x8]
x.arith(Op::Add, 3, 0, true);  // 4018f7 faddp  st(3),st
x.arith(Op::Add, 3, 0, true);  // 4018f9 faddp  st(3),st
x.arith(Op::Add, 3, 0, true);  // 4018fb faddp  st(3),st
m.wf(edi.wrapping_add(0x28), x.pop());  // 4018fd fstp   DWORD PTR [edi+0x28]
m.wf(edi.wrapping_add(0x24), x.pop());  // 401900 fstp   DWORD PTR [edi+0x24]
m.wf(edi.wrapping_add(0x20), x.pop());  // 401903 fstp   DWORD PTR [edi+0x20]
}
/// 0x40d0f4: normalise the vec3 at eax in place (zero vectors untouched)
pub fn v_norm(m: &mut Mem, eax: u32) {
    let mut x = X87::new();
x.push(m.rf(eax));  // 40d0f5 fld    DWORD PTR [eax]
x.arith(Op::Mul, 0, 0, false);  // 40d0f7 fmul   st,st(0)
x.push(m.rf(eax.wrapping_add(0x4)));  // 40d0f9 fld    DWORD PTR [eax+0x4]
x.arith(Op::Mul, 0, 0, false);  // 40d0fc fmul   st,st(0)
x.push(m.rf(eax.wrapping_add(0x8)));  // 40d0fe fld    DWORD PTR [eax+0x8]
x.arith(Op::Mul, 0, 0, false);  // 40d101 fmul   st,st(0)
x.fxch(2);  // 40d103 fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 40d105 faddp  st(1),st
x.arith(Op::Add, 1, 0, true);  // 40d107 faddp  st(1),st
x.sqrt();  // 40d109 fsqrt
    let l = x.top();
    if l.to_bits() & 0x7fff_ffff == 0 { return; }
x.fld1();  // 40d117 fld1
x.arith(Op::DivR, 1, 0, true);  // 40d119 fdivrp st(1),st
x.push(m.rf(eax));  // 40d11b fld    DWORD PTR [eax]
x.arith(Op::Mul, 0, 1, false);  // 40d11d fmul   st,st(1)
x.push(m.rf(eax.wrapping_add(0x4)));  // 40d11f fld    DWORD PTR [eax+0x4]
x.arith(Op::Mul, 0, 2, false);  // 40d122 fmul   st,st(2)
x.push(m.rf(eax.wrapping_add(0x8)));  // 40d124 fld    DWORD PTR [eax+0x8]
x.arith(Op::Mul, 3, 0, true);  // 40d127 fmulp  st(3),st
x.fxch(1);  // 40d129 fxch   st(1)
m.wf(eax, x.pop());  // 40d12b fstp   DWORD PTR [eax]
m.wf(eax.wrapping_add(0x4), x.pop());  // 40d12d fstp   DWORD PTR [eax+0x4]
m.wf(eax.wrapping_add(0x8), x.pop());  // 40d130 fstp   DWORD PTR [eax+0x8]
}
/// 0x40d198: ebx = eax x edx
pub fn v_cross(m: &mut Mem, eax: u32, edx: u32, ebx: u32) {
    let mut x = X87::new();
x.push(m.rf(eax));  // 40d198 fld    DWORD PTR [eax]
x.push(m.rf(eax.wrapping_add(0x4)));  // 40d19a fld    DWORD PTR [eax+0x4]
x.fldst(0);  // 40d19d fld    st(0)
x.push(m.rf(edx.wrapping_add(0x8)));  // 40d19f fld    DWORD PTR [edx+0x8]
x.arith(Op::Mul, 1, 0, false);  // 40d1a2 fmul   st(1),st
x.arith(Op::Mul, 0, 3, false);  // 40d1a4 fmul   st,st(3)
x.push(m.rf(eax.wrapping_add(0x8)));  // 40d1a6 fld    DWORD PTR [eax+0x8]
x.push(m.rf(edx.wrapping_add(0x4)));  // 40d1a9 fld    DWORD PTR [edx+0x4]
x.push(m.rf(edx));  // 40d1ac fld    DWORD PTR [edx]
x.arith(Op::Mul, 5, 0, false);  // 40d1ae fmul   st(5),st
x.arith(Op::Mul, 0, 2, false);  // 40d1b0 fmul   st,st(2)
x.fxch(1);  // 40d1b2 fxch   st(1)
x.arith(Op::Mul, 2, 0, false);  // 40d1b4 fmul   st(2),st
x.arith(Op::Mul, 6, 0, true);  // 40d1b6 fmulp  st(6),st
x.arith(Op::SubR, 2, 0, true);  // 40d1b8 fsubrp st(2),st
x.arith(Op::Sub, 2, 0, true);  // 40d1ba fsubp  st(2),st
x.fxch(2);  // 40d1bc fxch   st(2)
x.arith(Op::Sub, 3, 0, true);  // 40d1be fsubp  st(3),st
m.wf(ebx, x.pop());  // 40d1c0 fstp   DWORD PTR [ebx]
m.wf(ebx.wrapping_add(0x4), x.pop());  // 40d1c2 fstp   DWORD PTR [ebx+0x4]
m.wf(ebx.wrapping_add(0x8), x.pop());  // 40d1c5 fstp   DWORD PTR [ebx+0x8]
}
/// 0x40d17b: [ebx] = eax . edx
pub fn v_dot(m: &mut Mem, eax: u32, edx: u32, ebx: u32) {
    let mut x = X87::new();
x.push(m.rf(eax));  // 40d17b fld    DWORD PTR [eax]
x.push(m.rf(eax.wrapping_add(0x4)));  // 40d17d fld    DWORD PTR [eax+0x4]
x.push(m.rf(eax.wrapping_add(0x8)));  // 40d180 fld    DWORD PTR [eax+0x8]
x.push(m.rf(edx));  // 40d183 fld    DWORD PTR [edx]
x.push(m.rf(edx.wrapping_add(0x4)));  // 40d185 fld    DWORD PTR [edx+0x4]
x.push(m.rf(edx.wrapping_add(0x8)));  // 40d188 fld    DWORD PTR [edx+0x8]
x.arith(Op::Mul, 3, 0, true);  // 40d18b fmulp  st(3),st
x.arith(Op::Mul, 3, 0, true);  // 40d18d fmulp  st(3),st
x.arith(Op::Mul, 3, 0, true);  // 40d18f fmulp  st(3),st
x.arith(Op::Add, 1, 0, true);  // 40d191 faddp  st(1),st
x.arith(Op::Add, 1, 0, true);  // 40d193 faddp  st(1),st
m.wf(ebx, x.pop());  // 40d195 fstp   DWORD PTR [ebx]
}
/// 0x40d13d: eax += edx (vec3, in place)
pub fn v_add_in(m: &mut Mem, eax: u32, edx: u32) {
    let mut x = X87::new();
x.push(m.rf(eax));  // 40d13d fld    DWORD PTR [eax]
x.push(m.rf(eax.wrapping_add(0x4)));  // 40d13f fld    DWORD PTR [eax+0x4]
x.push(m.rf(eax.wrapping_add(0x8)));  // 40d142 fld    DWORD PTR [eax+0x8]
x.push(m.rf(edx));  // 40d145 fld    DWORD PTR [edx]
x.push(m.rf(edx.wrapping_add(0x4)));  // 40d147 fld    DWORD PTR [edx+0x4]
x.push(m.rf(edx.wrapping_add(0x8)));  // 40d14a fld    DWORD PTR [edx+0x8]
x.arith(Op::Add, 3, 0, true);  // 40d14d faddp  st(3),st
x.arith(Op::Add, 3, 0, true);  // 40d14f faddp  st(3),st
x.arith(Op::Add, 3, 0, true);  // 40d151 faddp  st(3),st
m.wf(eax.wrapping_add(0x8), x.pop());  // 40d153 fstp   DWORD PTR [eax+0x8]
m.wf(eax.wrapping_add(0x4), x.pop());  // 40d156 fstp   DWORD PTR [eax+0x4]
m.wf(eax, x.pop());  // 40d159 fstp   DWORD PTR [eax]
}

impl Scene {
    /// 0x408a4e: table of 16-byte entries (ray direction components) for a viewport node (esi): 1/sqrt(u^2 + v^2 + 1) per lattice point, mirrored into four quadrants
    fn build_tables(&mut self, esi: u32) {
        let m = &mut self.m;
        let mut edi = m.r32(esi + 0x3c);
        let w = m.r32(esi + 0x18); let h = m.r32(esi + 0x1c);
        let s1 = (w >> 1) + 1;                      // [esp+4]
        let s2 = (w + 1) << 4;                      // [esp+8]
        let mut ebx = s2.wrapping_mul(h).wrapping_add(edi);
        let mut s0 = h >> 1;                        // [esp]
        let mut x = X87::new();
        x.push(m.rf(esi + 0x50)); x.ptan(); x.pop();
        x.fldst(0); m.wf(esi + 0x58, x.top()); x.fxch(1); x.arith(Op::Add, 0, 0, false);
        x.arith_m(Op::Div, m.r32(esi + 0x18) as i32 as f32); m.wf(esi + 0x54, x.top());
        x.fild(s0 as i32); x.arith(Op::Mul, 0, 1, false);
        s0 += 1;
        m.wf(esi + 0x5c, x.top());
        loop {
            let mut edx = m.r32(esi + 0x18) << 4;
            let mut ecx = s1;
            let mut ebp = edi.wrapping_add(edx); edx = ebx.wrapping_add(edx);
            x.fldst(0); x.arith(Op::Mul, 0, 0, false); x.fld1(); x.arith(Op::Add, 1, 0, true);
            x.fldst(3);
            let (sbx, sdi) = (ebx, edi);
            loop {
                x.fldst(0); x.arith(Op::Mul, 0, 0, false); x.arith(Op::Add, 0, 2, false); x.sqrt(); x.fld1(); x.arith(Op::DivR, 1, 0, true);
                edi = edi.wrapping_add(0x10); ebp = ebp.wrapping_sub(0x10); ebx = ebx.wrapping_add(0x10); edx = edx.wrapping_sub(0x10); ecx -= 1;
                m.wf(edi.wrapping_sub(0xc), x.top());
                m.wf(ebp.wrapping_add(0x14), x.top());
                m.wf(ebx.wrapping_sub(0xc), x.top());
                m.wf(edx.wrapping_add(0x14), x.pop());
                x.arith(Op::Sub, 0, 3, false);
                if ecx == 0 { break; }
            }
            edi = sdi; ebx = sbx;
            ebx = ebx.wrapping_sub(s2); edi = edi.wrapping_add(s2);
            x.pop(); x.pop();
            x.arith(Op::Sub, 0, 1, false);
            s0 -= 1;
            if s0 == 0 { break; }
        }
    }
    /// 0x408af1 (esi = viewport node, eax = camera fov): rebuild the tables when the fov changed, copy the viewport node (0x60 bytes) to ebp+0x80, and write the 4 doubling size groups at ebp+0x198
    fn cam_tables(&mut self, esi: u32, eax: u32) {
        let ebp = self.ebp;
        if eax != self.m.r32(esi + 0x50) { self.m.w32(esi + 0x50, eax); self.build_tables(esi); }
        for i in 0..0x18u32 { let v = self.m.r32(esi + 4 * i); self.m.w32(ebp + 0x80 + 4 * i, v); }
        let m = &mut self.m;
        let mut ebx = (m.r32(ebp + 0x98) + 1).wrapping_mul(0x80);
        let (mut eax, mut edi, mut edx) = (0x80u32, 4u32, m.r32(ebp + 0xa0));
        for g in 0..4u32 {
            let b = ebp + 0x198 + 0x10 * g;
            m.w32(b, eax); eax = eax.wrapping_add(eax);
            m.w32(b + 4, ebx); ebx = ebx.wrapping_add(ebx);
            m.w32(b + 8, edi); edi = edi.wrapping_add(edi);
            m.w32(b + 0xc, edx); edx = edx.wrapping_add(edx);
        }
    }
    /// 0x408b54 (esi = camera object): basis vectors from the camera angle [+0xa8] and the target object [+0xa0]: +0xc4 = normalised direction to the target, +0xac = normalised (sin, -cos, 0) x +0xc4... (see code)
    fn cam_basis(&mut self, esi: u32) {
        let m = &mut self.m;
        let mut x = X87::new();
        let edx = SCRATCH; let ebx = esi + 0xac; let eax = esi + 0xc4;
        m.w32(edx + 8, 0);
x.push(m.rf(esi.wrapping_add(0xa8)));  // 408b58 fld    DWORD PTR [esi+0xa8]
x.sincos();  // 408b5e fsincos
x.fchs();  // 408b6f fchs
m.wf(edx.wrapping_add(0x4), x.pop());  // 408b78 fstp   DWORD PTR [edx+0x4]
        m.wf(edx, x.pop());
        let edi = m.r32(esi + 0xa0);
x.push(m.rf(edi.wrapping_add(0x40)));  // 408b83 fld    DWORD PTR [edi+0x40]
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x40)));  // 408b86 fsub   DWORD PTR [esi+0x40]
x.push(m.rf(edi.wrapping_add(0x44)));  // 408b89 fld    DWORD PTR [edi+0x44]
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x44)));  // 408b8c fsub   DWORD PTR [esi+0x44]
x.push(m.rf(edi.wrapping_add(0x48)));  // 408b8f fld    DWORD PTR [edi+0x48]
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x48)));  // 408b92 fsub   DWORD PTR [esi+0x48]
x.fxch(2);  // 408b95 fxch   st(2)
m.wf(eax, x.pop());  // 408b97 fstp   DWORD PTR [eax]
m.wf(eax.wrapping_add(0x4), x.pop());  // 408b99 fstp   DWORD PTR [eax+0x4]
m.wf(eax.wrapping_add(0x8), x.pop());  // 408b9c fstp   DWORD PTR [eax+0x8]
        v_norm(m, eax);
        v_cross(m, eax, edx, ebx);
        let (edx, eax) = (eax, ebx);                // mov edx,eax; xchg ebx,eax
        let ebx = esi + 0xb8;
        v_norm(m, eax);
        v_cross(m, eax, edx, ebx);
    }
    /// 0x40885d: record list {obj, key, 0xc} for the objects of the list `esi` (+0x12c links) at [ebp+0x124], terminated by {0, 0x4e800001, 0xc}; returns the start
    fn emit_plain(&mut self, mut esi: u32) -> u32 {
        let (m, ebp) = (&mut self.m, self.ebp);
        let start = m.r32(ebp + 0x124); let mut ebx = start;
        while esi != 0 { let k = m.r32(esi + 0x128); m.w32(ebx, esi); m.w32(ebx + 4, k); m.w32(ebx + 8, 0xc); ebx += 0xc; esi = m.r32(esi + 0x12c); }
        m.w32(ebx, 0); m.w32(ebx + 4, 0x4e80_0001); m.w32(ebx + 8, 0xc); ebx += 0xc; m.w32(ebp + 0x124, ebx);
        start
    }
    /// 0x408814 (esi = list, edi = point): records {obj, key, size, data...} with the data written by the object's [+0xa0] entry; returns the start
    fn emit_list(&mut self, mut esi: u32, edi: u32) -> u32 {
        let ebp = self.ebp;
        let start = self.m.r32(ebp + 0x124); let mut ebx = start;
        while esi != 0 {
            let k = self.m.r32(esi + 0x128); self.m.w32(ebx, esi); self.m.w32(ebx + 4, k);
            let h = self.m.r32(esi + 0xa0);
            let eax = self.vt_a0(h, esi, edi, ebx);
            self.m.w32(ebx + 8, eax); ebx = ebx.wrapping_add(eax); esi = self.m.r32(esi + 0x12c);
        }
        let m = &mut self.m;
        m.w32(ebx, 0); m.w32(ebx + 4, 0x4e80_0001); m.w32(ebx + 8, 0xc); ebx += 0xc; m.w32(ebp + 0x124, ebx);
        start
    }
    /// insert `esi` (key `eax` = float bits compared as signed ints) into the list whose head cell is `cell` (links at +0x12c, key at +0x128)
    fn sorted_insert(&mut self, cell: u32, esi: u32, eax: u32) {
        let m = &mut self.m; let mut ebx = cell;
        loop {
            let edx = m.r32(ebx);
            if edx == 0 || (m.r32(edx + 0x128) as i32) > (eax as i32) { m.w32(esi + 0x12c, edx); m.w32(ebx, esi); return; }
            ebx = edx + 0x12c;
        }
    }
    /// 0x408699: every drawable object of the tree under `esi` (not the target `edx` itself unless [edx+0xd0] & 0x40, mask test [esi+0xcc] & [edx+0xd0]) sorted by distance from the target
    /// position, key = distance - [edx+0x164]  (clamped to 0)
    fn list_occluders(&mut self, mut esi: u32, edx: u32, cell: u32) {
        while esi != 0 {
            if self.m.r8(esi) != 0 {
                if self.m.r8(esi + 1) & 4 != 0 && self.m.r8(esi + 0x18) & 0x20 == 0 {
                    let tf = self.m.r32(edx + 0xd0);
                    let skip = (tf & 0x40 == 0 && esi == edx) || (self.m.r32(esi + 0xcc) & tf) != 0;
                    if !skip {
                        let ecx = edx + 0x40;
                        let mut eax = self.m.r32(esi + 0xd4);
                        if eax == 0 { eax = self.vt_b0(self.m.r32(esi + 0xb0), esi, ecx).0; }
                        if (eax as i32) > (self.m.r32(edx + 0x164) as i32) {
                            let mut x = X87::new(); x.push(f32::from_bits(eax)); x.arith_m(Op::Sub, self.m.rf(edx + 0x164)); eax = x.pop().to_bits();
                            self.m.w32(esi + 0x128, eax);
                        } else { self.m.w32(esi + 0x128, 0); eax = 0; }
                        self.sorted_insert(cell, esi, eax);
                    }
                }
                let c = self.m.r32(esi + 8); if c != 0 { self.list_occluders(c, edx, cell); }
            }
            esi = self.m.r32(esi + 4);
        }
    }
    /// 0x408758 (all drawable objects sorted by their [+0xb0] distance from the point ecx) / 0x4087b7 (same, skipping objects whose [+0xc8] & edx; no [+0xd4] shortcut)
    fn list_all(&mut self, mut esi: u32, cell: u32, ecx: u32, mask: Option<u32>) {
        while esi != 0 {
            if self.m.r8(esi) != 0 {
                if self.m.r8(esi + 1) & 4 != 0 && self.m.r8(esi + 0x18) & 0x20 == 0 && mask.map_or(true, |e| self.m.r32(esi + 0xc8) & e == 0) {
                    let mut eax = if mask.is_none() { self.m.r32(esi + 0xd4) } else { 0 };
                    if eax == 0 { eax = self.vt_b0(self.m.r32(esi + 0xb0), esi, ecx).0; }
                    self.m.w32(esi + 0x128, eax);
                    self.sorted_insert(cell, esi, eax);
                }
                let c = self.m.r32(esi + 8); if c != 0 { self.list_all(c, cell, ecx, mask); }
            }
            esi = self.m.r32(esi + 4);
        }
    }
    /// 0x40aea9: walk the object tree: lights (+1 & 8) onto the list [ebp+0x78] (+0xcc links), drawable objects (+1 & 4, +0x18 & 0xc) get their sorted occluder record list ([+0x120]),
    /// the camera (+1 & 0x10) -> [ebp+0x64], others (+1 & 0x40) onto [ebp+0x130] (+0xa4 links)
    fn walk_objects(&mut self, mut esi: u32) {
        let ebp = self.ebp;
        while esi != 0 {
            if self.m.r8(esi) != 0 {
                let f = self.m.r8(esi + 1);
                if f & 8 != 0 {
                    let one = 0x3f80_0000i32;
                    if (self.m.r32(esi + 0xa4) as i32) > one || (self.m.r32(esi + 0xa8) as i32) > one || (self.m.r32(esi + 0xac) as i32) > one {
                        let old = self.m.r32(ebp + 0x78); self.m.w32(ebp + 0x78, esi); self.m.w32(esi + 0xcc, old);
                    }
                } else if f & 4 != 0 {
                    if self.m.r8(esi + 0x18) & 0xc != 0 {
                        let cell = SCRATCH + 0x100; self.m.w32(cell, 0);
                        let root = self.m.r32(ebp + 0x60);
                        self.list_occluders(root, esi, cell);
                        let head = self.m.r32(cell);
                        let a = self.emit_plain(head);
                        self.m.w32(esi + 0x120, a);
                    }
                } else if f & 0x10 != 0 { self.m.w32(ebp + 0x64, esi); }
                else if f & 0x40 != 0 { let old = self.m.r32(ebp + 0x130); self.m.w32(ebp + 0x130, esi); self.m.w32(esi + 0xa4, old); }
                let c = self.m.r32(esi + 8); if c != 0 { self.walk_objects(c); }
            }
            esi = self.m.r32(esi + 4);
        }
    }
    /// append the record `ecx` to the object list of line `line` (0x200-byte line records: count at +0x1c, entries from +0x20, at most 0x6f)
    fn line_add(&mut self, line: u32, ecx: u32) {
        let m = &mut self.m; let n = m.r32(line + 0x1c);
        m.w32(line + n.wrapping_mul(4) + 0x20, ecx);
        if n != 0x6e { m.w32(line + 0x1c, n + 1); }
    }
    /// 0x4089b0: record onto every line from `ebx` to `edx` inclusive
    fn line_range(&mut self, mut ebx: u32, edx: u32, ecx: u32) { loop { self.line_add(ebx, ecx); ebx += 0x200; if ebx > edx { break; } } }
    /// 0x4088a5 ([obj+0xbc]): mark the lines (columns, then rows) the object of record `ecx` touches
    fn mark_lines(&mut self, esi: u32, ecx: u32) {
        let ebp = self.ebp;
        for (n, base) in [(0x98u32, 0xc4u32), (0x9c, 0xc8)] {
            let edx = self.m.r32(ebp + base); let ebx = (self.m.r32(ebp + n) << 9).wrapping_add(edx);
            self.mark_pass(esi, ecx, ebx, edx);
        }
    }
    /// 0x4088d2: ebx = last line, edx = first line; the object's [+0xb4] line test returns (eax = signed distance bits, CF = touches the line)
    fn mark_pass(&mut self, esi: u32, ecx: u32, mut ebx: u32, mut edx: u32) {
        let hb4 = self.m.r32(esi + 0xb4);
        let (eax, cf) = self.vt_b4(hb4, esi, edx);
        let mut edi = cf as u32;
        if edi == 0 && (eax >> 31) != 0 { return; }
        std::mem::swap(&mut ebx, &mut edx);
        let (eax, cf) = self.vt_b4(hb4, esi, edx);
        edi = (edi << 1) | cf as u32;
        if edi == 0 && (eax >> 31) == 0 { return; }
        match edi {
            0 => { self.mark_900(esi, ecx, hb4, ebx, edx); }
            1 => { self.mark_970(esi, ecx, hb4, ebx, edx); }
            2 => { self.mark_93a(esi, ecx, hb4, ebx, edx); }
            _ => { self.line_range(ebx, edx, ecx); }
        }
    }
    /// 0x408900
    fn mark_900(&mut self, esi: u32, ecx: u32, hb4: u32, mut ebx: u32, mut edx: u32) {
        loop {
            let mut edi = edx.wrapping_sub(ebx) >> 10;
            if edi == 0 { return; }
            edi <<= 9; edx = edx.wrapping_sub(edi);
            let (eax, cf) = self.vt_b4(hb4, esi, edx);
            if cf {
                self.mark_970(esi, ecx, hb4, ebx, edx);        // push ebx, edx, edi; call 408970
                ebx = edx.wrapping_add(0x200); edx = edx.wrapping_add(edi);
                self.mark_93a(esi, ecx, hb4, ebx, edx); return;
            }
            if (eax >> 31) == 0 { ebx = edx; edx = edx.wrapping_add(edi); }
        }
    }
    /// 0x40893a
    fn mark_93a(&mut self, esi: u32, ecx: u32, hb4: u32, mut ebx: u32, mut edx: u32) {
        loop {
            let mut edi = edx.wrapping_sub(ebx) >> 10;
            if edi == 0 { self.line_add(ebx, ecx); return; }
            edi <<= 9; edx = edx.wrapping_sub(edi);
            let (_, cf) = self.vt_b4(hb4, esi, edx);
            if !cf { continue; }
            self.line_range(ebx, edx, ecx);
            ebx = edx.wrapping_add(0x200); edx = edx.wrapping_add(edi);
        }
    }
    /// 0x408970
    fn mark_970(&mut self, esi: u32, ecx: u32, hb4: u32, mut ebx: u32, mut edx: u32) {
        loop {
            let mut edi = edx.wrapping_sub(ebx) >> 10;
            if edi == 0 { self.line_add(edx, ecx); return; }
            edi <<= 9; edx = edx.wrapping_sub(edi);
            let (_, cf) = self.vt_b4(hb4, esi, edx);
            if cf {
                let sb = ebx; ebx = edx; edx = edx.wrapping_add(edi);
                self.line_range(ebx, edx, ecx);
                edx = ebx.wrapping_sub(0x200); ebx = sb;
            } else { ebx = edx; edx = edx.wrapping_add(edi); }
        }
    }

    /// [obj+0xa0]: write the object's record data at ebx+0xc.. (edi = point); returns the record size
    fn vt_a0(&mut self, h: u32, esi: u32, edi: u32, ebx: u32) -> u32 {
        let m = &mut self.m;
        let mut x = X87::new();
        match h {
            0x406ba3 => {
x.push(m.rf(edi));  // 406ba3 fld    DWORD PTR [edi]
x.push(m.rf(edi.wrapping_add(0x4)));  // 406ba5 fld    DWORD PTR [edi+0x4]
x.push(m.rf(edi.wrapping_add(0x8)));  // 406ba8 fld    DWORD PTR [edi+0x8]
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x48)));  // 406bab fsub   DWORD PTR [esi+0x48]
x.fxch(1);  // 406bae fxch   st(1)
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x44)));  // 406bb0 fsub   DWORD PTR [esi+0x44]
x.fxch(2);  // 406bb3 fxch   st(2)
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x40)));  // 406bb5 fsub   DWORD PTR [esi+0x40]
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0x134)));  // 406bb8 fmul   DWORD PTR [esi+0x134]
x.fxch(1);  // 406bbe fxch   st(1)
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0x14c)));  // 406bc0 fmul   DWORD PTR [esi+0x14c]
x.fxch(2);  // 406bc6 fxch   st(2)
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0x140)));  // 406bc8 fmul   DWORD PTR [esi+0x140]
x.fxch(2);  // 406bce fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 406bd0 faddp  st(1),st
x.arith(Op::Add, 1, 0, true);  // 406bd2 faddp  st(1),st
x.fchs();  // 406bd4 fchs
m.wf(ebx.wrapping_add(0xc), x.pop());  // 406bd6 fstp   DWORD PTR [ebx+0xc]
                0x10
            }
            0x406fd0 => {
x.push(m.rf(edi));  // 406fd0 fld    DWORD PTR [edi]
x.push(m.rf(edi.wrapping_add(0x4)));  // 406fd2 fld    DWORD PTR [edi+0x4]
x.push(m.rf(edi.wrapping_add(0x8)));  // 406fd5 fld    DWORD PTR [edi+0x8]
                xform(m, &mut x, esi);
                x.fchs();
                let t = x.top(); m.wf(ebx + 0x20, t);
                let r = if t.to_bits() >> 31 == 0 { m.rf(esi + 0x170) } else { m.rf(esi + 0x170) };
                x.arith_m(if t.to_bits() >> 31 == 0 { Op::Sub } else { Op::Add }, r); m.wf(ebx + 0x1c, x.pop());
                x.fchs();
                let t = x.top(); m.wf(ebx + 0x10, t);
                x.arith_m(if t.to_bits() >> 31 == 0 { Op::Sub } else { Op::Add }, m.rf(esi + 0x16c)); m.wf(ebx + 0xc, x.pop());
                x.fchs();
                let t = x.top(); m.wf(ebx + 0x18, t);
                x.arith_m(if t.to_bits() >> 31 == 0 { Op::Sub } else { Op::Add }, m.rf(esi + 0x168)); m.wf(ebx + 0x14, x.pop());
                0x24
            }
            0x40747e => {
x.push(m.rf(edi));  // 40747e fld    DWORD PTR [edi]
x.push(m.rf(edi.wrapping_add(0x4)));  // 407480 fld    DWORD PTR [edi+0x4]
x.push(m.rf(edi.wrapping_add(0x8)));  // 407483 fld    DWORD PTR [edi+0x8]
                xform(m, &mut x, esi);
x.fldst(0);  // 40748b fld    st(0)
x.arith(Op::Mul, 0, 0, false);  // 40748d fmul   st,st(0)
x.fldst(3);  // 40748f fld    st(3)
x.arith(Op::Mul, 0, 0, false);  // 407491 fmul   st,st(0)
x.fxch(3);  // 407493 fxch   st(3)
m.wf(ebx.wrapping_add(0x18), x.pop());  // 407495 fstp   DWORD PTR [ebx+0x18]
x.arith(Op::Add, 2, 0, true);  // 407498 faddp  st(2),st
x.fxch(1);  // 40749a fxch   st(1)
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x178)));  // 40749c fsub   DWORD PTR [esi+0x178]
m.wf(ebx.wrapping_add(0xc), x.pop());  // 4074a2 fstp   DWORD PTR [ebx+0xc]
m.wf(ebx.wrapping_add(0x10), x.pop());  // 4074a5 fstp   DWORD PTR [ebx+0x10]
m.wf(ebx.wrapping_add(0x14), x.pop());  // 4074a8 fstp   DWORD PTR [ebx+0x14]
                0x1c
            }
            0x40a932 => {   // composite: header copy of the point, then the children listed from +0x168 (zero-terminated pointer array)
                let (a, b, c) = (m.r32(edi), m.r32(edi + 4), m.r32(edi + 8));
                m.w32(ebx + 0xc, a); m.w32(ebx + 0x10, b); m.w32(ebx + 0x14, c);
                let mut eax = 0x18u32; let mut ebx = ebx + 0x18; let mut slot = esi;
                loop {
                    let child = self.m.r32(slot + 0x168);
                    if child == 0 { break; }
                    let ch = self.m.r32(child + 0xa0);
                    let sz = self.vt_a0(ch, child, edi, ebx);
                    self.m.w32(ebx + 8, sz); ebx = ebx.wrapping_add(sz); eax = eax.wrapping_add(sz); slot += 4;
                }
                eax
            }
            _ => panic!("vtable a0 {h:#x} not ported"),
        }
    }
    /// [obj+0xb0]: distance of the point at ecx from the object -> (eax = |d| bits, CF)
    fn vt_b0(&mut self, h: u32, esi: u32, ecx: u32) -> (u32, bool) {
        let m = &mut self.m;
        let mut x = X87::new();
        let esp = SCRATCH + 0x200;
        match h {
            0x406f3c => {
x.push(m.rf(ecx));  // 406f3f fld    DWORD PTR [ecx]
x.push(m.rf(ecx.wrapping_add(0x4)));  // 406f41 fld    DWORD PTR [ecx+0x4]
x.push(m.rf(ecx.wrapping_add(0x8)));  // 406f44 fld    DWORD PTR [ecx+0x8]
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x48)));  // 406f47 fsub   DWORD PTR [esi+0x48]
x.fxch(1);  // 406f4a fxch   st(1)
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x44)));  // 406f4c fsub   DWORD PTR [esi+0x44]
x.fxch(2);  // 406f4f fxch   st(2)
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x40)));  // 406f51 fsub   DWORD PTR [esi+0x40]
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0x134)));  // 406f54 fmul   DWORD PTR [esi+0x134]
x.fxch(1);  // 406f5a fxch   st(1)
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0x14c)));  // 406f5c fmul   DWORD PTR [esi+0x14c]
x.fxch(2);  // 406f62 fxch   st(2)
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0x140)));  // 406f64 fmul   DWORD PTR [esi+0x140]
x.fxch(2);  // 406f6a fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 406f6c faddp  st(1),st
x.arith(Op::Add, 1, 0, true);  // 406f6e faddp  st(1),st
m.wf(esp, x.top());  // 406f70 fst    DWORD PTR [esp]
x.fabs();  // 406f73 fabs
m.wf(esp.wrapping_add(0x4), x.pop());  // 406f75 fstp   DWORD PTR [esp+0x4]
                let sgn = m.r32(esp); (m.r32(esp + 4), sgn >> 31 != 0)
            }
            0x406f81 => {
x.push(m.rf(ecx));  // 406f84 fld    DWORD PTR [ecx]
x.push(m.rf(ecx.wrapping_add(0x4)));  // 406f86 fld    DWORD PTR [ecx+0x4]
x.push(m.rf(ecx.wrapping_add(0x8)));  // 406f89 fld    DWORD PTR [ecx+0x8]
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x48)));  // 406f8c fsub   DWORD PTR [esi+0x48]
x.fxch(1);  // 406f8f fxch   st(1)
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x44)));  // 406f91 fsub   DWORD PTR [esi+0x44]
x.fxch(2);  // 406f94 fxch   st(2)
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x40)));  // 406f96 fsub   DWORD PTR [esi+0x40]
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0x134)));  // 406f99 fmul   DWORD PTR [esi+0x134]
x.fxch(1);  // 406f9f fxch   st(1)
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0x14c)));  // 406fa1 fmul   DWORD PTR [esi+0x14c]
x.fxch(2);  // 406fa7 fxch   st(2)
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0x140)));  // 406fa9 fmul   DWORD PTR [esi+0x140]
x.fxch(2);  // 406faf fxch   st(2)
x.arith(Op::Add, 1, 0, true);  // 406fb1 faddp  st(1),st
x.arith(Op::Add, 1, 0, true);  // 406fb3 faddp  st(1),st
x.fabs();  // 406fb5 fabs
m.wf(esp.wrapping_add(0x4), x.pop());  // 406fb7 fstp   DWORD PTR [esp+0x4]
                let a = m.r32(esp + 4);
                let t = a.wrapping_sub(m.r32(esi + 0x16c)) ^ m.r32(esi + 0xf8);
                (a, t >> 31 != 0)
            }
            0x40a6cd => {
x.push(m.rf(ecx));  // 40a6d0 fld    DWORD PTR [ecx]
x.push(m.rf(ecx.wrapping_add(0x4)));  // 40a6d2 fld    DWORD PTR [ecx+0x4]
x.push(m.rf(ecx.wrapping_add(0x8)));  // 40a6d5 fld    DWORD PTR [ecx+0x8]
                xform(m, &mut x, esi);
x.arith(Op::Mul, 0, 0, false);  // 40a6dd fmul   st,st(0)
x.fxch(2);  // 40a6df fxch   st(2)
x.arith(Op::Mul, 0, 0, false);  // 40a6e1 fmul   st,st(0)
x.fxch(1);  // 40a6e3 fxch   st(1)
m.wf(esp, x.pop());  // 40a6e5 fstp   DWORD PTR [esp]
x.arith(Op::Add, 1, 0, true);  // 40a6e8 faddp  st(1),st
                let eax = m.r32(esp);
                if (eax as i32) < 0 || (eax as i32) > (m.r32(esi + 0x16c) as i32) {
x.push(m.rf(esp));  // 40a718 fld    DWORD PTR [esp]
x.arith(Op::Mul, 0, 0, false);  // 40a71b fmul   st,st(0)
x.arith(Op::Add, 1, 0, true);  // 40a71d faddp  st(1),st
x.sqrt();  // 40a71f fsqrt
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x164)));  // 40a721 fsub   DWORD PTR [esi+0x164]
x.fabs();  // 40a727 fabs
m.wf(esp.wrapping_add(0x4), x.pop());  // 40a729 fstp   DWORD PTR [esp+0x4]
                    (m.r32(esp + 4), m.r32(esi + 0xf8) >> 31 != 0)
                } else {
x.sqrt();  // 40a6f9 fsqrt
x.arith_m(Op::Sub, m.rf(esi.wrapping_add(0x168)));  // 40a6fb fsub   DWORD PTR [esi+0x168]
x.arith_m(Op::Mul, m.rf(esi.wrapping_add(0xf8)));  // 40a701 fmul   DWORD PTR [esi+0xf8]
m.wf(esp, x.top());  // 40a707 fst    DWORD PTR [esp]
x.fabs();  // 40a70a fabs
m.wf(esp.wrapping_add(0x4), x.pop());  // 40a70c fstp   DWORD PTR [esp+0x4]
                    let sgn = m.r32(esp); (m.r32(esp + 4), sgn >> 31 != 0)
                }
            }
            0x40696c => { let c = m.r32(esi + 0x168); let hh = m.r32(c + 0xb0); self.vt_b0(hh, c, ecx) }
            _ => panic!("vtable b0 {h:#x} not ported"),
        }
    }
    /// [obj+0xb4]: line test (edx = line record) -> (eax = signed distance bits, CF = |d| < radius)
    fn vt_b4(&mut self, h: u32, esi: u32, edx: u32) -> (u32, bool) {
        match h {
            0x4070b6 => (0, true),
            0x408ca3 => {
                let m = &mut self.m; let mut x = X87::new(); let esp = SCRATCH + 0x300;
x.push(m.rf(esi.wrapping_add(0x40)));  // 408ca6 fld    DWORD PTR [esi+0x40]
x.push(m.rf(esi.wrapping_add(0x44)));  // 408ca9 fld    DWORD PTR [esi+0x44]
x.push(m.rf(esi.wrapping_add(0x48)));  // 408cac fld    DWORD PTR [esi+0x48]
x.push(m.rf(edx));  // 408caf fld    DWORD PTR [edx]
x.push(m.rf(edx.wrapping_add(0x4)));  // 408cb1 fld    DWORD PTR [edx+0x4]
x.push(m.rf(edx.wrapping_add(0x8)));  // 408cb4 fld    DWORD PTR [edx+0x8]
x.arith(Op::Mul, 3, 0, true);  // 408cb7 fmulp  st(3),st
x.arith(Op::Mul, 3, 0, true);  // 408cb9 fmulp  st(3),st
x.arith(Op::Mul, 3, 0, true);  // 408cbb fmulp  st(3),st
x.push(m.rf(edx.wrapping_add(0xc)));  // 408cbd fld    DWORD PTR [edx+0xc]
x.arith(Op::Add, 1, 0, true);  // 408cc0 faddp  st(1),st
x.fxch(1);  // 408cc2 fxch   st(1)
x.arith(Op::Add, 2, 0, true);  // 408cc4 faddp  st(2),st
x.arith(Op::Add, 1, 0, true);  // 408cc6 faddp  st(1),st
                m.wf(esp + 4, x.top());
                x.fabs(); m.wf(esp, x.pop());
                (m.r32(esp + 4), m.r32(esp) < m.r32(esi + 0x164))
            }
            _ => panic!("vtable b4 {h:#x} not ported"),
        }
    }
    /// 0x40af49 (esi = class-16 node): camera / lights / sorted record lists / line marking
    pub(crate) fn camera_lists(&mut self, node: u32, _t: i32) {
        let ebp = self.ebp;
        let m = &mut self.m;
        let e = node;
        let v = (m.r32(e + 0x40) << 16) | 0x4000; m.w32(0x42a3c4, v); m.w32(0x42a3c8, v);
        let (a, b) = (m.r32(e + 0x1c), m.r32(e + 0x20)); m.w32(ebp + 0xe0, a); m.w32(ebp + 0xe4, b);
        let (a, b) = (m.r32(e + 0x34), m.r32(e + 0x38)); m.w32(ebp + 0xe8, a); m.w32(ebp + 0xec, b);
        let eax = m.r32(e + 0x2c); m.w32(ebp + 0x70, eax);
        let mut edx = m.r32(e + 0x30).wrapping_add(eax); edx = edx.wrapping_add(eax >> 16); edx <<= 16; edx = (edx & 0xffff_0000) | m.r16(e + 0x30) as u32;
        m.w32(ebp + 0x74, edx);
        let (a, b) = (m.r32(e + 0x18), m.r32(e + 0x28)); m.w32(ebp + 0x60, a); m.w32(ebp + 0x6c, b);
        let vp = m.r32(e + 0x3c);
        m.w32(ebp + 0x124, ebp + 0x2a_01e0); m.w32(ebp + 0x78, 0); m.w32(ebp + 0x64, 0); m.w32(ebp + 0x130, 0);
        let root = m.r32(ebp + 0x60);
        self.walk_objects(root);
        let cam = self.m.r32(ebp + 0x64);
        let fov = self.m.r32(cam + 0xa4);
        self.cam_tables(vp, fov);
        self.cam_basis(cam);
        // 0x40afe5: camera position copy, inverse of the camera basis, the frustum line records
        let esi = cam; let edx = esi + 0xac;
        for i in 0..3u32 { let v = self.m.r32(esi + 0x40 + 4 * i); self.m.w32(ebp + 0xf0 + 4 * i, v); }
        mat_inv(&mut self.m, edx, ebp + 0xfc);
        let esp = SCRATCH + 0x400;              // 0x60-byte frame: scratch at esp.., results at esp+0x30..
        let m = &mut self.m;
        m.w32(esp + 4, 0); m.w32(esp + 8, 0x3f80_0000);
        let ecx = m.r32(ebp + 0xd8) ^ 0x8000_0000; m.w32(esp, ecx);
        let mut ebx = esp + 0x30;
        crate::mat3::vec_mat(m, esp, edx, ebx);
        m.w32(esp + 8, 0); ebx += 0xc;
        let ecx = m.r32(ebp + 0xd4); m.w32(esp, ecx);
        crate::mat3::vec_mat(m, esp, edx, ebx);
        ebx += 0xc; m.w32(esp, 0);
        let ecx = m.r32(ebp + 0xdc) ^ 0x8000_0000; m.w32(esp + 4, ecx);
        crate::mat3::vec_mat(m, esp, edx, ebx);
        ebx += 0xc;
        let ecx = m.r32(ebp + 0xd4); m.w32(esp + 4, ecx);
        crate::mat3::vec_mat(m, esp, edx, ebx);
        let s = esp + 0x30;                     // add esp,0x30: A = s, B = s+0xc, C = s+0x18, D = s+0x24
        let (pa, pb, pc, pd) = (s, s + 0xc, s + 0x18, s + 0x24);
        let mut edi = m.r32(ebp + 0xc4); let mut cnt = m.r32(ebp + 0x98) + 1;
        let campos = ebp + 0xf0;
        while cnt != 0 {
            for i in 0..3u32 { let v = m.r32(pa + 4 * i); m.w32(edi + 0x10 + 4 * i, v); }
            crate::mat3::vec_add(m, pa, pd, edi);
            v_cross(m, pa, edi, edi);
            v_norm(m, edi);
            v_dot(m, edi, campos, edi + 0xc);
            let v = m.r32(edi + 0xc) ^ 0x8000_0000; m.w32(edi + 0xc, v);
            m.w32(edi + 0x1c, 0); edi += 0x200;
            v_add_in(m, pa, pb);
            cnt -= 1;
        }
        edi = m.r32(ebp + 0xc8); cnt = m.r32(ebp + 0x9c) + 1;
        v_add_in(m, pa, pc);
        while cnt != 0 {
            for i in 0..3u32 { let v = m.r32(pc + 4 * i); m.w32(edi + 0x10 + 4 * i, v); }
            crate::mat3::vec_add(m, pa, pb, edi);
            v_cross(m, edi, pa, edi);
            v_norm(m, edi);
            v_dot(m, edi, campos, edi + 0xc);
            let v = m.r32(edi + 0xc) ^ 0x8000_0000; m.w32(edi + 0xc, v);
            m.w32(edi + 0x1c, 0); edi += 0x200;
            v_add_in(m, pc, pd);
            v_add_in(m, pa, pd);
            cnt -= 1;
        }
        // 0x40b172: every drawable object sorted by distance from the camera, records, line marking
        let cell = SCRATCH + 0x500; self.m.w32(cell, 0);
        let root = self.m.r32(ebp + 0x60);
        self.list_all(root, cell, campos, None);
        let head = self.m.r32(cell);
        let rec = self.emit_list(head, campos);
        self.m.w32(ebp + 0x120, rec);
        let mut ecx = rec;
        loop { let esi = self.m.r32(ecx); if esi == 0 { break; } let h = self.m.r32(esi + 0xbc); assert_eq!(h, 0x4088a5, "vtable bc {h:#x}"); self.mark_lines(esi, ecx); ecx += self.m.r32(ecx + 8); }
        for (n, base) in [(0x98u32, 0xc4u32), (0x9c, 0xc8)] {
            let edx = self.m.r32(ebp + base); let end = (self.m.r32(ebp + n) << 9).wrapping_add(edx);
            self.line_range(edx, end, ecx);
        }
        // lights
        let mut esi = self.m.r32(ebp + 0x78);
        while esi != 0 {
            let pos = esi + 0x40; let mask = self.m.r32(esi + 0xa0);
            let cell = SCRATCH + 0x500; self.m.w32(cell, 0);
            let root = self.m.r32(ebp + 0x60);
            self.list_all(root, cell, pos, Some(mask));
            let head = self.m.r32(cell);
            let rec = self.emit_list(head, pos);
            self.m.w32(esi + 0xc4, rec); self.m.w32(esi + 0xc8, rec);
            esi = self.m.r32(esi + 0xcc);
        }
    }
}
