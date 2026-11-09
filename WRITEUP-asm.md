# Heaven Seven - asm snippets worth reading

Disassembly of the unpacked `HEAVEN7W.EXE` (`re/image.bin`, base 0x400000; `objdump -D -b binary -mi386 -M intel --adjust-vma=0x400000`). Every snippet below was read from that listing and its behaviour checked against the Rust port (bit-exact, see `re/NOTES.md`).

## 1. 32-bit volume multiply from two 16-bit MMX multiplies (mixer, 0x40e6b4..0x40e6e6)
MMX has no 32x16 multiply. The mixer needs `sample * volume` accumulated in 32 bits, so it multiplies twice and glues the halves:
```
40e6be: punpcklwd mm0,mm0      ; duplicate the 16-bit samples
40e6c4: movq      mm1,mm0
40e6c7: pmullw    mm0,mm2      ; low 16 bits of sample*vol
40e6ca: pmulhw    mm1,mm2      ; high 16 bits (signed)
40e6d3: movq      mm2,mm0
40e6d6: punpcklwd mm0,mm1      ; interleave low/high = two 32-bit products
40e6dc: punpckhwd mm2,mm1
40e6df: paddd     mm0,[edi-8]  ; accumulate into the 32-bit mix buffer
40e6e6: paddd     mm0,mm2
```
`pmullw` + `pmulhw` + `punpck*wd` is the classic MMX widening multiply; the result is added straight into the 32-channel accumulator.

## 2. Volume ramps without branches (mixer, 0x40e6b1..0x40e6e9)
Per-channel volume ramps (the notes call them volume ramps; the exact semantics were not documented further). A value in `mm3` is stepped by `mm5` (`paddw mm3,mm5`) and clamped branch-free:
```
40e6d9: movq     mm6,mm3
40e6e3: pcmpgtw  mm6,mm7       ; mask = (ramp > 0)
40e6e9: pand     mm3,mm6       ; clamp to 0 when it ran out
```
A counter in memory (`dec [ebp+0x94]`) decides when the step is zeroed (`pxor mm5,mm5`).

## 3. Resampler phase in a carry chain (mixer, 0x40e676..0x40e686)
The 16.16 position of the sample read pointer is advanced with `add ebx,eax` / `adc esi,[ebp+0x7c]`: the fractional part adds into `ebx`, the carry propagates into the integer part `esi`. End-of-sample/loop handling is a `cmp esi,edx / jb` that is only taken off the hot path.

## 4. Idle channels cost almost nothing (mixer, 0x40e5c7..0x40e605)
The first pass over the channels (stride 0x100 bytes, count at `[ebp+0x1c0]`) tests `[ebp+0x94]` and the phase increment (`or edx,eax`): a channel with no increment and no ramp contributes one constant sample `movd mm1,[edx]` scaled once with `pmullw/pmulhw`, instead of running the resampler.

## 5. Clipping for free: `packssdw` (mixer output)
The 32-bit accumulators are shifted down and packed with `packssdw`, which saturates to int16. The peak of the demo reaches full scale (~140-150 s); there is no explicit clipping code anywhere.

## 6. The 3x3 blur as a rolling row-sum pipeline (font loader, 0x409578)
The bitmap font is blurred by a MMX routine that never forms a 3x3 window. Each pass computes horizontal 3-sums for a row into a scratch strip (`punpcklbw` against zero, `paddw` of x-1, x, x+1: 0x4095a6..0x4095d5) and a later pass adds three row-sums (above, middle, below) and scales by a *reciprocal*:
```
4095fc: lea  ebx,[eax+9]       ; kernel weight sum = w+9
4095ff: mov  eax,0x10000
409604: xor  edx,edx
409606: div  ebx               ; 65536/(w+9), once per glyph
40960b: punpcklwd mm7,mm7      ; broadcast to 4 lanes
        ; mm7 then multiplies the sums (pmulhw) = division by (w+9)
```
One `div` per glyph, then only multiplies (`pmulhw` truncates, so e.g. 127*4681>>16 = 9, not 9.x). Quirks reproduced bit-exactly in `re/blur_mmx.py`: the first chunk doubles the centre pixel instead of using a left neighbour, and the right-neighbour offset is 4 instead of 5 on the last chunk.

## 7. A signed compare that disables a clamp (crack growth, 0x409c7c)
Every non-zero texel becomes `(23*d)&0xff`, d = distance to the nearest zero cell, found by a recursive depth-limited search (bound 256). The depth clamp is `cmp al,0x7f; jng`: a *signed* byte compare, so it never triggers for depths up to 255. The "bug" is part of the look; the port is a multi-source BFS that gives the same numbers. The search is exponential on large non-zero areas (10^8+ instructions) - fine for a 64k intro that generates its textures during "please wait".

## 8. A table-driven image VM (0x40d43f)
Textures are not code but *bytecode*: the routine clears a 1.25 MB buffer, then loops `byte -> lookup in a {key, handler} table (0x42a29c, stride 8) -> call handler`. Handlers select a 256x256 plane (`al<<18`), run blend modes (0x40d7b0..), HSV conversions, or a tiny hash (`ecx*0xfacedead, rcr 3, xchg cl,ch`). The final step converts 0x10000 dwords with `lodsd; bswap; shr 8`. This is the whole texture generator of a 64 KB executable.

## 9. Clock: PIT ticks to demo time (main loop, 0x401252)
`t = clock*0x800/0x1234dc`: the clock is scaled by 1193180 (the PIT rate) into 1/2048 s units, so no division by 1000 and no float. With sound on, the clock is the DirectSound play cursor, which keeps picture and music in lock step.
