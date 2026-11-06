//! The 3x3 matrix / vec3 helpers of the 3D engine (0x40d13d .. 0x40d2c7), literal x87 transcriptions on the flat memory image (same operand order: the sums
//! are ((p0+p1)+p2) or the odd orders of 0x40d1c9, results rounded to f32 after every op).
use crate::scene::Mem;

/// 0x40d256: ebx[r*3+j] = sum_k eax[r*3+k] * edx[k*3+j]  (matrix product, ((p0+p1)+p2))
pub fn mat_mul(m: &mut Mem, a: u32, b: u32, out: u32) {
    for r in 0..3u32 { for j in 0..3u32 {
        let p = |k: u32| m.rf(a + 12 * r + 4 * k) * m.rf(b + 12 * k + 4 * j);
        let v = (p(0) + p(1)) + p(2);
        m.wf(out + 12 * r + 4 * j, v);
    } }
}
/// 0x40d1c9: ebx = eax(vec3) * edx(3x3 matrix, columns at stride 0xc), with the original's addition orders: out0 = (p0+p1)+p2, out1 = q2 + (q0+q1), out2 = (r0+r1)+r2
pub fn vec_mat(m: &mut Mem, v: u32, mat: u32, out: u32) {
    let (v0, v1, v2) = (m.rf(v), m.rf(v + 4), m.rf(v + 8));
    let col = |j: u32| (v0 * m.rf(mat + 4 * j), v1 * m.rf(mat + 0xc + 4 * j), v2 * m.rf(mat + 0x18 + 4 * j));
    let (p0, p1, p2) = col(0); let (q0, q1, q2) = col(1); let (r0, r1, r2) = col(2);
    let o0 = (p1 + p0) + p2; let o1 = q2 + (q0 + q1); let o2 = (r0 + r1) + r2;
    m.wf(out, o0); m.wf(out + 4, o1); m.wf(out + 8, o2);
}
/// 0x40d15c: ebx = eax + edx (vec3);  0x40d13d: eax += edx
pub fn vec_add(m: &mut Mem, a: u32, b: u32, out: u32) { for i in 0..3u32 { let v = m.rf(a + 4 * i) + m.rf(b + 4 * i); m.wf(out + 4 * i, v); } }
/// 0x40d23b: identity matrix at edi (9 floats)
pub fn identity(m: &mut Mem, a: u32) { for i in 0..9u32 { m.wf(a + 4 * i, if i % 4 == 0 { 1.0 } else { 0.0 }); } }
