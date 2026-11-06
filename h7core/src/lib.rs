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

/// Render at w x h with aa x aa supersampling (letterboxed to the original 4:3 frame with a 320:176 picture).
pub fn render(t: f32, w: usize, h: usize, aa: usize) -> Frame {
    let mut rgba = vec![0u8; w * h * 4];
    let pic_h = h * PIC_H / NATIVE_H;
    let y0 = (h - pic_h) / 2;
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
        assert_eq!(&f.rgba[0..4], &[0, 0, 0, 255]);          // top bar
        let mid = (120 * 320 + 160) * 4; assert!(f.rgba[mid + 2] > 0);
    }
}
