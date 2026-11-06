//! The original's music, exactly: the MXM tracker/synth (0x40b84d load, 0x40bee1 start, mixer 0x40e594 / 0x40ea58 fill, all in the static recompile `orig_gen.rs`) run on the
//! DirectSound fake of `win32.rs`. The original's mixer thread (0x40ea16) loops `Sleep(5); GetCurrentPosition; fill [write pos .. play cursor & ~0xff)` into a 4 s looping secondary buffer
//! (44.1 kHz, 16-bit stereo, 705,600 bytes) after pre-filling the whole buffer once; the PCM stream = every Lock/Unlock chunk in write order. The chunking (fill schedule) decides on which chunk
//! a sequencer tick lands (ticks run at chunk start when the chunk END reaches the tick time), so the schedule is part of the definition: `PERIOD_US` = an ideal 5 ms thread (play cursor = floor(k * 220.5) frames).
use crate::cold::IMAGE;
use crate::scene::*;
use crate::x86rt::Cpu;

pub const RATE: usize = 44100;
/// thread period: 5 ms of play time per fill iteration -> k-th cursor = (k*441/2) frames
const SENTINEL: u32 = 0xdead_0000;

pub struct Synth { m: Mem, c: Cpu, ctx: u32, k: u64, size: u32, /** interleaved stereo i16 produced so far */ pub pcm: Vec<i16> }
impl Synth {
    /// start-up of the original with sound enabled up to the first main-loop iteration (0x401252): scene build, MXM load, DS init + pre-fill of the whole buffer
    pub fn new() -> Synth {
        let mut m = Mem { img: IMAGE.to_vec(), heap: vec![0; 0x100_0000], stk: vec![0; STACK_LEN], log: None };
        let mut c = Cpu::new();
        c.w.audio = Some(Default::default());
        c.r[4] = 0x00ff_ff00; m.w32(c.r[4], 0x7f00_0040);
        c.stop_pc = 0x401252; c.stop_n = 1;
        crate::orig_gen::run(&mut c, &mut m, crate::cold::OEP);
        let ctx = m.r32(0x42a400); let size = c.w.audio.as_ref().unwrap().size;
        let mut s = Synth { m, c, ctx, k: 0, size, pcm: vec![] };
        s.drain(); s
    }
    fn drain(&mut self) {
        let o = std::mem::take(&mut self.c.w.audio.as_mut().unwrap().out);
        self.pcm.extend(o.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])));
    }
    /// one iteration of the mixer thread loop body at the play cursor of the k-th 5 ms tick
    fn step(&mut self) {
        self.k += 1;
        let play = ((self.k * 441 / 2) as u32).wrapping_mul(4) % self.size;
        self.c.w.audio.as_mut().unwrap().play = play;
        self.c.stop_pc = u32::MAX; self.c.r[4] = 0x00ff_e000; self.c.r[5] = self.ctx; self.c.r[3] = play;
        self.c.push(&mut self.m, SENTINEL);
        crate::orig_gen::run(&mut self.c, &mut self.m, 0x40ea58);
        self.drain();
    }
    /// extend the stream until at least `frames` stereo frames exist
    pub fn ensure(&mut self, frames: usize) { while self.pcm.len() < frames * 2 { self.step(); } }
}

/// Interleaved stereo i16 (L, R, ...) of the frames `sample_from .. sample_from + count` of the original's music (44.1 kHz). The synth state is sequential, so the stream is generated once, forward,
/// and cached (about 88 KB per second); a request behind the generated end is a slice of the cache.
pub fn render_pcm(sample_from: usize, count: usize) -> Vec<i16> {
    use std::sync::{Mutex, OnceLock};
    static S: OnceLock<Mutex<Synth>> = OnceLock::new();
    let mut s = S.get_or_init(|| Mutex::new(Synth::new())).lock().unwrap();
    s.ensure(sample_from + count);
    s.pcm[sample_from * 2..(sample_from + count) * 2].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference() -> Option<Vec<i16>> {
        let p = std::env::var("H7PCM").unwrap_or("/tmp/claude-1000/aud/pcm_emu.raw".into());
        std::fs::read(p).ok().map(|r| r.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect())
    }
    /// PCM of the original (Unicorn run, re/h7emu.py H7AUDIO=1) vs the recompiled synth: H7ASEC seconds (default 30)
    #[test] fn pcm_matches_the_emulator() {
        let Some(r) = reference() else { eprintln!("no reference pcm"); return; };
        let secs: usize = std::env::var("H7ASEC").ok().and_then(|x| x.parse().ok()).unwrap_or(30);
        let n = (secs * RATE).min(r.len() / 2);
        let t0 = std::time::Instant::now();
        let p = render_pcm(0, n);
        eprintln!("{} s of audio in {:?}", n as f64 / RATE as f64, t0.elapsed());
        let bad: Vec<usize> = (0..n * 2).filter(|&i| p[i] != r[i]).collect();
        eprintln!("differing samples: {} of {} (first {:?})", bad.len(), n * 2, &bad[..bad.len().min(8)]);
        assert!(bad.is_empty());
    }
}
