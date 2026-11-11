//! Runtime of the recompiled rasteriser (raster_gen.rs): integer CPU state with eager flags, MMX instruction semantics on u64, x87 via `X87`.
use crate::scene::{Mem, X87, STACK_BASE};
pub const ADD: u8 = 0; pub const SUB: u8 = 1; pub const AND: u8 = 2; pub const OR: u8 = 3; pub const XOR: u8 = 4; pub const ADC: u8 = 5; pub const SBB: u8 = 6;
#[derive(Clone)]
pub struct Cpu { pub r: [u32; 8], pub cf: bool, pub zf: bool, pub sf: bool, pub of: bool, pub mm: [u64; 8], pub x: X87, pub trace: Option<Vec<u32>>,
    /** x87 status word (C0 = bit 8, C2 = bit 10, C3 = bit 14) set by fcom */ pub fsw: u16, pub pf: bool, pub idf: bool, pub fpu_saved: Vec<X87>,
    /** the loader's stop condition: return from `run` when pc == stop_pc for the stop_n-th time */ pub stop_pc: u32, pub stop_n: u32, /** offline HQ: trace every pixel (skip the adaptive flat/interpolate decision at 0x40224d) */ pub force_full: bool, pub w: crate::win32::Win32 }
fn msk(w: u32) -> u32 { if w == 32 { u32::MAX } else { (1u32 << w) - 1 } }
impl Cpu {
    pub fn new() -> Cpu { Cpu { r: [0; 8], cf: false, zf: false, sf: false, of: false, mm: [0; 8], x: X87::new(), trace: None, fsw: 0, pf: false, idf: false, fpu_saved: vec![], stop_pc: u32::MAX, stop_n: 0, force_full: crate::full(), w: Default::default() } }
    fn zs(&mut self, r: u32, w: u32) { self.zf = r & msk(w) == 0; self.sf = (r >> (w - 1)) & 1 != 0; }
    pub fn alu(&mut self, op: u8, a: u32, b: u32, w: u32) -> u32 {
        let k = msk(w); let (a, b) = (a & k, b & k); let sb = 1u32 << (w - 1);
        let r = match op {
            ADD | ADC => { let c = if op == ADC && self.cf { 1u64 } else { 0 }; let s = a as u64 + b as u64 + c; let r = (s as u32) & k; self.cf = s > k as u64; self.of = ((a ^ r) & (b ^ r) & sb) != 0; r }
            SUB | SBB => { let c = if op == SBB && self.cf { 1u64 } else { 0 }; let r = (a as u64).wrapping_sub(b as u64).wrapping_sub(c) as u32 & k; self.cf = (a as u64) < b as u64 + c; self.of = ((a ^ b) & (a ^ r) & sb) != 0; r }
            _ => { let r = match op { AND => a & b, OR => a | b, _ => a ^ b }; self.cf = false; self.of = false; r }
        };
        self.zs(r, w); r
    }
    pub fn inc(&mut self, a: u32, w: u32) -> u32 { let cf = self.cf; let r = self.alu(ADD, a, 1, w); self.cf = cf; r }
    pub fn dec(&mut self, a: u32, w: u32) -> u32 { let cf = self.cf; let r = self.alu(SUB, a, 1, w); self.cf = cf; r }
    pub fn shl(&mut self, a: u32, n: u32, w: u32) -> u32 { let n = n & 31; if n == 0 { return a; } let a = a & msk(w); let r = ((a as u64) << n) as u32 & msk(w); self.cf = ((a as u64) << n) >> w & 1 != 0; self.zs(r, w); self.of = ((r >> (w - 1)) & 1 != 0) != self.cf; r }
    pub fn shr(&mut self, a: u32, n: u32, w: u32) -> u32 { let n = n & 31; if n == 0 { return a; } let a = a & msk(w); let r = a >> n; self.cf = (a >> (n - 1)) & 1 != 0; self.zs(r, w); self.of = (a >> (w - 1)) & 1 != 0; r }
    pub fn sar(&mut self, a: u32, n: u32, w: u32) -> u32 { let n = n & 31; if n == 0 { return a; } let s = ((a << (32 - w)) as i32) >> (32 - w); let r = (s >> n) as u32 & msk(w); self.cf = (s >> (n - 1)) & 1 != 0; self.zs(r, w); self.of = false; r }
    pub fn shld(&mut self, d: u32, s: u32, n: u32) -> u32 { let n = n & 31; if n == 0 { return d; } let r = (d << n) | (s >> (32 - n)); self.cf = (d >> (32 - n)) & 1 != 0; self.zs(r, 32); self.of = ((r ^ d) >> 31) & 1 != 0; r }
    pub fn imul(&mut self, a: u32, b: u32) -> u32 { let p = (a as i32 as i64) * (b as i32 as i64); let r = p as u32; self.cf = p != r as i32 as i64; self.of = self.cf; r }
    pub fn mul(&mut self, b: u32) { let p = self.r[0] as u64 * b as u64; self.r[0] = p as u32; self.r[2] = (p >> 32) as u32; self.cf = self.r[2] != 0; self.of = self.cf; }
    pub fn push(&mut self, m: &mut Mem, v: u32) { self.r[4] = self.r[4].wrapping_sub(4); m.w32(self.r[4], v); }
    pub fn pop(&mut self, m: &mut Mem) -> u32 { let v = m.r32(self.r[4]); self.r[4] = self.r[4].wrapping_add(4); v }
    pub fn pusha(&mut self, m: &mut Mem) { let s = self.r[4]; for i in 0..8 { let v = if i == 4 { s } else { self.r[i] }; self.push(m, v); } }
    pub fn popa(&mut self, m: &mut Mem) { for i in (0..8).rev() { let v = self.pop(m); if i != 4 { self.r[i] = v; } } }

