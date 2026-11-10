//! h7gen - exact frames of the original demo, raw to stdout (or a file) for ffmpeg.
//!   h7gen --size 320x240 --fps 25 --out - | ffmpeg -f rawvideo -pix_fmt rgba -s 320x240 -r 25 -i - -c:v libx264 -pix_fmt yuv420p demo.mp4
//! Frames come from the original's 40 ms grid (25 fps); other --fps repeat/skip grid frames (nearest earlier). --size other than 320x240 is nearest-neighbour (exact pixels, no AA).
//! --crop picture = only the top 320x176 (the original letterboxes the picture to 320x176 at the TOP of its 320x240 frame; the 64 rows below are black).
//! --secs N (default: whole demo, 167.2 s), --start S, --bench (no output, print ms/frame), --audio file.wav (music only, no video; then mux with ffmpeg).
use std::io::Write;
fn main() {
    let (mut w, mut h, mut fps, mut secs, mut start, mut out, mut crop, mut bench, mut audio) = (320usize, 240usize, 25f64, 167.2f64, 0.0f64, "-".to_string(), false, false, None::<String>);
    let mut a = std::env::args().skip(1);
    while let Some(k) = a.next() { match k.as_str() {
        "--size" => { let s = a.next().unwrap(); let (x, y) = s.split_once('x').expect("WxH"); w = x.parse().unwrap(); h = y.parse().unwrap(); }
        "--fps" => fps = a.next().unwrap().parse().unwrap(), "--secs" => secs = a.next().unwrap().parse().unwrap(), "--start" => start = a.next().unwrap().parse().unwrap(),
        "--out" => out = a.next().unwrap(), "--crop" => crop = true, "--bench" => bench = true, "--audio" => audio = a.next(),
        _ => { eprintln!("unknown {k}"); std::process::exit(2) } } }
    if let Some(wav) = audio { // 16-bit stereo 44.1 kHz WAV of [start, start+secs) of the original's music; mux with ffmpeg -i video -i wav
        let (from, n) = ((start * 44100.0) as usize, ((secs * 44100.0) as usize).min(15_435_000usize.saturating_sub((start * 44100.0) as usize)));
        let pcm = h7core::audio::render_pcm(from, n); let bytes = (pcm.len() * 2) as u32;
        let mut f = std::io::BufWriter::new(std::fs::File::create(&wav).unwrap());
        f.write_all(&[b"RIFF" as &[u8], &(36 + bytes).to_le_bytes(), b"WAVEfmt ", &16u32.to_le_bytes(), &[1, 0, 2, 0], &44100u32.to_le_bytes(), &176400u32.to_le_bytes(), &[4, 0, 16, 0], b"data", &bytes.to_le_bytes()].concat()).unwrap();
        for v in pcm { f.write_all(&v.to_le_bytes()).unwrap(); }
        eprintln!("{wav}: {n} frames ({:.1} s)", n as f64 / 44100.0); return; }
    let mut sink: Box<dyn Write> = if bench { Box::new(std::io::sink()) } else if out == "-" { Box::new(std::io::stdout().lock()) } else { Box::new(std::io::BufWriter::new(std::fs::File::create(&out).unwrap())) };
    let (nw, nh) = h7core::native_size(); let (sw, sh) = if crop { (nw, nh * 176 / 240) } else { (nw, nh) }; let (w, h) = if w == 320 && h == 240 { (sw, sh) } else { (w, h) };
    let mut sk = h7core::seek::Seeker::new(40, 25, 256 << 20);
    let n = (secs * fps) as u64; let t0 = std::time::Instant::now();
    let mut rgba = vec![255u8; w * h * 4];
    for i in 0..n {
        let k = ((start + i as f64 / fps) * 25.0 + 1e-6) as u32 + 1;     // grid frame shown at this output time
        let f = sk.frame(k.min(h7core::FRAMES));
        for y in 0..h { for x in 0..w { let (sx, sy) = (x * sw / w, y * sh / h); let s = (sy * nw + sx) * 4; let o = (y * w + x) * 4; rgba[o] = f[s + 2]; rgba[o + 1] = f[s + 1]; rgba[o + 2] = f[s]; } }
        if bench && i % 250 == 249 { eprintln!("t={:.0}s: {:.1} ms/frame so far", i as f64 / fps, t0.elapsed().as_secs_f64() * 1000.0 / (i + 1) as f64); }
        if sink.write_all(&rgba).is_err() { break; }    // ffmpeg closed the pipe
    }
    eprintln!("{n} frames in {:?} ({:.2} ms/frame)", t0.elapsed(), t0.elapsed().as_secs_f64() * 1000.0 / n.max(1) as f64);
}
