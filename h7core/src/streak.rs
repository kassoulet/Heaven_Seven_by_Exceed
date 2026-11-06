//! The light-streak / glow filter on the 8-bit layer: 0x40ac69 (seed 2x2 blend at the light position) + 0x40ab3f (four mirrored quadrant sweeps with an IIR feedback),
//! literal transcription on the flat memory image (registers = variables, MMX = mmx.rs helpers). Class 50 draw handler 0x408fab calls it.
use crate::mmx::*;
use crate::scene::Mem;

fn w32(a: u32, b: i32) -> u32 { a.wrapping_add(b as u32) }
fn q(m: &Mem, a: u32) -> u64 { m.r32(a) as u64 | (m.r32(a.wrapping_add(4)) as u64) << 32 }

/// 0x40ab3f: eax/edx = the step vectors (bytes), edi = start pointer, ebp = table pointer (4 words per entry, row stride [0x42a3ec]), x/y = the caller's [esp], [esp+4], qq = the
/// 64-bit gain operand (caller [esp+0x10])
fn sweep(m: &mut Mem, eax: i32, edx: i32, mut edi: u32, mut ebp: u32, x: i32, y: i32, qq: u64, row_stride: u32) {
    let esi = edx.wrapping_add(eax);
    let l = if y < x { y } else { x };                // cmp ecx(Y),X ; jl ... : min
    let mut i: i32 = 0;
    let mut first = true;
    loop {
        // first loop of the outer iteration i (along eax)
        let mut ebx = if first { x - 1 - i } else { x - i };
        let (mut a, mut d) = (eax, edx);
        let mut skip_first = ebx <= 0;
        if !skip_first {
            let (sebp, sedi) = (ebp, edi);
            if first { ebp = ebp.wrapping_add(8); edi = w32(edi, -a); }
            run(m, &mut ebx, a, esi, &mut edi, &mut ebp, qq);
            edi = sedi; ebp = sebp;
        }
        let _ = &mut skip_first;
        // 0x40abdb
        ebp = ebp.wrapping_add(8); edi = w32(edi, -d);
        let mut ebx = y - 1 - i;   // [esp+0x2c] after the push is the caller's Y (L lives at [esp+0x2c] only without the push, used by the loop test)
        if ebx > 0 {
            std::mem::swap(&mut a, &mut d);                 // xchg edx,eax
            let (sebp, sedi) = (ebp, edi);
            run(m, &mut ebx, a, esi, &mut edi, &mut ebp, qq);
            edi = sedi; ebp = sebp;
        }
        // 0x40ac53
        ebp = ebp.wrapping_add(row_stride); i += 1; edi = w32(edi, -eax);
        first = false;
        if i == l { break; }
    }
}

/// the loop body 0x40ab76 / 0x40ab82 (`ebx` iterations): feedback of the previous output through mm1
fn run(m: &mut Mem, ebx: &mut i32, eax: i32, esi: i32, edi: &mut u32, ebp: &mut u32, qq: u64) {
    // 0x40ab76: fresh load of the two bytes ahead
    let mut cl = m.r8(w32(*edi, eax.wrapping_mul(2))) as u32; let mut ch = m.r8(w32(*edi, eax)) as u32;
    let mut ecx = cl | ch << 8;
    let mut mm1 = dupbw(ecx as u64);
    loop {
        cl = m.r8(*edi) as u32; ch = m.r8(w32(*edi, esi)) as u32;
        ecx = cl | ch << 8;
        let mm0 = q(m, *ebp);
        let mut mm2 = dupbw(ecx as u64);
        let mut mm0 = pmulhw(mm0, qq);
        mm2 = punpckldq(mm2, mm1);
        mm0 = paddw(mm0, mm0);
        mm2 = pmaddwd(mm2, mm0);
        let mm5 = 0x0000_2000_0000_2000u64;
        let mm4 = mm2 >> 32;
        mm2 = paddd(mm2, mm4);
        mm1 = (mm1 as u32 >> 16) as u64 | (((mm1 >> 32) as u32 >> 16) as u64) << 32;   // psrld mm1,16
        mm2 = paddd(mm2, mm5);
        mm2 = psrad(mm2, 14);
        mm2 = packssdw_same(mm2);
        *edi = w32(*edi, -eax);
        mm2 = psraw(mm2, 8);
        *ebp = ebp.wrapping_add(8);
        mm2 = packsswb_same(mm2);
        let out = mm2 as u32;
        *ebx -= 1;
        mm2 = dupbw(mm2);
        m.w8(w32(*edi, eax), out as u8);
        mm1 = punpcklwd(mm1, mm2);
        if *ebx == 0 { break; }
    }
}

