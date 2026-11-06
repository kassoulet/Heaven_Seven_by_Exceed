//! The Win32 / DirectDraw / DirectSound surface the original talks to during start-up, as the emulator harness (re/h7emu.py `api`) fakes it - reproduced exactly, including the
//! bump allocator (the heap layout of the original's state depends on it) and the virtual clock (advances 40 ms per presented frame, so frame k is the demo at k*40 ms).
//! Import thunks live at 0x7f000100.. (IAT of the unpacked image), COM method thunks at 0x7f080000 + 16*index.
use crate::scene::{Mem, HEAP_BASE};
use crate::x86rt::Cpu;
use std::collections::HashMap;

#[derive(Default)]
pub struct Win32 { pub vt: u32, pub heap_end: u32, pub fb_buf: u32, pub com: HashMap<u32, u8>, pub presents: u32, pub exited: bool }
const DD: u8 = 1; const SURF: u8 = 2;
const DDN: [u32; 22] = [2, 0, 0, 0, 3, 4, 3, 2, 4, 4, 0, 2, 1, 2, 1, 1, 1, 1, 1, 0, 2, 3];
const SFN: [u32; 40] = [2, 0, 0, 1, 1, 5, 3, 5, 2, 2, 3, 2, 2, 1, 1, 1, 2, 1, 1, 2, 1, 1, 1, 2, 0, 4, 1, 0, 1, 2, 2, 1, 1, 5, 2, 2, 2, 2, 2, 2];
pub const STEP: u32 = 40;
pub const FB_W: u32 = 320; pub const FB_H: u32 = 240;

fn alloc(c: &mut Cpu, m: &mut Mem, n: u32) -> u32 {
    if c.w.heap_end == 0 { c.w.heap_end = HEAP_BASE; }
    let a = c.w.heap_end; c.w.heap_end += (n + 0xfff) & !0xf;
    let need = (c.w.heap_end - HEAP_BASE) as usize + 0x2000; if m.heap.len() < need { m.heap.resize(need, 0); }
    a
}
fn mkcom(c: &mut Cpu, m: &mut Mem, tag: u8) -> u32 {
    let vt = alloc(c, m, 64 * 4); let obj = alloc(c, m, 16); m.w32(obj, vt);
    for i in 0..64 { m.w32(vt + i * 4, 0x7f08_0000 + i * 16); }
    c.w.com.insert(obj, tag); obj
}
/// stdcall return: pop the return address and n argument dwords, eax = v
fn ret(c: &mut Cpu, m: &mut Mem, n: u32, v: u32) -> u32 { let sp = c.r[4]; let r = m.r32(sp); c.r[0] = v; c.r[4] = sp + 4 + n * 4; r }

/// called when the recompiled code branches to an address >= 0x7f000000; returns the next pc
pub fn call(c: &mut Cpu, m: &mut Mem, pc: u32) -> u32 {
    let sp = c.r[4]; let a: Vec<u32> = (0..6).map(|i| m.r32(sp + 4 + 4 * i)).collect();
    if (0x7f08_0000..0x7f09_0000).contains(&pc) {
        let idx = ((pc - 0x7f08_0000) / 16) as usize; let this = a[0]; let tag = *c.w.com.get(&this).unwrap_or(&0);
        let n = if tag == DD { DDN.get(idx).copied().unwrap_or(0) } else { SFN.get(idx).copied().unwrap_or(0) };
        match (tag, idx) {
            (DD, 6) => { let s = mkcom(c, m, SURF); m.w32(a[2], s); }
            (SURF, 22) => { let d0 = a[1]; m.w32(d0 + 4, 0x100f); m.w32(d0 + 8, FB_H); m.w32(d0 + 12, FB_W); for (i, v) in [32u32, 0x40, 0, 32, 0xff0000, 0xff00, 0xff].iter().enumerate() { m.w32(d0 + 72 + 4 * i as u32, *v); } }
            (SURF, 12) => { let s = mkcom(c, m, SURF); m.w32(a[2], s); }
            (SURF, 25) => {
                if c.w.fb_buf == 0 { let b = alloc(c, m, FB_W * FB_H * 4 + 4096); c.w.fb_buf = b; }
                m.w32(a[2] + 4, 0x100f); m.w32(a[2] + 8, FB_H); m.w32(a[2] + 12, FB_W); m.w32(a[2] + 16, FB_W * 4); m.w32(a[2] + 36, c.w.fb_buf);
                for (i, v) in [32u32, 0x40, 0, 32, 0xff0000, 0xff00, 0xff].iter().enumerate() { m.w32(a[2] + 72 + 4 * i as u32, *v); }
            }
            (SURF, 5) if a[2] != 0 && c.w.fb_buf != 0 => { c.w.presents += 1; c.w.vt += STEP; }   // Blt to the primary = present
            _ => {}
        }
        return ret(c, m, n + 1, 0);
    }
    match pc {
        0x7f00_0040 | 0x7f00_0150 => { c.w.exited = true; 0xdead_0000 }                     // ExitProcess
        0x7f00_0100 => { let s = alloc(c, m, 16); m.bytes_mut(s, 3).copy_from_slice(b"h7\0"); ret(c, m, 0, s) }          // GetCommandLineA
        0x7f00_01b0 => { let r = alloc(c, m, a[1]); ret(c, m, 2, r) }                       // GlobalAlloc
        0x7f00_0130 => ret(c, m, 1, 0x400000),                                              // GetModuleHandleA
        0x7f00_0240 => ret(c, m, 1, if a[0] == 0 { 1024 } else { 768 }),                   // GetSystemMetrics
        0x7f00_01e0 => { let o = mkcom(c, m, DD); m.w32(a[1], o); ret(c, m, 3, 0) }          // DirectDrawCreate
        0x7f00_02e0 => ret(c, m, 5, 0),                                                     // PeekMessageA: no messages
        0x7f00_0310 => { let v = c.w.vt; ret(c, m, 0, v) }                                  // timeGetTime
        0x7f00_0300 => { let v = match a[1] { 0x3eb => 0, 0x3ec => 2, _ => if a[2] == 0xf0 && false { 1 } else { 0 } }; ret(c, m, 5, v) }   // SendDlgItemMessageA: resolution 0 (320x240), sound 2 = disabled
        0x7f00_02f0 => {      // DialogBoxIndirectParamA: the harness calls the dialog procedure with WM_INITDIALOG (0x110) and WM_COMMAND/IDOK (0x111), then returns 1
            let proc = a[3];
            let sp0 = c.r[4]; let ra = m.r32(sp0);
            for (msg, wp) in [(0x110u32, 0u32), (0x111, 1)] {
                c.push(m, 0); c.push(m, wp); c.push(m, msg); c.push(m, 1);   // lParam, wParam, msg, hwnd (stdcall)
                c.push(m, 0xdead_0000);
                crate::load_gen::run(c, m, proc);
            }
            c.r[0] = 1; c.r[4] = sp0 + 4 + 5 * 4; ra
        }
        0x7f00_0210 => ret(c, m, 2, 1),                                                     // EndDialog
        0x7f00_0200 => ret(c, m, 12, 1), 0x7f00_02a0 => ret(c, m, 1, 1), 0x7f00_0270 => ret(c, m, 2, 1), 0x7f00_0250 => ret(c, m, 1, 1),   // CreateWindowExA, RegisterClassA, ShowWindow, SetCursor
        0x7f00_01f0 => ret(c, m, 3, 1),                                                     // DirectSound ordinal 1 (DirectSoundCreate): fails -> sound off
        _ => panic!("win32 import {pc:#x} not modelled"),
    }
}
