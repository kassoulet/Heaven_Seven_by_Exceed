# Heaven Seven - how the demo is built

**Heaven Seven** by Exceed (Mekka & Symposium 2000, 64k intro; the Win32 `HEAVEN7W.EXE` is 65,536 bytes after UPX, 112 KB unpacked). Pure software renderer: DirectDraw only to blit one surface, DirectSound for the music (it also provides the clock when sound is on). 168 s, 4181 frames on a 25 fps grid in this port.

## Phases
1. **"please wait"**: every texture is generated procedurally (about 1.5 G x86 instructions in the emulator). 67 % of the first 200 M instructions are in the crack-growth routine 0x409c7c.
2. **Playback**: the music thread fills a 4 s looping DirectSound buffer; the main loop converts the clock to demo time and evaluates the scene graph.

## An operator-graph engine, not hard-coded effects
A bytecode stream builds operator instances (0xa8 bytes each: kind, inputs, three handlers, time accumulator, animation channels). Textures and effects are the same graph, evaluated once (textures) or per frame (scene). Node classes include text (bitmap font with MMX blur and a bilinear glyph blitter), sprites (nearest / rotated / unscaled blitters), glow streaks, a spline animation system (TCB-like tangents, one channel evaluator for everything), and a 3D pass.

## The 3D pass
A subdivision ray caster: per-line records, object ray/plane/sphere tests through a vtable, MMX span shaders (0x4036af / 0x404234). About 1,850 distinct instructions.

## What the port does
`h7core` does not re-implement the effects by hand for the final result: it **statically recompiles the original x86** (17,063 instructions, 2,391 basic blocks, `re/mk_full.py` + `re/recomp.py` -> `orig_gen.rs`), runs it on a small x86/x87/MMX runtime, and fakes only the Win32/DirectDraw/DirectSound calls. Hand ports (texture ops, blitters, scene evaluator, 3D set-up) were written first, each verified against the emulator with differential vectors (thousands in total), and remain as readable models.
- **Frames:** all 4181 frames bit-exact from a cold start. The scene is not a pure function of time (rotating objects accumulate float rounding every frame), so frames are only exact on the 40 ms grid stepped from the start; seeking replays from a checkpoint.
- **Music:** an MXM tracker module synthesised at 44.1 kHz stereo (32 channels, MMX 16.16 linear resampling with volume ramps). 15,435,000 samples = 175 s, 0 differing samples vs the emulator, defined as an ideal 5 ms mixer thread (the fill schedule changes note onsets by a few ms).
- **Display:** 320x240 with the picture letterboxed at 320x176 at the top (see `--crop` in `h7gen`), shown at 4:3. An optional WebGL CRT pass (display only) adds curvature, scanlines, shadow mask and bloom.

## Method
Unpack with Unicorn (run the UPX stub, stop at the first fetch from the unpacked section) -> harness with fake DirectX and a virtual clock -> trace and profile -> port class by class with differential tests -> recompile what is too big to port -> compare every frame and sample. The full log, with the dead ends, is `re/NOTES.md`.
