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
/// the original's music: 44.1 kHz stereo i16 (L,R,...) frames `from..from+n` (clamped to the end); pointer valid until the next call. First call builds the synth (~4 s), then ~0.06 s per second of audio.
#[no_mangle] pub extern "C" fn pcm(from: u32, n: u32) -> *const i16 {
    thread_local! { static P: RefCell<Vec<i16>> = RefCell::new(vec![]); }
    let from = (from as usize).min(PCM_FRAMES); let n = (n as usize).min(PCM_FRAMES - from);
    P.with(|p| { *p.borrow_mut() = h7core::audio::render_pcm(from, n); p.borrow().as_ptr() })
}
const PCM_FRAMES: usize = 15_435_000; // 175 s, the whole stream (audio.rs / NOTES)
#[no_mangle] pub extern "C" fn pcm_frames() -> u32 { PCM_FRAMES as u32 }
