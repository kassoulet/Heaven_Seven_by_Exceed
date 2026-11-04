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

## Ported + verified: the packed-data readers (h7core/src/pack.rs, test = 600 vectors from the original in re/pack_vectors.txt, bit-exact)
- 0x407fc0 `read_int`: byte b: b&1==0 -> b>>1; else 16-bit word w (signed) >> 1.
- 0x407fd0 `read_float`: engine-private float formats - 16-bit (bit0 flag, 5-bit exp field via `(w>>10)+0xe0`, 9-bit mantissa, sign = bit 15 *via the carry into ah that
  shl 23 turns into the sign bit*), 24-bit (3 bytes; 17-bit mantissa; sign = bit 23), or a single 0 byte for 0.0. A neat 64k trick: no sign byte, no separate exponent byte, the
  and al,0x1f / add al,0x7a pair re-biases the exponent into the useful 2^-5..2^20 range, and the carry gives the sign for free.
(An earlier draft assumed the dword form took 4 bytes and had no sign - the emulator vectors caught both mistakes.)

## Ported + verified: simple operator handlers (h7core/src/ops.rs; 1400 differential vectors from the original in re/handler_vectors.txt, generated by re/handler_vectors.py)
0x408040 cmp40 (interval < 64 ticks flag: `shr eax,7; sbb [ebx],0` turns bit 6 into the result - a 2-instruction compare), 0x408051 f2i51 / 0x408064 f2b64 (float -> int/byte, x87 round-to-nearest-even,
out of range -> 0x80000000), 0x408057 byte2f57, 0x40806f int2f6f (stream int -> float value), 0x40807d quad4f7d (four floats -> four words clamped 0..255 = a colour),
0x4081b0 copy1b0 (value slots -> out), 0x4081c1 fill1c1 (n packed floats from the stream into the value slots, *last slot first* - `loop` counts down).
Not yet ported (need the operator layout first): 0x408027 (texture index -> pointer), 0x4080fd/0x408121 (quad reader), 0x408148/0x408185 (matrix ops via 0x40d256/0x40d2c8), 0x408211 (aspect/viewport setup), the evaluator 0x407bdd.

## FPU mode and the animation system (read from 0x4099bd, 0x407bc5-0x407f99)
- The CRT start (0x4099bd) does `finit; fldcw 0x007f`: **24-bit (single) precision**, round-to-nearest, all exceptions masked. All x87 code therefore behaves like plain f32 arithmetic (each
  op rounded to 24 bits, extended exponent range): the Rust port can use f32 directly (check x87 stack-order/fxch sequences op by op; Unicorn/QEMU honours the precision control, so the
  differential vectors stay valid).
- 0x407e05 = animation-channel evaluator `eval(channel edx, time ebx)`: keys are 0x40-byte blocks (+0 time, +0x24.. values, +0x60.. next block), the key list wraps (`div` of time by the length
  [edx+0x1c]), segment search loop 0x407e34 (call [edx+0x14] = key-block allocator/reader, 0x407bc5 = copy a 0x40-byte key into the pool at [ebp+0x34]), tangent builder 0x407d84
  (three neighbouring keys, tension/continuity handling with `jcxz` flags, Hermite-style blend with weights 0.5 and 3.0 constants, writes 4 floats per component at [edx+0x68..]), the
  final cubic evaluation at 0x407f78 (`fld [eax+0xc]; fmul t; fadd [eax+8]` ... Horner form) and the interpolation parameter shaping 0x407ee0-0x407f68 (smooth-step style easing).
  This is the engine's spline/timeline system: every moving parameter in the demo (camera, object transforms, effect parameters) is a channel evaluated by it.
Port plan: Rust `Channel` with the same 0x40-byte key layout, ported op by op, verified against differential vectors (random key lists + times, results compared bit-exact).

## Ported + verified: spline tangent builder 0x407d84 (h7core/src/anim.rs, 400 vectors re/tangent_vectors.txt; bit-exact on the first try = confirms f32-only arithmetic under fldcw 0x7f)
Kochanek-Bartels tangent at key `cur` from keys cur-0x40, cur, cur+0x40 (key block: +0 time i32, +4.. values 1-based, +0x28 tension, +0x2c bias, +0x30 continuity):
  r = dt/(dtp+dt) (flag!=0: outgoing, bias negated) or dtp/(dt+dtp) (flag==0), X = (r-0.5)(1-|bias|)+0.5  (time-ratio weighting of the two one-sided differences),
  out = ((cur-prev)(1-bias)(1+cont) - (next-cur)(1+bias)(cont-1)) * (1-tension) * X.
