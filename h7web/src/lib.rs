//! h7web - raw wasm exports (no wasm-bindgen): render a frame into a static buffer, optionally through the CRT pass.
use h7core::{render, crt};
static mut BUF: Vec<u8> = Vec::new();
#[no_mangle] pub extern "C" fn frame(t: f32, w: u32, h: u32, aa: u32, use_crt: u32) -> *const u8 {
    let f = render(t, w as usize, h as usize, aa as usize);
    let f = if use_crt != 0 { crt(&f) } else { f };
    unsafe { let b = &mut *std::ptr::addr_of_mut!(BUF); *b = f.rgba; b.as_ptr() }
}
