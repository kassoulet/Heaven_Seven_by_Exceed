//! MMX register helpers (a 64-bit register is a u64; lane 0 = least significant). One function per instruction, same semantics as the original (wrapping 16-bit lanes, signed saturation for pack*).
pub fn lanes(x: u64) -> [u16; 4] { [x as u16, (x >> 16) as u16, (x >> 32) as u16, (x >> 48) as u16] }
pub fn pk(l: [u16; 4]) -> u64 { l[0] as u64 | (l[1] as u64) << 16 | (l[2] as u64) << 32 | (l[3] as u64) << 48 }
pub fn unpcklbw(x: u64) -> u64 { pk([x as u8 as u16, (x >> 8) as u8 as u16, (x >> 16) as u8 as u16, (x >> 24) as u8 as u16]) }
pub fn unpckhbw(x: u64) -> u64 { unpcklbw(x >> 32) }
pub fn paddw(a: u64, b: u64) -> u64 { let (a, b) = (lanes(a), lanes(b)); pk([a[0].wrapping_add(b[0]), a[1].wrapping_add(b[1]), a[2].wrapping_add(b[2]), a[3].wrapping_add(b[3])]) }
pub fn pmullw(a: u64, b: u64) -> u64 { let (a, b) = (lanes(a), lanes(b)); pk([a[0].wrapping_mul(b[0]), a[1].wrapping_mul(b[1]), a[2].wrapping_mul(b[2]), a[3].wrapping_mul(b[3])]) }
pub fn pmulhw(a: u64, b: u64) -> u64 { let (a, b) = (lanes(a), lanes(b)); let h = |x: u16, y: u16| (((x as i16 as i32) * (y as i16 as i32)) >> 16) as u16; pk([h(a[0], b[0]), h(a[1], b[1]), h(a[2], b[2]), h(a[3], b[3])]) }
/// pmaddwd: two 32-bit results, each the sum of two adjacent signed 16x16 products
pub fn pmaddwd(a: u64, b: u64) -> u64 { let (a, b) = (lanes(a), lanes(b)); let m = |i: usize| (a[i] as i16 as i32) * (b[i] as i16 as i32); ((m(0).wrapping_add(m(1))) as u32 as u64) | ((m(2).wrapping_add(m(3))) as u32 as u64) << 32 }
pub fn packsswb_lo(a: u64) -> u64 { let l = lanes(a); let s = |v: u16| (v as i16).clamp(-128, 127) as i8 as u8 as u64; s(l[0]) | s(l[1]) << 8 | s(l[2]) << 16 | s(l[3]) << 24 }
/// packsswb mm,mm (same source twice): low dword = the four saturated bytes of the source, high dword = the same
pub fn packsswb_same(a: u64) -> u64 { let v = packsswb_lo(a); v | v << 32 }
pub fn psllw(a: u64, n: u32) -> u64 { let l = lanes(a); pk([l[0].wrapping_shl(n), l[1].wrapping_shl(n), l[2].wrapping_shl(n), l[3].wrapping_shl(n)]) }
pub fn punpcklwd(a: u64, b: u64) -> u64 { let (a, b) = (lanes(a), lanes(b)); pk([a[0], b[0], a[1], b[1]]) }
pub fn punpckhwd(a: u64, b: u64) -> u64 { let (a, b) = (lanes(a), lanes(b)); pk([a[2], b[2], a[3], b[3]]) }
pub fn punpckldq(a: u64, b: u64) -> u64 { (a & 0xffff_ffff) | (b << 32) }
pub fn punpckhdq(a: u64, b: u64) -> u64 { (a >> 32) | (b & 0xffff_ffff_0000_0000) }
pub fn psrad(a: u64, n: u32) -> u64 { let lo = ((a as u32 as i32) >> n) as u32 as u64; let hi = (((a >> 32) as u32 as i32) >> n) as u32 as u64; lo | hi << 32 }
pub fn paddd(a: u64, b: u64) -> u64 { ((a as u32).wrapping_add(b as u32) as u64) | (((a >> 32) as u32).wrapping_add((b >> 32) as u32) as u64) << 32 }
pub fn movd(b: &[u8], o: isize) -> u64 { let o = o.max(0) as usize; u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as u64 }