Remaining pieces of 0x407e05: segment search + the lazy key reader (call [edx+0x14]), the Hermite coefficient block at [edx+0x68..] (0x407e54-0x407eb5) and the easing + Horner evaluation (0x407ec1-0x407f95).

## Ported + verified: the whole animation channel evaluator 0x407e05 (h7core/src/anim.rs `Channel::eval`; 500 vectors re/eval_vectors.txt incl. result values, segment pointer and the cached coefficient block; bit-exact)
- time handling: t<=0 -> 1, t >= length -> t mod length (looping channels); keep the current segment while key[cur].t < t < key[cur+1].t, restart from the first key when time goes backwards.
- per segment (computed once, cached): cubic Hermite coefficients per component c0=P0, c1=T0, c2=3D-2T0-T1, c3=T0+T1-2D with D=P1-P0 and T0/T1 from the KB tangent builder (outgoing at the key, incoming at the next).
- parameter shaping: s=(t-t0)/(t1-t0); with ease-out a=[key+0x24] and ease-in b=[nextkey+0x20]: a three-piece curve (quadratic ease-in over [0,a], linear, quadratic ease-out over [1-b,1]) built with
  q = 1/(2-(a+b)) (or the rescaled form when a+b>1): y = q/a s^2 | q(2s-a) | 1-q(1-s)^2/b  - i.e. constant-speed with parabolic ramps, exactly "slow-in/slow-out" with adjustable ramps. If a+b==0: y=s.
- the cubic is evaluated per component in Horner form with y.
The x87 transcription used a 12-line register-stack helper (struct X) so every instruction maps to one line; this is the pattern for the remaining x87 handlers.

## The class table 0x429cd0 resolved: it is the PARAMETER-TYPE table (entries 0..10 only; the rest of the 8-byte stride is unrelated data)
[ebp+0] = 0x40695a (the address of a bare `ret`) is the base the 16-bit handler offsets are added to (so "0" = empty handler = 0x40695a).
Entry = {kind, n values, reader/decoder h1, evaluator h2, extra h3}:
 1 {1,1} texture reference: read 0x40806f (int), eval 0x408027 (index -> texture pointer ebp+0x1e0+idx*0x200)      2 {0,0} time test: eval 0x408040 (interval < 64 ticks)
 3 {1,1} integer: 0x40806f / 0x408051 (float->int)                                                              4,5,6 {1,2,3 floats}: reader 0x4081c1 (packed floats), eval 0x4081b0 (copy)
 7 {4,1} matrix/rotation: reader 0x4081c1, eval 0x408185, extra 0x408148 (both use the 3x3 matrix helpers 0x40d256/0x40d2c8)   8 {4,4} colour: 0x408121 (quad), 0x40807d (clamped words)
 9 {1,1} byte: 0x408057 / 0x408064                                                                              10 {4,0}: 0x416061/0x416472/0x406a72 (not yet looked at)
All of 1-9 except 7 are ported and verified (ops.rs). The OPERATORS themselves (texture ops, 3D, effects) are defined by the 5-byte records of the tables at 0x42993c/0x42994f/0x4299c7 walked by 0x409a6b..0x409b36 -
next: decode that record format (and dump the 0x429xxx tables after init), then write a Python disassembler for the operator script.

## The operator CLASS hierarchy (re/classes.py -> re/classes.txt; from the word table 0x42994f -> records 0x42993c+off)
Record: +0 class id, +1 parent (single inheritance, root = itself), +2 flags, +3 handler (word + 0x40695a), +5 word, +7.. six (param type, slot) pairs.  0x409a6b instantiates by walking the parent chain
to the root and running the handlers root-first, creating each level's declared parameters (types of the 0x429cd0 table). Example chains:
 root 0 {time}  -> 2 -> 3 (handler 0x408211 viewport/aspect) -> 17 (0x4089cd)         root 31 {f1,f2} -> 47 -> 48 -> 5 {int,color,tex} / 12 {color} / 15 {int};  31 -> 8, 13, 14, 46 (0x40b213), 55
 root 18 (0x408bc1, {f3 position, matrix, int} = the 3D/transform base) -> 19 (0x40b6ca {tex,color,color,f1,int}) -> 20 -> 24/25/33/42/43, 29 -> 26 -> 28/44, 32 -> 45, 34 -> 58, 35 -> 37, 54;
 18 -> 22 (0x4083e1), 23 (0x408b4f), 51 (0x409729 {f1,color,tex}) -> 41 (0x40b3b1), 53 (0x40973e), 52 (0x40b734).
