#!/usr/bin/env python3
"""font_model.py - Python model of the font loader (opcode 4, 0x408e82) checked against the emulator's glyph surfaces (glyph_truth.json from H7GLYPH2=1 h7emu.py 800000000).
Stream at 0x41903c: A (bytes per column, =6) then groups {B = skip (record index advance = ASCII code delta), C = glyph count, C x {D width, A*D bitmap bytes}} until B<0.
Glyph surface: w=D+4, h=8A=48, stride=(w+7)&~7; ink (bit CLEAR in the column bytes, bit0 = top) = 0x7f at (x+2, y+1). Then op 17 (0x409578, param 5) = 3x3 blur with centre weight 5:
out = ((sum3x3 + 5*centre) * (65536//14)) >> 16  (pmulhw).  RESULT: 20 of 29 glyphs bit-exact; 9 differ in 4-14 pixels along one stem column (x=2..3): the MMX loop's in-place
8-pixel pipeline (left-neighbour via [ebx+eax] with eax=0 on the first chunk, row buffers at [ebp+0xc01e0]) is not modelled exactly yet."""
import json,numpy as np
d=open('image.bin','rb').read()
def rint(p):
    b=d[p-0x400000]
    if b&1==0:return b>>1,p+1
    return int.from_bytes(d[p-0x400000:p-0x400000+2],'little',signed=True)>>1,p+2
def parse():
    p=0x41903c;A,p=rint(p);code=0;glyphs={}
    while True:
        B,p=rint(p)
        if B<0:break
        code+=B;C,p=rint(p)
        for g in range(C):
            D,p=rint(p);bits=d[p-0x400000:p-0x400000+A*D];p+=A*D;glyphs[code]=(D,bits);code+=1
    return A,glyphs
def glyph(A,D,bits,w=5):
    W=D+4;H=8*A;S=(W+7)&~7;img=np.zeros((H,S),np.int32)
    for x in range(D):
        for by in range(A):
            byte=bits[x*A+by]
            for bit in range(8):
                if not (byte>>bit)&1:img[by*8+bit+1,x+2]=0x7f
    P=np.pad(img,1);s=sum(P[1+dy:1+dy+H,1+dx:1+dx+S] for dy in(-1,0,1) for dx in(-1,0,1))+w*img
    return ((s*(65536//(w+9)))>>16).astype(np.uint8),img
if __name__=='__main__':
    T=json.load(open('glyph_truth.json'));A,gl=parse();ok=0
    for c,(D,bits) in gl.items():
        if str(c) not in T:continue
        w,h,st,hx=T[str(c)];tr=np.frombuffer(bytes.fromhex(hx),np.uint8).reshape(h,st);g,_=glyph(A,D,bits);ok+=int(g.shape==tr.shape and (g==tr).all())
    print(len(gl),'glyphs; bit-exact',ok)
