#!/usr/bin/env python3
"""font_check.py - the font loader model (font_model.parse + blur_mmx.blur) vs the emulator's glyph surfaces (glyph_truth.json): expects 30/30 bit-exact."""
import json
from font_model import parse
from blur_mmx import blur
T=json.load(open('glyph_truth.json'));A,gl=parse();ok=0
for c,(D,bits) in gl.items():
    W=D+4;H=8*A;S=(W+7)&~7;img=bytearray(S*H+64)
    for x in range(D):
        for by in range(A):
            byte=bits[x*A+by]
            for bit in range(8):
                if not (byte>>bit)&1:img[(by*8+bit+1)*S+x+2]=0x7f
    out=blur(img,W,H,S,5);ok+=bytes(out[:S*H])==bytes.fromhex(T[str(c)][3])
print('bit-exact',ok,'of',len(gl))
