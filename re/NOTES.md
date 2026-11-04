# Heaven Seven (Exceed, 64k intro, Mekka/Symposium 2000) - recon

Files: HEAVEN7D.EXE (DOS, LE, UPX 1.01, Watcom/DOS4GW, 63851 B), HEAVEN7W.EXE (Win32 PE, UPX, 65536 B, the "final" 64k), H7INFO.SWF (Flash info page).
Reversing target: HEAVEN7W.EXE (flat PE, no extender).

## Unpacking
`unpack_upx_pe.py HEAVEN7W.EXE OUT` runs the UPX stub in Unicorn with LoadLibraryA/GetProcAddress/VirtualProtect emulated and stops at the
first fetch from UPX0 -> image.bin (0x400000.., 188416 B, decompressed), info.json (OEP 0x40168c, import list).
Image: code+data in UPX0 0x401000-0x41d000 (112 KB unpacked from 64 KB), imports rebuilt by the stub.

## Imports (tiny): pure software renderer
DDRAW!DirectDrawCreate (one blit surface), DSOUND ord1 = DirectSoundCreate (the music is synthesised, no sample file), WINMM!timeGetTime,
KERNEL32 (CreateThread: sound thread, critical sections, GlobalAlloc), USER32 (window, DialogBoxIndirectParam = start-up dialog).
Start-up dialog strings (0x410000): "Heaven 7", "MMX instruction set required!" (the renderer uses MMX), resolutions 320x240 512x384 640x480 800x600,
sound 44 Khz / 22 Khz / Disabled.

## Text data (0x41a3f4..): "please wait", "we used to dream...", story captions ("being born", "growing", "living", "rising", "shouting",
"collapsing", "without meaning", "the newborn. died right after he was born ...", "the elderly. ... losing his memories ...", "just questions... ...dreams about
a place far away"), credits ".picard .warpig .shaman .stephen", greetings (3state aardbei blasphemy astroidea calodox chrysalis elitegroup haujobb amable
mekka-symposium kolor pathos mandula ramjam replay sunflower tpolm kooma).

## Entry
WinMain 0x400aa5 via CRT stub 0x401000 (command line parse); message pump 0x401040.

## Plan (same pipeline as b-hyper / inside)
1. Map functions (rasteriser/MMX loops, 3D object generator (64k = procedural meshes/textures), synth, scene script/timeline).
2. Emulator harness: Unicorn + stubbed DirectDraw surface (frame dump) + DirectSound buffer (audio dump) + timeGetTime = virtual clock.
3. JS port per scene, accuracy = mean abs error vs emulator frames; audio port of the synth (compare PCM).
4. Web player (pause/seek, aspect ratio), writeup (clever asm snippets), figures.