The handlers of the leaf/mid classes (0x4083e1, 0x408b4f, 0x40b3b1, 0x40b6ca, 0x409729, 0x40973e, 0x40b734, 0x40b213, 0x4089cd, 0x408bc1) are the actual effect/texture/mesh operators.
Next: find the stream start of the operator script (0x408fff builder) and write the script disassembler (class id byte, then the params read by the type readers).

## The build script and its opcode dispatcher (0x409a0c loop, stream at 0x419029, handler table words at 0x4299c7 + [ebp+0])
main calls `0x409a0c` with edi = 0x419029: it pumps messages (Esc aborts) and reads one opcode byte per iteration (0 = end), then `call [0x4299c7 + 2*opcode] + 0x40695a`. 0x408fff (called just before) is NOT the
script builder: it precomputes a (dx,dy)->unit-direction lookup (fsqrt/fdivr, 0x7f clamps) for a screen-sized table (bump/light map).
Opcode -> handler: 1 0x409a6b (create an instance of class <next byte>, resets the LCG seed to 0x3039 per op, allocates a 0x200-byte object slot), 2 0x409b68 (pop), 3 0x409bfb (rand), 4 0x408e82, 5 0x40832b,
6 0x4082b9, 7 0x408279, 8..16 0x409ac3 (parameter set, `al-7` = parameter index), 17 0x409574, 18 0x409bcc (seed), 19 0x406a3b, 20 0x40828d (8-bit 256x256 from the 32-bit surface),
23 0x409c0f (2x2 downsample), 24 0x409c7c (crack growth), 25 0x409ffc, 26 0x40a3c3, 27 0x40a320, 28 0x40a468, 29 0x409a3f, 30 0x40a42a, 31 0x40867a, 32 0x4069c6, 33/34 0x406a5a, 35 0x406ab4, 36 0x406db5, 37 0x4069c4 ... .
The stream starts `01 04 02 00 02 00 02 cd 29 02 00 01 00 02 02 01 11 00 04 0c 40 02 20 ff ff ...` (create class 4, ...).  Next: disassemble each opcode handler's argument reading to write the script disassembler.

## Script trace (H7SCRIPT=1 h7emu.py 800000000 -> re/script_trace.json: per dispatched opcode {stream start, handler, stream end}; 864 ops in the first 800 M instructions, stream 0x419029..0x428ca3+)
Op histogram: 1 create-instance x282, 2 pop x280 (a tree: instances are created, parameters set with ops 8-16, then popped), 8-16 parameter sets x274 (op 11 x161, 10 x73, 15 x25, 8 x15), 3 rand x2, 4 x1 (0x408e82: runs the
long ff..ff stream = a big table/animation block), 5,6,7 (0x40832b, 0x4082b9 x2, 0x408279 x3: viewport/surface setup), 17 x2, 19, 20 x3 (8-bit conversion), 23 x3 (downsample), 24 (crack growth) x1, 25-30 once or 4x (filters).
The bulk of the stream is the *scene description*: class 0x2e (=46, handler 0x40b213, params {f1,f1,int}) instances with animation key data (the words/captions: the strings at 0x41a3f4.. are their text), class 0x0e/0x0d/0x05/0x32/0x12
etc. for the 3D/2D elements. So 7 KB of script + the keys = the whole demo structure; the texture generation is only a few ops (23-30) with 256x256 surfaces.
Next: decode each handler's arguments (use the trace: bytes consumed per op/class) and write re/script_dis.py.