    pub fn rol(&mut self, a: u32, n: u32, w: u32) -> u32 { let n = n & 31; if n == 0 { return a; } let a = a & msk(w); let k = n % w; let r = if k == 0 { a } else { ((a << k) | (a >> (w - k))) & msk(w) }; self.cf = r & 1 != 0; r }
    pub fn ror(&mut self, a: u32, n: u32, w: u32) -> u32 { let n = n & 31; if n == 0 { return a; } let a = a & msk(w); let k = n % w; let r = if k == 0 { a } else { ((a >> k) | (a << (w - k))) & msk(w) }; self.cf = (r >> (w - 1)) & 1 != 0; r }
    pub fn rcl(&mut self, a: u32, n: u32, w: u32) -> u32 { let n = n & 31; let mut v = a & msk(w); for _ in 0..n { let t = (v >> (w - 1)) & 1 != 0; v = ((v << 1) | self.cf as u32) & msk(w); self.cf = t; } v }
    pub fn rcr(&mut self, a: u32, n: u32, w: u32) -> u32 { let n = n & 31; let mut v = a & msk(w); for _ in 0..n { let t = v & 1 != 0; v = (v >> 1) | ((self.cf as u32) << (w - 1)); self.cf = t; } v }
    pub fn shrd(&mut self, d: u32, s: u32, n: u32) -> u32 { let n = n & 31; if n == 0 { return d; } let r = (d >> n) | (s << (32 - n)); self.cf = (d >> (n - 1)) & 1 != 0; self.zs(r, 32); r }
    /** one-operand mul/imul/div/idiv (op 0..3) on al / ax / eax of width w */
    pub fn md(&mut self, op: u8, v: u32, w: u32) {
        let acc: u64 = match w { 8 => (self.r[0] & 0xffff) as u64, 16 => ((self.r[2] & 0xffff) as u64) << 16 | (self.r[0] & 0xffff) as u64, _ => (self.r[2] as u64) << 32 | self.r[0] as u64 };
        let sx = |x: u32| -> i64 { match w { 8 => x as u8 as i8 as i64, 16 => x as u16 as i16 as i64, _ => x as i32 as i64 } };
        let (lo, hi): (u64, u64);
        match op {
            0 | 1 => {
                let a = self.r[0] & msk(w); let b = v & msk(w);
                let p: i128 = if op == 0 { a as i128 * b as i128 } else { sx(a) as i128 * sx(b) as i128 };
                let pu = p as u128 as u64; let m = msk(w) as u64;
                lo = pu & m; hi = (pu >> w) & m;
                self.cf = if op == 0 { hi != 0 } else { p != sx(lo as u32) as i128 }; self.of = self.cf;
            }
            _ => {
                let d = v & msk(w); assert!(d != 0, "divide by zero");
                if op == 2 { let q = acc / d as u64; let r = acc % d as u64; assert!(q <= msk(w) as u64, "div overflow"); lo = q; hi = r; }
                else {
                    let a: i64 = match w { 8 => acc as u16 as i16 as i64, 16 => acc as u32 as i32 as i64, _ => acc as i64 };
                    let dd = sx(d); let q = a / dd; let r = a % dd; assert!(q == sx(q as u32 & msk(w)), "idiv overflow"); lo = q as u64 & msk(w) as u64; hi = r as u64 & msk(w) as u64;
                }
            }
        }
        match w { 8 => { self.r[0] = (self.r[0] & 0xffff0000) | (hi as u32) << 8 | lo as u32; }
                  16 => { self.r[0] = (self.r[0] & 0xffff0000) | lo as u32; self.r[2] = (self.r[2] & 0xffff0000) | hi as u32; }
                  _ => { self.r[0] = lo as u32; self.r[2] = hi as u32; } }
    }
    pub fn div(&mut self, d: u32) { self.md(2, d, 32); }
    pub fn idiv(&mut self, d: u32) { self.md(3, d, 32); }
    pub fn pushf(&mut self, m: &mut Mem) { let f = 2 | self.cf as u32 | (self.pf as u32) << 2 | (self.zf as u32) << 6 | (self.sf as u32) << 7 | (self.of as u32) << 11 | (self.idf as u32) << 21; self.push(m, f); }
    pub fn popf(&mut self, m: &mut Mem) { let f = self.pop(m); self.cf = f & 1 != 0; self.pf = f >> 2 & 1 != 0; self.zf = f >> 6 & 1 != 0; self.sf = f >> 7 & 1 != 0; self.of = f >> 11 & 1 != 0; self.idf = f >> 21 & 1 != 0; }
    /// Unicorn's default CPU (qemu64), leaf 1: only the MMX bit of edx is tested by the original
    pub fn cpuid(&mut self) { self.r[0] = 0x663; self.r[3] = 0x800; self.r[1] = 0x80802001; self.r[2] = 0x078bfbfd; }
    pub fn fcom(&mut self, v: f64) { let a = self.x.st64(0); self.fsw = if a.is_nan() || v.is_nan() { 0x4500 } else if a < v { 0x0100 } else if a == v { 0x4000 } else { 0 }; }
    pub fn sahf(&mut self) { let ah = (self.r[0] >> 8) & 0xff; self.cf = ah & 1 != 0; self.pf = ah >> 2 & 1 != 0; self.zf = ah >> 6 & 1 != 0; self.sf = ah >> 7 & 1 != 0; }
    pub fn fsave(&mut self) { let x = std::mem::replace(&mut self.x, X87::new()); self.fpu_saved.push(x); }
    pub fn frstor(&mut self) { self.x = self.fpu_saved.pop().expect("frstor without fsave"); }
    /// run the routine at `pc` (a `call` from the sentinel address 0xdead0000)
    pub fn call(&mut self, m: &mut Mem, pc: u32) { self.push(m, 0xdead0000); crate::raster_gen::run(self, m, pc); }
    pub fn init_stack(&mut self) { self.r[4] = STACK_BASE + 0xff00; }
}
/// fistp m16: out of range / NaN -> 0x8000 (integer indefinite)
pub fn fist16(x: f64) -> u16 { if x.is_finite() && x.abs() < 32768.0 { x.round_ties_even() as i32 as i16 as u16 } else { 0x8000 } }
fn l(x: u64) -> [u16; 4] { [x as u16, (x >> 16) as u16, (x >> 32) as u16, (x >> 48) as u16] }
fn p(v: [u16; 4]) -> u64 { v[0] as u64 | (v[1] as u64) << 16 | (v[2] as u64) << 32 | (v[3] as u64) << 48 }
fn lb(x: u64) -> [u8; 8] { x.to_le_bytes() }
fn w4(a: u64, b: u64, f: impl Fn(u16, u16) -> u16) -> u64 { let (a, b) = (l(a), l(b)); p([f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2]), f(a[3], b[3])]) }
fn b8(a: u64, b: u64, f: impl Fn(u8, u8) -> u8) -> u64 { let (a, b) = (lb(a), lb(b)); let mut r = [0u8; 8]; for i in 0..8 { r[i] = f(a[i], b[i]); } u64::from_le_bytes(r) }
fn d2(a: u64, b: u64, f: impl Fn(u32, u32) -> u32) -> u64 { f(a as u32, b as u32) as u64 | (f((a >> 32) as u32, (b >> 32) as u32) as u64) << 32 }
pub fn paddw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| x.wrapping_add(y)) }
pub fn psubw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| x.wrapping_sub(y)) }
pub fn paddd(a: u64, b: u64) -> u64 { d2(a, b, |x, y| x.wrapping_add(y)) }
pub fn psubd(a: u64, b: u64) -> u64 { d2(a, b, |x, y| x.wrapping_sub(y)) }
pub fn paddb(a: u64, b: u64) -> u64 { b8(a, b, |x, y| x.wrapping_add(y)) }
pub fn psubb(a: u64, b: u64) -> u64 { b8(a, b, |x, y| x.wrapping_sub(y)) }
pub fn paddsw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| (x as i16).saturating_add(y as i16) as u16) }
pub fn psubsw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| (x as i16).saturating_sub(y as i16) as u16) }
pub fn paddusw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| x.saturating_add(y)) }
pub fn psubusw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| x.saturating_sub(y)) }
pub fn paddsb(a: u64, b: u64) -> u64 { b8(a, b, |x, y| (x as i8).saturating_add(y as i8) as u8) }
pub fn paddusb(a: u64, b: u64) -> u64 { b8(a, b, |x, y| x.saturating_add(y)) }
pub fn psubusb(a: u64, b: u64) -> u64 { b8(a, b, |x, y| x.saturating_sub(y)) }
pub fn pmulhw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| (((x as i16 as i32) * (y as i16 as i32)) >> 16) as u16) }
pub fn pmullw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| x.wrapping_mul(y)) }
pub fn pmaddwd(a: u64, b: u64) -> u64 { let (a, b) = (l(a), l(b)); let m = |i: usize| (a[i] as i16 as i32) * (b[i] as i16 as i32); (m(0).wrapping_add(m(1)) as u32 as u64) | (m(2).wrapping_add(m(3)) as u32 as u64) << 32 }
pub fn pcmpgtw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| if (x as i16) > (y as i16) { 0xffff } else { 0 }) }
pub fn pcmpeqw(a: u64, b: u64) -> u64 { w4(a, b, |x, y| if x == y { 0xffff } else { 0 }) }
pub fn pcmpgtb(a: u64, b: u64) -> u64 { b8(a, b, |x, y| if (x as i8) > (y as i8) { 0xff } else { 0 }) }
pub fn pcmpeqb(a: u64, b: u64) -> u64 { b8(a, b, |x, y| if x == y { 0xff } else { 0 }) }
pub fn pcmpgtd(a: u64, b: u64) -> u64 { d2(a, b, |x, y| if (x as i32) > (y as i32) { u32::MAX } else { 0 }) }
pub fn pcmpeqd(a: u64, b: u64) -> u64 { d2(a, b, |x, y| if x == y { u32::MAX } else { 0 }) }
pub fn pand(a: u64, b: u64) -> u64 { a & b }
pub fn pandn(a: u64, b: u64) -> u64 { !a & b }
pub fn por(a: u64, b: u64) -> u64 { a | b }
pub fn pxor(a: u64, b: u64) -> u64 { a ^ b }
pub fn punpcklbw(a: u64, b: u64) -> u64 { let (a, b) = (lb(a), lb(b)); p([a[0] as u16 | (b[0] as u16) << 8, a[1] as u16 | (b[1] as u16) << 8, a[2] as u16 | (b[2] as u16) << 8, a[3] as u16 | (b[3] as u16) << 8]) }
pub fn punpckhbw(a: u64, b: u64) -> u64 { let (a, b) = (lb(a), lb(b)); p([a[4] as u16 | (b[4] as u16) << 8, a[5] as u16 | (b[5] as u16) << 8, a[6] as u16 | (b[6] as u16) << 8, a[7] as u16 | (b[7] as u16) << 8]) }
pub fn punpcklwd(a: u64, b: u64) -> u64 { let (a, b) = (l(a), l(b)); p([a[0], b[0], a[1], b[1]]) }
pub fn punpckhwd(a: u64, b: u64) -> u64 { let (a, b) = (l(a), l(b)); p([a[2], b[2], a[3], b[3]]) }
pub fn punpckldq(a: u64, b: u64) -> u64 { (a & 0xffff_ffff) | (b << 32) }
pub fn punpckhdq(a: u64, b: u64) -> u64 { (a >> 32) | (b & 0xffff_ffff_0000_0000) }
fn sat8(v: u16, lo: i16, hi: i16) -> u64 { (v as i16).clamp(lo, hi) as u8 as u64 }
pub fn packuswb(a: u64, b: u64) -> u64 { let (a, b) = (l(a), l(b)); let mut r = 0u64; for i in 0..4 { r |= sat8(a[i], 0, 255) << (8 * i) | sat8(b[i], 0, 255) << (8 * i + 32); } r }
pub fn packsswb(a: u64, b: u64) -> u64 { let (a, b) = (l(a), l(b)); let mut r = 0u64; for i in 0..4 { r |= sat8(a[i], -128, 127) << (8 * i) | sat8(b[i], -128, 127) << (8 * i + 32); } r }
pub fn packssdw(a: u64, b: u64) -> u64 { let s = |v: u32| (v as i32).clamp(-32768, 32767) as i16 as u16 as u64; s(a as u32) | s((a >> 32) as u32) << 16 | s(b as u32) << 32 | s((b >> 32) as u32) << 48 }
pub fn psllw(a: u64, n: u32) -> u64 { if n > 15 { 0 } else { p(l(a).map(|v| v << n)) } }
pub fn psrlw(a: u64, n: u32) -> u64 { if n > 15 { 0 } else { p(l(a).map(|v| v >> n)) } }
pub fn psraw(a: u64, n: u32) -> u64 { let n = n.min(15); p(l(a).map(|v| ((v as i16) >> n) as u16)) }
pub fn pslld(a: u64, n: u32) -> u64 { if n > 31 { 0 } else { d2(a, 0, |x, _| x << n) } }
pub fn psrld(a: u64, n: u32) -> u64 { if n > 31 { 0 } else { d2(a, 0, |x, _| x >> n) } }
pub fn psrad(a: u64, n: u32) -> u64 { let n = n.min(31); d2(a, 0, |x, _| ((x as i32) >> n) as u32) }
pub fn psllq(a: u64, n: u32) -> u64 { if n > 63 { 0 } else { a << n } }
pub fn psrlq(a: u64, n: u32) -> u64 { if n > 63 { 0 } else { a >> n } }
