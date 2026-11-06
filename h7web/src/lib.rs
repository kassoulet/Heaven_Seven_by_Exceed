//! h7web - raw wasm exports (no wasm-bindgen): exact 320x240 RGBA frames of the original demo via h7core::seek::Seeker.
use std::cell::RefCell;
use h7core::seek::Seeker;
thread_local! { static S: RefCell<Option<Seeker>> = RefCell::new(None); static OUT: RefCell<Vec<u8>> = RefCell::new(vec![]); }
/// build the cold state (a few seconds), keep at most `budget_mb` of seek checkpoints
#[no_mangle] pub extern "C" fn init(budget_mb: u32) { S.with(|s| *s.borrow_mut() = Some(Seeker::new(40, 25, (budget_mb as usize) << 20))); }
#[no_mangle] pub extern "C" fn frames() -> u32 { h7core::FRAMES }
/// frame k (1-based, 40 ms grid) as 320x240 RGBA; returns a pointer valid until the next call
#[no_mangle] pub extern "C" fn frame(k: u32) -> *const u8 {
    let f = S.with(|s| s.borrow_mut().as_mut().expect("init first").frame(k.clamp(1, h7core::FRAMES)));
    OUT.with(|o| { let mut o = o.borrow_mut(); o.clear(); for p in f.chunks_exact(4) { o.extend_from_slice(&[p[2], p[1], p[0], 255]); } o.as_ptr() })
}
// AUDIO hook (stub): will become `pcm(from_sample, n) -> *const i16` over h7core::audio::render_pcm once it exists.
#[no_mangle] pub extern "C" fn audio_ready() -> u32 { 0 }
