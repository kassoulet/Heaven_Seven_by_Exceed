//! Cheap seeking for `cold::Player`: checkpoints of the whole state (image + heap + stack + Cpu) every `every` frames, stored as byte-run deltas against the frame-0 state
//! (about 0.7 MB each: the scene only rewrites ~5% of the 13 MB heap). Restore = clone the base + patch one delta, then replay at most `every`-1 frames.
//! Memory is bounded: when the checkpoints exceed `budget` bytes every second one is dropped and `every` doubles. The scene is deterministic, so seeks are bit-exact (tests).
use crate::cold::Player;
use crate::scene::Mem;
use crate::x86rt::Cpu;

struct Cp { k: u32, c: Cpu, lens: [usize; 3], runs: Vec<(u32, u32)>, data: Vec<u8> }   // run = (region << 28 | offset, len); bytes are concatenated in `data`
pub struct Seeker { pub p: Player, base: (Mem, Cpu), cps: Vec<Cp>, pub every: u32, pub budget: usize }

fn regions(m: &Mem) -> [&Vec<u8>; 3] { [&m.img, &m.heap, &m.stk] }
impl Seeker {
    pub fn new(step_ms: u32, every: u32, budget: usize) -> Seeker {
        let p = Player::new(step_ms); let base = (p.m.clone(), p.c.clone());
        Seeker { p, base, cps: vec![], every, budget }
    }
    /// bytes held by the checkpoints
    pub fn bytes(&self) -> usize { self.cps.iter().map(|c| c.data.len() + c.runs.len() * 8).sum() }
    fn checkpoint(&mut self) {
        let (b, m) = (regions(&self.base.0), regions(&self.p.m)); let (mut runs, mut data) = (vec![], vec![]);
        for r in 0..3 {
            let (x, y) = (b[r], m[r]); let n = y.len(); let mut i = 0;
            while i < n {
                // skip equal 8-byte words quickly
                while i + 8 <= n && i + 8 <= x.len() && x[i..i + 8] == y[i..i + 8] { i += 8; }
                if i >= n { break; }
                if i < x.len() && x[i] == y[i] { i += 1; continue; }
                let s = i; let mut last = i;   // run: extend while a differing byte occurs within 16 bytes of the last one
                while i < n && i - last < 16 { if i >= x.len() || x[i] != y[i] { last = i; } i += 1; }
                runs.push(((r as u32) << 28 | s as u32, (last + 1 - s) as u32)); data.extend_from_slice(&y[s..=last]); i = last + 1;
            }
        }
        self.cps.push(Cp { k: self.p.k, c: self.p.c.clone(), lens: [m[0].len(), m[1].len(), m[2].len()], runs, data });
        while self.bytes() > self.budget && self.cps.len() > 1 { self.every *= 2; let e = self.every; self.cps.retain(|c| c.k % e == 0); }
    }
    fn restore(&mut self, j: Option<usize>) {
        let (mut m, mut c) = self.base.clone();
        if let Some(j) = j {
            let cp = &self.cps[j]; c = cp.c.clone();
            for (r, v) in [&mut m.img, &mut m.heap, &mut m.stk].into_iter().enumerate() { v.resize(cp.lens[r], 0); }
            let mut d = 0usize;
            for &(ro, len) in &cp.runs { let (r, o, len) = (ro >> 28, (ro & 0x0fff_ffff) as usize, len as usize);
                let v = match r { 0 => &mut m.img, 1 => &mut m.heap, _ => &mut m.stk }; v[o..o + len].copy_from_slice(&cp.data[d..d + len]); d += len; }
            self.p.k = cp.k;
        } else { self.p.k = 0; }
        self.p.m = m; self.p.c = c;
    }
    fn fb(&self) -> Vec<u8> { self.p.m.bytes(self.p.c.w.fb_buf, crate::NATIVE_W * crate::NATIVE_H * 4).to_vec() }
    /// BGRX frame k (1-based, like Player). Steps forward when that is cheaper than a restore, else restores the nearest checkpoint <= k first.
    pub fn frame(&mut self, k: u32) -> Vec<u8> {
        let k = k.max(1);
        if self.p.k != k && (self.p.k > k || k - self.p.k > 1) {
            let j = self.cps.iter().rposition(|c| c.k <= k);
            let from = j.map_or(0, |j| self.cps[j].k);
            if self.p.k > k || from > self.p.k { self.restore(j); }
        }
        while self.p.k < k { self.p.step(); if self.p.k % self.every == 0 && self.cps.last().map_or(true, |c| c.k < self.p.k) { self.checkpoint(); } }
        self.fb()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// seek == continuous playback, bit-exact, and measure the delta memory / timing
    #[test] fn seek_equals_playback() {
        let n: u32 = std::env::var("H7N").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
        let mut a = Player::new(40); let want: Vec<Vec<u8>> = (0..n).map(|_| a.step()).collect();
        let mut s = Seeker::new(40, 10, 1 << 30);
        let t0 = std::time::Instant::now(); for k in 1..=n { assert!(s.frame(k) == want[k as usize - 1], "play {k}"); }
        eprintln!("play {n}: {:?}, checkpoints {} = {} KB", t0.elapsed(), s.cps.len(), s.bytes() / 1024);
        for &k in &[n, 3, n / 2, 1, n - 1, 11, 10, 2, n] { let t0 = std::time::Instant::now(); let f = s.frame(k); eprintln!("seek {k}: {:?}", t0.elapsed()); assert!(f == want[k as usize - 1], "seek {k}"); }
    }
}
