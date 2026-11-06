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
pub const NATIVE_W: usize = 320;
pub const NATIVE_H: usize = 240;
pub const PIC_H: usize = 176; // the original letterboxes the picture to 320x176 inside 320x240

pub struct Frame { pub w: usize, pub h: usize, pub rgba: Vec<u8> }

/// Placeholder scene until the effects are ported: a vertical gradient with a moving light shaft.
fn scene_pixel(u: f32, v: f32, t: f32) -> [f32; 3] {
    let cx = 0.5 + 0.05 * (t * 0.7).sin();
    let d = ((u - cx).abs() * 6.0 + v * 2.0).max(0.0);
    let l = (-d).exp();
    [0.05 + 0.9 * l, 0.1 + 0.9 * l, 0.15 + 0.95 * l]
}

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

/// Placeholder scene render (no 3D engine yet) at w x h with aa x aa supersampling: the picture area is the TOP 320:176 part of the frame like the original's, the bars below are black.
pub fn render(t: f32, w: usize, h: usize, aa: usize) -> Frame {
    let mut rgba = vec![0u8; w * h * 4];
    let pic_h = h * PIC_H / NATIVE_H;
    let y0 = 0;
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            rgba[o + 3] = 255;
            if y < y0 || y >= y0 + pic_h { continue; }
            let mut acc = [0f32; 3];
            for sy in 0..aa { for sx in 0..aa {
                let u = (x as f32 + (sx as f32 + 0.5) / aa as f32) / w as f32;
                let v = ((y - y0) as f32 + (sy as f32 + 0.5) / aa as f32) / pic_h as f32;
                let c = scene_pixel(u, v, t);
                for k in 0..3 { acc[k] += c[k]; }
            }}
            let n = (aa * aa) as f32;
            for k in 0..3 { rgba[o + k] = (acc[k] / n * 255.0).clamp(0.0, 255.0) as u8; }
        }
    }
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
        let mid = (88 * 320 + 160) * 4; assert!(f.rgba[mid + 2] > 0);                 // picture area = top 176 rows
        let bar = (200 * 320 + 160) * 4; assert_eq!(&f.rgba[bar..bar + 4], &[0, 0, 0, 255]);   // black bars at the bottom
    }
}