/// 0x40ac69: eax = A, edx = B (both from the node: x87 products * 65536), (x, y) = the light position in 16.16, esi = the layer (viewport) object
pub fn streak(m: &mut Mem, ebp_tab: u32, row_stride: u32, a_in: u32, b_in: u32, pos: (i32, i32), vp: u32) {
    let a = a_in >> 2; let b = b_in >> 2;
    let ecx = (a << 16) | a;
    let lo = ((ecx & 0xffff_0000) | (b & 0xffff)) as u64;
    let qq = lo | (ecx as u64) << 32;                 // [esp+0x10] = (a<<16)|b-low, [esp+0x14] = (a<<16)|a
    let mut ecx = 0x8000i32.wrapping_sub(a as i32).wrapping_sub(a as i32); if ecx <= 0 { ecx = 1; }
    let eax = (b << 6).wrapping_sub(0x20);
    let quo = ((eax as u32) as i64) / (ecx as i64);   // edx = 0, idiv ecx: the dividend is eax zero-extended
    let g = (quo as u32 & 0xffff) as u64; let mm6 = g | g << 16 | g << 32 | g << 48;   // movd; punpcklwd; punpckldq (low dword twice): all four words = the low word
    let (w, h, pitch, data) = (m.r32(vp + 0x18) as i32, m.r32(vp + 0x1c) as i32, (m.r32(vp + 0x20) >> 2) as i32, m.r32(vp + 0x28));
    let mut yy = pos.1 >> 16; if yy <= 0 { yy = 1; } if !(yy < h) { yy = h - 1; }
    let ydone = yy;
    let mut edi = (pitch.wrapping_mul(yy - 1)) as u32;
    let mut xx = pos.0 >> 16; if xx <= 0 { xx = 1; } if !(xx < w) { xx = w - 1; }
    let xdone = xx;
    edi = edi.wrapping_add(data).wrapping_add(xx as u32).wrapping_sub(1);
    // seed: the 2x2 pixels (x-1..x, y-1..y) become the gain-weighted sum
    let p = m.r16(edi) as u32; let r = m.r16(w32(edi, pitch)) as u32;
    let mut mm0 = dupbw(p as u64); let mut mm1 = dupbw(r as u64);
    mm0 = punpcklwd(mm0, mm0); mm1 = punpcklwd(mm1, mm1);
    mm1 = psraw(mm1, 8); mm0 = psraw(mm0, 8);
    mm0 = paddw(mm0, mm1); mm0 = pmaddwd(mm0, mm6); mm0 = psrad(mm0, 7);
    mm0 = packssdw_same(mm0); mm0 = packsswb_same(mm0);
    let v = mm0 as u16; m.w16(edi, v); m.w16(w32(edi, pitch), v);
    let (mut x, mut y) = (xdone, ydone);
    let (e0, e1) = (1i32, pitch);
    sweep(m, e0, e1, edi, ebp_tab, x, y, qq, row_stride);
    let (mut eax2, mut edx2) = (-e0, e1);
    edi = w32(edi, e0); x = w - x;
    sweep(m, eax2, edx2, edi, ebp_tab, x, y, qq, row_stride);
    edi = w32(edi, edx2); edx2 = -edx2; y = h - y;
    sweep(m, eax2, edx2, edi, ebp_tab, x, y, qq, row_stride);
    edi = w32(edi, eax2); eax2 = -eax2; x = w - x;
    sweep(m, eax2, edx2, edi, ebp_tab, x, y, qq, row_stride);
}
