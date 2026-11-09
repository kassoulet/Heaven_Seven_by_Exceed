# Heaven Seven (Exceed, Mekka & Symposium 2000) - bit-exact Rust/wasm port

Play: `cd h7web && python3 -m http.server`, open http://localhost:8000/ (pause, seek, CRT filter, mute; music from the original synth).
Seeking far ahead is slow for now (the video replays from the start until checkpoints exist; it blocks the page).

- `h7core` the original, statically recompiled, plus verified hand ports; `h7web` wasm player; `h7gen` frames/WAV for ffmpeg; `re/` reverse engineering and `NOTES.md` (full log).
- Build: `cargo build --release`; wasm: `cargo build --release --target wasm32-unknown-unknown -p h7web` and copy the `.wasm` into `h7web/`.
- Video: `h7gen --out - | ffmpeg -f rawvideo -pix_fmt rgba -s 320x240 -r 25 -i - ...`; music: `h7gen --audio demo.wav`.
- Write-ups: `WRITEUP-effects.md` (how it is built and how it was reversed), `WRITEUP-asm.md` (the best asm snippets). Web page: `h7web/about.html`.
- Result: 4181/4181 frames and 15,435,000 PCM samples identical to the original run in an emulator.
