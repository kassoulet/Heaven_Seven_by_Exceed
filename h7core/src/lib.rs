//! h7core - Heaven Seven (Exceed, 2000) reconstruction core.
//! `render(t, w, h, aa)` is a pure function of the demo time; the exact mode is w=320,h=240 (picture area 320x176, letterboxed), aa=1.
pub mod pack;
pub mod ops;
pub mod anim;
pub mod mmx;
pub mod font;
pub mod blit;
pub mod text;
pub mod texops;
pub mod imgops;
pub mod imgprog;
pub mod texscript;
pub mod scene;
pub mod streak;
pub mod mat3;
pub mod three_d;
pub mod engine;
pub mod x86rt;
pub mod raster_gen;
pub mod win32;
pub mod orig_gen;
pub mod cold;
pub mod seek;
pub mod audio;
pub const NATIVE_W: usize = 320;
pub const NATIVE_H: usize = 240;
pub const FRAMES: u32 = 4181; // frames of the whole demo on the 40 ms grid (167.2 s: the evaluator reports the end after frame 4181)
pub const PIC_H: usize = 176; // the original letterboxes the picture to 320x176 inside 320x240

pub struct Frame { pub w: usize, pub h: usize, pub rgba: Vec<u8> }

/// The presented 320x240 frame of a `scene::Scene` (what 0x401565 does): the viewport's 320x176 32-bit buffer (B,G,R,0 bytes) is copied to the TOP of the frame, the 64 rows below stay black
/// (measured on the original's frames: the picture is NOT centred). Returns RGBA, alpha 255.
pub fn present(s: &scene::Scene) -> Frame {
    let root = s.m.r32(s.ebp + 0x50); let vp = s.m.r32(root + 0x18);
    let (w, h, d) = (s.m.r32(vp + 0x18) as usize, s.m.r32(vp + 0x1c) as usize, s.m.r32(vp + 0x24));
    let mut rgba = vec![0u8; NATIVE_W * NATIVE_H * 4];
    for y in 0..NATIVE_H { for x in 0..NATIVE_W { rgba[(y * NATIVE_W + x) * 4 + 3] = 255; } }
    for y in 0..h.min(NATIVE_H) { for x in 0..w.min(NATIVE_W) {
        let a = d + ((y * w + x) * 4) as u32; let o = (y * NATIVE_W + x) * 4;
        rgba[o] = s.m.r8(a + 2); rgba[o + 1] = s.m.r8(a + 1); rgba[o + 2] = s.m.r8(a);
    } }
    Frame { w: NATIVE_W, h: NATIVE_H, rgba }
}

/// The demo frame at time `t` seconds, w x h (aa = supersampling is meaningless for the exact path and ignored): the exact 320x240 frame of the original, nearest-neighbour scaled for other sizes.
/// The frame is computed by running the original's own code (static recompile) from the cold start on the 40 ms frame grid (`cold::Player`: the scene accumulates state frame by frame, so a seek replays the grid
/// from the start - about 1 frame/10 ms in release); consecutive calls with increasing t reuse the player. Frames are exact (0 differing bytes) vs the emulator at the same grid.
pub fn render(t: f32, w: usize, h: usize, _aa: usize) -> Frame {
    use std::sync::{Mutex, OnceLock};
    static P: OnceLock<Mutex<(cold::Player, Vec<u8>)>> = OnceLock::new();
    let k = ((t.max(0.0) * 1000.0 / 40.0) as u32 + 1).min(4181);          // frame k is shown at (k-1)*40 ms; the demo ends after frame 4181 (t = 167.2 s: the evaluator then reports the end)
    let mut g = P.get_or_init(|| Mutex::new((cold::Player::new(40), vec![]))).lock().unwrap();
    if g.0.k > k || g.1.is_empty() { *g = (cold::Player::new(40), vec![]); }
    while g.0.k < k { let f = g.0.step(); g.1 = f; }
    let src = &g.1; let mut rgba = vec![255u8; w * h * 4];
    for y in 0..h { for x in 0..w {
        let (sx, sy) = (x * NATIVE_W / w, y * NATIVE_H / h); let a = (sy * NATIVE_W + sx) * 4; let o = (y * w + x) * 4;
        rgba[o] = src[a + 2]; rgba[o + 1] = src[a + 1]; rgba[o + 2] = src[a];
    } }
    Frame { w, h, rgba }
}

/// CRT emulation (post-process only, never part of the exact core): scanlines, slot-mask-ish RGB phosphor offset and a soft bloom/vignette.
pub fn crt(f: &Frame) -> Frame {
    let (w, h) = (f.w, f.h);
    let mut out = vec![255u8; w * h * 4];
    for y in 0..h {
        let scan = 0.78 + 0.22 * (std::f32::consts::PI * (y as f32 + 0.5) * (240.0 / h as f32) * 1.0).cos().abs().powf(0.5);
        for x in 0..w {
            let vx = (x as f32 / w as f32 - 0.5) * 2.0; let vy = (y as f32 / h as f32 - 0.5) * 2.0;
            let vig = 1.0 - 0.25 * (vx * vx + vy * vy);
            for k in 0..3 {
                let xs = (x as i32 + k as i32 - 1).clamp(0, w as i32 - 1) as usize;   // phosphor offset per channel
                let c = f.rgba[(y * w + xs) * 4 + k] as f32 / 255.0;
                out[(y * w + x) * 4 + k] = (c.powf(1.1) * scan * vig * 255.0).clamp(0.0, 255.0) as u8;
            }
        }
    }
    Frame { w, h, rgba: out }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn renders_letterboxed() {
        let f = render(1.0, 320, 240, 1);
        assert_eq!(f.rgba.len(), 320 * 240 * 4);
        let mid = (88 * 320 + 160) * 4; assert!(f.rgba[mid..mid + 3].iter().any(|&v| v > 0));                 // picture area = top 176 rows
        let bar = (200 * 320 + 160) * 4; assert_eq!(&f.rgba[bar..bar + 4], &[0, 0, 0, 255]);   // black bars at the bottom
    }
}