## Correction (class 46)
Earlier I called class 46 "the captions". Its handler 0x40b213 is only: read a zero-terminated list of bytes from the stream; entry i -> pointer [esi+0x44+4i] = [ebp+0x4c] + byte*0x600 (a table of 0x600-byte records
indexed by the byte): class 46 = "list of references to 0x600-byte records" (glyph/shape/layer slots - to be identified from the producer of [ebp+0x4c]). The text-like strings at 0x41a3f4+ are plain data
read elsewhere. Nearby helpers: 0x40b231 (walk children, call [esi+0x30]), 0x40b251/0x40b27b (allocate a w x h surface: pitch = (w+7)&~7 * 4, +4 bytes guard, via 0x409989 = the demo's bump allocator),
0x40b2a8 (cached midpoint vertex with a random jitter, `0x409bed` rnd*0.015625: a subdivision/"midpoint displacement" helper) and 0x40b341 (its recursive triangle subdivision, ecx = depth) -> a procedural mesh/terrain generator exists.

## What builds the 0x600-byte records: opcode 4 (0x408e82) = the BITMAP FONT loader (read from the code; not yet checked against emulator memory)
`[ebp+0x4c] = [ebp+0x44]` (current pool pointer) then, per glyph group: two packed ints (count/size), `imul 0x600` and the pool pointer advances by that much per record; for each record a surface is allocated (0x40b27b: w,h
from the stream), and the stream's **1-bit-per-pixel bitmap** is expanded (`test [edi],ah; ... mov dword [edx],0x7f` for the clear bits, `add ah,ah` walking the 8 bits of each byte) into an 8-bit surface with value 0x7f,
then the glyph is smoothed twice with the 2x2 downsample/box filter 0x409c21 (anti-aliased glyphs). The 5000-byte `ff..ff` run at 0x41903c is that bitmap data (mostly empty / solid runs after the font's own packing).
=> class 46 (a zero-terminated list of bytes -> glyph record pointers) is therefore a *string*: the text of the intro is a list of glyph indices into this font (so my first reading, "captions", was right, my
"correction" two entries up was too cautious: the 0x600-byte records are glyph surfaces). To verify: dump [ebp+0x4c] after op 4 and render a glyph.

## VERIFIED: the font (H7GLYPH=1 h7emu.py 400000000 -> re/glyphs.png)
[ebp+0x4c] = 0x200015e0 (emulated heap) after opcode 4; records at base + i*0x600 hold a surface (+0x18 w, +0x1c h, +0x20 pitch*4, +0x28 data pointer, 8-bit pixels, stride pitch>>2).
Rendering the first records gives an anti-aliased Helvetica-like lower-case font: `- . 3 ? a b c d e f g h i j k l m n o p q r s t u v w` (re/glyphs.png) - so class 46 strings = lists of glyph indices into
this table, exactly the story text ("we used to dream", ...). 291 records are reserved in the pool at that point (only 28 filled by 400 M instructions; the rest are produced by the next font groups).

## Font loader decoded (re/font_model.py): stream = A=6 (bytes per column, 8A=48 rows), then groups {B = record skip = ASCII code delta, C = glyph count, C x {D width, A*D column bytes}} until B<0 (30 glyphs: space - . 3 ? a-y...).
Glyph = surface (D+4) x 48, ink (bit CLEAR, bit0 = top) = 0x7f at (x+2, y+1), then op 17 with parameter 5 (0x409578: 3x3 blur, centre weight 5, pmulhw by 65536/14) - 20/29 glyphs bit-exact vs the emulator, the rest differ in one stem column
(in-place MMX pipeline detail). Two 2x2 downsamples of the *unblurred* surface are stored in the next two 0x200 objects of the 0x600 record (mip levels for smaller text). Class 46 strings are ASCII codes (record = base + code*0x600).

## Font loader: 30/30 glyphs bit-exact (re/blur_mmx.py = literal instruction-by-instruction transcription of op 17 / 0x409578 incl. its quirks; re/font_check.py)
Quirks that the "plain 3x3 blur" model misses, all reproduced by the transcription: the scratch pipeline holds three row-sums (above, middle, below) shifted every row (the third loop 0x4096e1);
the first pass seeds row -1 and row 0 from row 0 only; first chunk of the first pass doubles the centre pixel instead of using the left neighbour; the left neighbour of pixel 0 of a row is the pixel itself (eax=0 on
the first chunk, -1 afterwards); the right neighbour offset becomes 4 instead of 5 on the last 8-pixel chunk (esi); pmulhw by 65536/(w+9) truncates (127*4681>>16 = 9, 126 for a full kernel).
Rust port: transcribe the same structure with u64 helper ops (paddw/pmullw/pmulhw/packsswb/punpck*) - the Python file is the 1:1 reference.

## Ported + verified in Rust: font loader + MMX blur (h7core/src/font.rs; test = re/glyph_truth.txt, 30/30 glyphs bit-exact, data = re/font_stream.bin cut from the original at 0x41903c..0x41a3c9)
`font::load(stream) -> Vec<Glyph>` (code = ASCII, w, h=48, stride, 8-bit data), `font::blur` = literal u64 transcription of 0x409578 (MMX lane helpers paddw/pmullw/pmulhw/packsswb/unpck*).

## The 64 text elements (class 46 / 0x2e instances; re/captions.txt = stream offset + text; strings are plain NUL-terminated ASCII inside the instance record, preceded by position/animation parameters and keys)
Story, in script order: please wait / asking about places... / exist / like: / places like / used to / we used to dream... / heaven / heaven seven / being born / (growing) / (living) / rising / (shouting) / (collapsing) / without / meaning /
the newborn. / died right after / he was born / never had the chance / to make a choice of / his belief / the elderly. / going through irreversible / changes / losing his memories / losing himself... / will he ever regain / his lost abilities? /
what about people / who never had those / abilities / just questions... / ...dreams / about a place / far away; then the credits (.picard .warpig .shaman .stephen) and the greetings (3state aardbei blasphemy astroidea calodox chrysalis elitegroup
haujobb amable exceed. ... mekka-symposium ... kolor pathos mandula ramjam replay sunflower tpolm kooma). (A few strings with non-lowercase glyph codes print as None in captions.txt - to be decoded with the full record layout.)

## A text instance in memory (H7OBJ=1 h7emu.py, "please wait" = stream 0x41a3db; dump of the 0x200-byte object after op 1)
+0x00 flags 0x0101 (bit0/bit8), +0x0c parent pointer, +0x14 -> parameter node (its handlers: +0x0c 0x4081c1 fill, +0x10 0x4081b0 copy, +0x14 ret; +0x20/+0x24 key list pointers, +0x18 value buffer),
+0x28 render handler **0x409cda** (draws the string: per glyph `fild [glyph+0x18]` * scale [+0x34] * [parent+0x2c] -> positions, x87 + `fist`), +0x2c float 140.0 (0x430c0000), +0x3c 0xff (alpha),
+0x44.. the glyph record pointers of the string, zero terminated (one per character; repeated letters share the pointer: 'e' of "please" = same pointer), glyph records show w=0x25.. h=0x30=48 pitch=..,
so rendering a caption = 0x409cda walking those pointers with a per-glyph advance and the animated parameters. Next: disassemble 0x409cda-0x409deb (seen above: per-glyph loop with `call 0x40c8d0` = the glyph blitter chosen by size: ecx += 0x200 per mip level when the glyph is small -> uses the +0x200/+0x400 downsampled copies).

## The glyph blitter 0x40c8d0 (called from the text renderer 0x409cda per glyph, ecx = glyph record or its mip copies +0x200/+0x400)
Entry: eax -> {x, y} fixed point (16.16) of the destination, esi -> {x, y, w, h} = the sprite rectangle in 16.16 (w,h at +8/+0xc; bails out when w or h <= 0x8000 = half a pixel), ecx = source surface (+0x18 w, +0x1c h, +0x20 pitch*4, +0x28 data),
edi = destination (frame buffer line table), tint colour words in the stack frame (mm4 = colour << 2 broadcast). It computes the source step (`(src_w << 27) / (rect_w << 5)` = 16.16 texel increment per pixel via idiv),
clips the rectangle to the destination and source (the `and 0xffff / sub / imul / shrd eax,edx,16` blocks = sub-pixel start offsets), then runs an MMX bilinear-filtered, tinted, alpha-weighted span loop (constants at 0x42a1e0 / 0x42a1f0, from 0x40ca29).
Port plan: literal transcription (like blur_mmx.py) verified with differential vectors from the emulator: random source glyph + rectangle + tint -> compare the destination words.

## Three sprite blitters share the entry convention (0x40c8d0, 0x40cace, 0x40ccc5)
- 0x40c8d0: **bilinear**, tinted, per pixel (mm4 = tint<<2, tables 0x42a1e0/0x42a1f0/0x42a1f8), reads the 8-bit glyph with two texel pairs (dl/dh from [eax+ebp] and the previous row), `pmulhw`s the 4 bilinear weights, `pmaddwd` with the tint, adds to the destination byte and stores 8 bits ([edi]): the destination is an **8-bit intensity layer**.
- 0x40cace: **nearest**, 4 pixels per iteration, `paddsb` (saturating add) of the glyph (mm1 = texel * mm4 >> 7) into the destination bytes - the fast path for large/unscaled text.
- 0x40ccc5: **rotated** sprite (fsincos of the angle at [esp], rotated step vectors, per-row loop) for the spinning/tilted captions.
Verification plan for each: differential vectors (random surface, rect, tint -> destination bytes) from the emulator, then a transcription in the style of blur_mmx.py.

## Ported + verified in Rust: the bilinear glyph blitter 0x40c8d0 (h7core/src/blit.rs + mmx.rs; 300 differential vectors re/blit_vectors.txt from re/blit_vectors.py, bit-exact, first run)
Semantics (all from the literal port): `blit_bilinear(tint_param, rect[x,y,w,h in 16.16], src surface (8-bit, pitch), dst 8-bit layer)`; the sprite covers (x-w..x, y-h..y) (anchored bottom-right);
source step = (srcw<<27)/w per pixel in 1/4096-texel units (vertical likewise); positions/clips in 16.16 with the `ceil` idiom `(v+0xffff)>>16`; the bilinear weights are the 15-bit fractions (f, 1-f) x (g, 1-g) built with pand 0x7fff / pxor
(pxor with 0x7fff gives 1-f exactly), the four texels (above row = src row - 1, hence the 1-row offset) are expanded to 16-bit (b*257), pmulhw'd by the weights, pmaddwd'd with the tint lanes (tint = high word of the parameter << 2),
summed with the destination byte (<<16) and shifted back (psrad 16), packed with signed saturation to a byte: out = clamp_i8(dst + sum(weights*texels*tint)>>16).
The MMX helper set moved to mmx.rs (paddw, pmullw, pmulhw, pmaddwd, psllw, punpck*, psrad, paddd, packsswb).

## Ported + verified in Rust: the text renderer 0x409cda (h7core/src/text.rs; test = a real capture of "please wait" from the emulator, re/text_capture.txt: object, layer, glyph mips, before/after layer; **0 differing pixels of 56,320**)
Layout (all f32, x87 order): S = sum_glyphs (w*scale*v2c) + (n-1)*spacing*v2c; start x = round((x0+65536)*160 - S); h = round(h0*scale*v2c*1.08), y = round((-y0 + 65536*H/W)*160 - h0*scale*v2c*1.08*0.535) (so the text is centred
on the layer centre); per glyph: a = w*scale*v2c, x += a (right edge), rect = {x, y, 1.08*a, h}, mip level from the sprite height in pixels (<15 px: half-size copy, <5 px: quarter), blit_bilinear with tint = alpha*4; then x += a + spacing*160.
Constants: 1.08 (overdraw to cover the blur margin) = 0x429eda, 0.535 = 0x429ed6. Object flag bit 0 = "draw even when the 't' command-line switch is set". Viewport object (class 3 / 0x408211): +0x2c = w/4, +0x30 = w/2, +0x34 = 65536.0, +0x38 = 65536*h/w.
(My first attempt drew nothing: I had inverted the flag test - the capture caught it.)

## First complete frame reproduced from Rust: frame 0 ("please wait") = text layer + composite (text.rs compose_layer; test vs re/frame0.rgb, the emulator's presented frame, all 320x240x3 bytes equal)
The 8-bit layer (values 0..126, i.e. 7-bit intensity) becomes RGB with r = g = (v*254)>>7, b = (v*255)>>7 (a near-white tint; derived from the 124 distinct layer values of the capture, unique and exact). The picture occupies the TOP 320x176 of the
320x240 frame (rows 0..175; the 64 bottom rows are black) - not centred as the h7core placeholder assumed.

## Ported + verified in Rust: crack growth (opcode 24 / 0x409c7c, h7core/src/texops.rs; 80 differential vectors re/texop_vectors.txt from re/texop_vectors.py)
Every non-zero texel -> (23*d)&0xff, d = distance through non-zero *memory cells* (+-1, +-pitch in linear memory, so with pitch == width the right edge sees the next row; the zero guard rows end the search) to the nearest zero cell
(the original's recursive depth-limited search, bound 256; `cmp al,0x7f; jng` is a signed compare so the clamp never triggers). Ported as a multi-source BFS. Gotcha found by the vectors: the original can take 10^8+ instructions on large
non-zero regions (exponential re-visiting) - the first vector set was silently truncated by my instruction limit and looked like "wrong output"; detect `EIP != return address` in the harness.

## Ported + verified in Rust: 2x2 downsample (opcode 23, 0x409c0f/0x409c21; texops.rs `down2`; 80 vectors incl. full byte range)
out = (b0 + b1 + b2 + b3 + 2) >> 2 where the adds are the original's 8-bit `add al` / `adc ah,0` chain starting from a sign-extended first texel (movsx) - exact also for texels >= 0x80. Output size (w/2, h/2).

## Ported + verified in Rust: texture ops 26 (multiply by a tiled 256x256 texture, 0x40a3c3) and 30 (copy, 0x40a42a) (texops.rs `mul_tiled`, `copy_tiled`; 40 vectors)
op 26: d = min(127, (d*s)>>8), the source read through a 16-bit (y<<8|x) wrap = a 256x256 tile, starting at row 128; op 30: plain tile copy from row 0. Source = the object selected by a stream index byte (0x407fad: ebp+0x1e0+idx*0x200, data pointer +0x28).
Other ops of the family seen: op 25 (0x409ffc) = a 2-pass +/- tap difference filter (emboss/edge: sums of 5 taps with `add/adc` minus 5 taps, 16-bit lanes) producing a 32-bit result surface; op 27 (0x40a320) = resize of a surface with the bilinear blitter (size params in 320-based units);
op 28 (0x40a468) = a lighting/shading blend (mm5 ramp from 0x42a002, xor mask 0x42a00a, paddsb into the destination); op 29 (0x409a3f) = reset the object stack pointer; op 20 (0x40828d) = 32-bit -> 8-bit conversion (`shr al,1`).

## Ported + verified in Rust: op 28 (0x40a468) shaded blend (texops.rs `light_tiled`; 20 vectors, bit-exact first run)
Per row the multiplier edx = 3, +4 per row up to 0xa3 while > 40 rows remain, then -4 per row (a triangular vertical profile); mm5 = edx*[1,0,2,3] (table 0x42a002), stepped by 4*edx per 4 pixels;
out = paddsb(d, packsswb(((src + (r>>12)) * ((d^0x7f) + (r>>9))) *hi r >> 7)). Constants: 0x42a002 words {1,0,2,3}, 0x42a00a = 7f x8. New MMX helpers: psrlw, packsswb (two sources), paddsb.

## Correction: the blitter rect is CENTRE + HALF-EXTENT (not bottom-right anchored)
0x40c8d0 clips with left = x - w, right = x + w, top = y - h, bottom = y + h (all 16.16): {x,y} is the sprite centre, {w,h} the half-size in 16.16 (the earlier note and the doc comment in blit.rs said "anchored bottom-right": wrong wording, the port itself is
a literal transcription and bit-exact, so only the description was off). The resize op 27 (0x40a320) confirms it: it calls the blitter with rect {W<<15, H<<15, W<<15, H<<15} = centre (W/2, H/2) px, half-size (W/2, H/2) px.

## Op 20 (0x40828d) hides a bigger piece: 0x40d43f = a table-driven image program
0x40d43f clears a 0x50000-dword buffer ([0x42a210], 1.25 MB), then loops `byte -> lookup in the (key, handler) table at 0x42a29c (stride 8) -> call handler` (handlers e.g. 0x40d48a = select a 0x40000-byte plane `al<<18`, 0x40d498 = an LCG-ish
mixer `ecx*0xfacedead, rcr 3, xchg cl,ch`), and finally converts 0x10000 dwords (`lodsd; bswap; shr 8`) to the 256x256 result. This is the procedural generator of the cave rock (and the other 256x256 textures); op 25 (0x409ffc)
is the emboss/edge pass, 0x40a0e7.. a normal/lighting evaluator (fld/fmul/fsqrt/fdivrp = vector normalise). Porting these = the next big block; verify with whole-surface dumps from the emulator at the end of the texture ops.

## The image-program interpreter 0x40d43f: handler table (0x42a29c, {code, handler}, stride 8; mem_a.bin)
Called by op 20 with eax = output (256x256x4 at ebp+0x2001e0) and edi = the script stream: it zeroes the 5-plane work area ([0x42a210], plane n = base + (n<<18)), then interprets the stream (`lodsb` = code -> table -> `call handler`, esi
passed in [esp+4]); handlers return carry clear to continue; codes 1-4 (0x40d48a) = "result is plane n": esi = plane, stc -> the loop ends and the plane is copied out (`lodsd; bswap eax; shr eax,8` = ARGB -> 0RGB, 65,536 dwords).
Codes -> handlers: 1,2,3,4 0x40d48a | 0x10 0x40d8c0 | 0x11 0x40d8f7 | 0x12 0x40da07 | 0x13 0x40daac | 0x14 0x40db51 | 0x15 0x40dbf0 | 0x20 0x40dc7b | 0x21 0x40dcba | 0x24 0x40dda7 | 0x31 0x40de29 | 0x32 0x40de6b |
0x33 0x40dece | 0x35 0x40df1d | 0x36 0x40df9a | 0x37 0x40e04a | 0x40 0x40e0a7 | 0x41 0x40e198 | 0x50 0x40e224 | 0x51 0x40e2a3 (the 0xffffffff entries are padding). Helper 0x40d498 = an LCG-style generator (ecx*0xfacedead, rcr 3, xchg cl,ch).
That is ~22 generator ops in ~8 KB of mostly MMX/x87 code: each to be ported and verified with differential vectors (random planes + stream args), exactly as the 0x408xxx handlers and texture ops were.

## Helper library under the image program (0x40d4b1..0x40d8c0, read from the listing; not ported yet)
0x40d4b1 bilinear sampler of an ARGB plane (esi plane, bl/bh = x/y step offsets from al/ah, ecx/edx = 8-bit fractions: four taps, psllw 4, psubw/pmulhw lerp in x then y, packuswb; wrapped in fsave/frstor);
0x40d536 RGB -> HSV (min/max with the sbb/and branch-free idiom, hue = ((d*0xff)/range + sector*0x200/0x400) * 0x2aaa >> 16), 0x40d604 HSV -> RGB (table-driven sextant selection: tables 0x42a23c.. 0x42a284, 6 sectors);
0x40d684.. per-pixel signed clamp/blend with pcmpgtw masks. These are the building blocks of the generator handlers 0x40d8c0..0x40e2a3 (a Photoshop-style filter set on ARGB planes).

## Ported + verified in Rust: the bilinear ARGB sampler 0x40d4b1 (h7core/src/imgops.rs `sample`; 300 vectors re/helper_vectors.txt, plane = re/plane.bin; bit-exact first run)
Four taps (x,y), (x+dx,y), (x+dx,y+dy), (x,y+dy) on a 256x256 dword plane with 8-bit wrap in x and y, widened to 16-bit lanes <<4, lerp in x with the 15-bit fraction (pmulhw), then in y, packuswb. New MMX helpers: psubw, psraw, packuswb_lo.

## Ported + verified in Rust: RGB<->HSV (0x40d536, 0x40d604; imgops.rs `rgb2hsv`, `hsv2rgb`; 400 vectors each incl. grey/black/white edge cases, bit-exact)
rgb2hsv: V = max, S = 255*(max-min)/max, hue numerator from the two differences by which channel is the max (offset 0, 0x200, 0x400), scaled (*255 idiv range) then `imul bx, 0x2aaa` and the high byte (a 6-sector hue in 0..255);
hsv2rgb: sector = high dword of H*0x06060606, f = low dword>>24, p/q/t as usual, channel placement via four 6-entry multiplier tables at 0x42a23c..0x42a284.

## Ported + verified in Rust: the blend-mode library (0x40d7b0..0x40d8bf; imgops.rs `blend_*`; 1050 vectors, bit-exact first run)
add (paddusb), subtract (psubusb b-a), difference (|a-b|, via a signed compare after a -0x80 bias), multiply-with-inverse ((255-a)*b>>8 on RGB), multiply (a*b>>8), screen (~(~a*~b>>8)), overlay
(2a split into low byte and an "a>=128" 0xff mask that conditionally inverts both operands). 0x40d758 selects two of them by the nibbles of a stream byte (tables 0x42a374 = channel masks, 0x42a394 = blend function pointers), 0x40d77a applies the chosen blend with that mask over a
whole 65,536-pixel plane: out = (blend(src,dst) & mask) | (dst & ~mask). 0x40d684 = per-channel auto-levels (min/max scan with pcmpgtw masks, 0x7fff/range reciprocals, `(v - min) * recip >> 7`).
