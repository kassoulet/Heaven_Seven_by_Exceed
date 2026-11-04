//! h7gen - offline generator: renders frames with supersampling and streams raw RGBA to ffmpeg.
//! usage: h7gen [--w 3840 --h 2880|--w 3840 --h 2160] [--aa 3] [--fps 60] [--secs 10] [--crt] [--out h7.mp4] [--audio file.wav]
use std::io::Write; use std::process::{Command, Stdio};
fn main() {
    let mut a = std::env::args().skip(1);
    let (mut w, mut h, mut aa, mut fps, mut secs, mut crt, mut out, mut audio) = (3840usize, 2880usize, 2usize, 60u32, 10f32, false, "h7.mp4".to_string(), None::<String>);
    while let Some(k) = a.next() { match k.as_str() {
        "--w" => w = a.next().unwrap().parse().unwrap(), "--h" => h = a.next().unwrap().parse().unwrap(),
        "--aa" => aa = a.next().unwrap().parse().unwrap(), "--fps" => fps = a.next().unwrap().parse().unwrap(),
        "--secs" => secs = a.next().unwrap().parse().unwrap(), "--crt" => crt = true,
        "--out" => out = a.next().unwrap(), "--audio" => audio = a.next(), _ => panic!("unknown {k}") } }
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-"]);
    if let Some(wav) = &audio { cmd.args(["-i", wav, "-shortest"]); }
    cmd.args(["-c:v", "libx264", "-crf", "16", "-pix_fmt", "yuv420p", &out]).stdin(Stdio::piped()).stderr(Stdio::null());
    let mut ff = cmd.spawn().expect("ffmpeg");
    let si = ff.stdin.as_mut().unwrap();
    let n = (secs * fps as f32) as u32;
    for i in 0..n {
        let f = h7core::render(i as f32 / fps as f32, w, h, aa);
        let f = if crt { h7core::crt(&f) } else { f };
        si.write_all(&f.rgba).unwrap();
    }
    drop(ff.stdin.take()); ff.wait().unwrap();
}