## Emulator harness (h7emu.py) - works
`h7emu.py [max_instr]` runs image.bin from the OEP in Unicorn: CRT stub 0x40168c -> main 0x401131; the start-up dialog is answered by calling its DlgProc
(0x401000, WM_INITDIALOG then WM_COMMAND/IDOK, list selections 0 = 320x240 / 44 kHz); DirectDraw is a fake COM object (SetCooperativeLevel, SetDisplayMode(320,240,32),
primary + offscreen surface, GetSurfaceDesc / Lock fill a 32-bpp DDSURFACEDESC: lPitch +0x10, lpSurface +0x24, pixel format +0x48); every Unlock/Blt captures the
320x240 BGRX buffer (FB['frames']). Stdcall arg counts are tabulated (NARGS / DDN / SFN) - a wrong count wrecks the stack.
Config struct (ebp in the blitter 0x4015xx): +0x00 bits/pixel-ish, +0x48 = row converter function pointer chosen from the surface's pixel format (that was the crash
until GetSurfaceDesc returned a real format).
First result: 2 G instructions in ~95 s -> 120 frames: "please wait", then the opening light-shaft scene (first_frames.png), letterboxed 320x176 inside 320x240.
Open: timeGetTime is a +14 ms/call fake clock (frame timing is therefore not the demo's), CreateThread (sound/synth thread) is a no-op stub, DirectSound (ord 1) not yet emulated.

## Deterministic frame capture (virtual clock, sound disabled)
`H7SND=2 H7STEP=40 python3 h7emu.py 3000000000`: dialog answered with sound = "Disabled" (list index 2), `timeGetTime` returns a virtual ms clock that advances
STEP ms on every present (Blt to the primary), so frame k is the demo at k*STEP ms. 227 frames (0..9.04 s) in ~150 s of emulation.
The demo clock: `0x4017e9` = timeGetTime * 1193 (0x4a9, units of 1/1193 ms -> used with the 44100 Hz sample clock); with DirectSound enabled the clock is the buffer play
cursor, sampled by the mixer thread 0x40ea16 (Sleep(5); GetCurrentPosition [vtbl+0x10]; fill 0x40ea58 -> Lock [+0x2c], synth/mixer 0x40e50b, Unlock [+0x4c]); the buffer is
0x1c000 bytes (44.1 kHz 16-bit stereo, DSBUFFERDESC at ebp+0x3300), Play [+0x30] looping. Audio capture therefore = emulate that thread (context switch on Sleep) or call the
fill routine directly.
First 9 s (first_9s.png): "please wait" -> light shaft in a cave (procedural rock texture) -> typographic captions ("we used to dream", "used to exist", "asking about places").
Image area is letterboxed 320x176 inside 320x240 (aspect!).

## Deliverables (decided)
1. Web player, 320x240 exact (original letterbox 320x176 inside 320x240), with and without a CRT emulator pass (post-process only, never in the core).
2. Web player hires (render at N x resolution, same scene data, real texture resolution via the procedural generators' size parameters).
3. `h7gen`: native 4K+AA generator streaming raw frames + PCM to ffmpeg (supersampling / analytic AA, audio from the synth).
Architecture: Rust workspace - `h7core` (synth, texture generators, meshes, timeline, `render(t, w, h, aa) -> frame`), `h7web` (wasm-pack, simd128, player glue + CRT shader/pass),
`h7gen` (native CLI, ffmpeg pipe), `h7test` (error vs emulator frames/PCM). Exact mode = original fixed-point/MMX semantics at 320x240; hires/4K = same code path with
the resolution-dependent parts parameterised (generator sizes, raster precision, AA).
4. Writeups (as for b-hyper): WRITEUP-effects.md (every scene/effect, how it is built, figures from the emulator and the port) and WRITEUP-asm.md (the best asm snippets: MMX rasteriser
   loops, procedural texture generators, the synth, the 64k tricks), plus the explanatory website.

## Rust workspace (skeleton, working end to end with a placeholder scene)
`h7core` (render(t,w,h,aa) + crt()), `h7gen` (CLI -> ffmpeg rawvideo pipe, 1280x960 aa2 CRT test: 60 frames h264 OK), `h7web` (wasm32 cdylib, 33 KB, exports `frame(t,w,h,aa,crt)`).
Build: `cargo build --release` (target dir /home/gautier/target); wasm: `cargo build --release --target wasm32-unknown-unknown -p h7web`.
Next: port the real scenes into h7core (texture generators, rasteriser, timeline), then h7test against the emulator frames (re/h7emu.py).

## First function findings (profile of the load phase: H7PROF=a h7emu.py 200000000 -> prof_a.json, per-block instruction counts)
Load ("please wait") = procedural texture generation: 67 % of the first 200 M instructions run in 0x409c00-0x409d00.
- ndisasm prints `call esi` / `call eax` (FF D6 / FF D0) as `db 0xff; salc` - read them as indirect calls (callback style loops, e.g. 0x409c7c with callback esi=0x409cb6).
- 0x409bd7: LCG `seed = seed*0x8088405 + 1` (the Borland/Delphi RandSeed constant), seed at 0x42a3fc, 0x409bed = rnd scaled by ebp (`rnd(n)` = (rand*n)>>32 + base).
- 0x409c0f..0x409c7b: 2x2 box-filter downsample (mip level) of an 8-bit texture: (a+b+c+d+2)>>2 per output texel; texture object: +0x18 w, +0x1c h, +0x20 pitch, +0x28 data.
- 0x409c7c..0x409cd9: distance-from-seed growth (crack/rock structure): per-texel `min(0x7f, 0x17 * ...)` with a 4-neighbour callback; the cave rock wall is built this way.
- 0x409cda: textured/shaded span-ish routine using x87 fild/fmul/fistp with per-object floats at +0x28..+0x44 (particle / sprite projection?), 0x409dec: MMX (psubw/psllw) fixed-point interpolation.
Hot later spots: 0x40e100 (audio synth/mixer, MMX), 0x40d400-0x40dd00 (rasteriser / blend loops), 0x401700 (blit row converters).
Method note: for the Rust port, the generators are small deterministic integer routines - port them one by one and verify bit-exact against emulator memory (texture buffers) - exactly the
b-hyper method; the x86->JS transpiler (b-hyper/tools/x2js.py) could be retargeted to Rust for the awkward ones.

## Main loop and structure (from reading 0x401131..0x4012b4 and 0x409a0c..0x409b9c)
- Command line: n = ?, a/b = sound mode (0x58), l = loop (0x1c), t (0x18), w = windowed (0x150), d (0x154), s (0x15), digit 0-3 = resolution (0x10: table at 0x410027 = 320x240, 512x384, 640x480, 800x600).
- main 0x401131: MMX/cpuid check ("MMX instruction set required!"), 0x4099bd, dialog, display init 0x40c4eb, 0x401800 (per-process init), 0x408fff (build the textures/objects, drives the "please wait" screen via 0x409a0c which also pumps messages and Esc),
  sound init 0x40b84d/0x40bee1, then the loop at 0x401252: key poll 0x4016cc (Esc 0x1b quits, space 0x20 toggles pause [ebp+0x54]), clock 0x40b80c, `t = clock*0x800/0x1234dc` (1193180 = PIT rate -> t is the demo time in 1/2048 s), then `0x409a50(t)` = render one frame
  (returns nonzero at the end); the loop repeats until the end (or forever with the 'l' flag).
- 0x409a50 walks a retained node tree: current node [ebp+0x50], each node = {+0 flag, +4 next, +8 first child, +0xc parent, +0x10 handler (called with edx = child),
  +0x14 inputs list}; tables at 0x42993c / 0x42994f / 0x4299c7 are the node-class tables (word offsets) filled by the init code 0x409a6b.. - i.e. the whole demo is a
  small interpreted scene graph: nodes are created from a compact script, evaluated per frame with the time. The effects are node classes (handlers) - map the handler table first,
  then port class by class (that is also the natural structure for h7core: a `Node` trait + the same tree built from the same script data).

## It is an operator-graph engine (werkkzeug-like), not hard-coded effects
0x407bdd..0x407d4e builds operator instances from a bytecode stream (edi): per operator an 8-byte descriptor in the class table at 0x429cd0 gives {kind, ninputs, 3 handler offsets (+base [ebp])},
instances are 0xa8 bytes (+0x0 kind, +0x4 n, +0xc/+0x10/+0x14 handlers, +0x18 input slot, +0x20 parent, +0x28 time accumulator, +0x48.. animation channel values, +0x40 stride 0x40 = per-op keyframe/state
blocks copied with `rep movsb`), a time-driven LCG (0x409bd7) seeds the random parts, and `0x409a50(t)` evaluates the graph for time t.  The textures ("please wait" phase) and the 3D/2D effects
are therefore *the same graph* evaluated once (textures) and per frame (effects).  Porting plan: (1) dump the operator class table 0x429cd0 + the bytecode, (2) port the class handlers
one by one in Rust (h7core::ops::*), (3) interpret the same bytecode (include_bytes! of the dumped script) - the output then follows from the data, no per-scene hand coding.

## Dump of the operator class table (H7DUMP=a h7emu.py 600000000 -> mem_a.bin = image + runtime state after init, ebp_a.bin = the context struct at [0x42a3f8] = 0x20001000)
Class table 0x429cd0 (8-byte entries {kind,n,h1,h2,h3}, handler offsets relative to [ebp+0]): only the first ~10 entries are real (offsets 0x16cd..0x1867 = tiny
handlers in the 0x401xxx-0x402xxx page, e.g. entry 1 = {1,1,0x1715,0x16cd}, 4 = {1,1,0x1867,0x1856}, 8 = {4,4,0x17c7,0x1723}); everything after index ~10 is other data - so the table is
NOT the list of effect classes. The effect/texture "ops" are driven through the node tree 0x409a50 / 0x407bdd instead. Next: trace the handler calls (call [esi+0x10] etc.) with an
indirect-call logger to get the real list of handler addresses used per frame, then group by address = the effect classes.

## Indirect-call log of the load phase (H7CALLS=a h7emu.py 400000000 -> calls_a.json, 88 (site,target) pairs; sites hooked via capstone)
Handler targets called from the operator evaluator (call sites 0x407cd7/0x407d50/0x407d54/0x407d5a/0x407fa0/0x409a35/0x409bc4):
- 0x4081c1 (451 calls), 0x40806f (284), 0x4081b0 (260), 0x408121 (65), 0x408040 (47), 0x408050/0x408051 (46/42), 0x40807d (17), 0x408027 (16), 0x408148 (2), 0x40828d, 0x40a320, 0x409c0f (downsample), 0x409ac3, 0x40ae69
  = the texture-op class handlers (the 0x408xxx page); 0x40695a (763) is just `ret` (empty handler = "no-op slot").
- 0x407fd0 = random-number op (called 128x by the generator opcode loop 0x407c77/0x407c81), 0x409a6b = op-script parser entry, 0x409b68/0x409b98 = node-list unlink / tree walk.
- inner pixel callbacks: 0x409cb6 (42 M calls: the crack/distance growth), 0x40d7af (327,680) and 0x40d811 (65,536) from the loop at 0x40d78f = per-pixel texture filters over a 256x256 (65,536) surface.
Port order: (1) the 0x408xxx handlers + 0x407bdd/0x407e05 evaluator, (2) 0x409c-0x409d filters, (3) 0x40d4-0x40dd per-pixel filters, then compare each result against the 8-bit/32-bit surfaces in emulator memory.
