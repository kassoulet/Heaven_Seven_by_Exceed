#!/usr/bin/env python3
"""blit_vectors.py - differential vectors for the bilinear glyph blitter 0x40c8d0.
Line: srcw srch dstw dsth xparam yparam rx ry rw rh srcdata(hex, padded) dstbefore(hex) dstafter(hex)"""
import struct,random
from unicorn import *; from unicorn.x86_const import *
img=open('image.bin','rb').read();BASE=0x400000;mem=open('mem_a.bin','rb').read()
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,mem[:len(img)]);mu.mem_map(0x10000000,0x100000);mu.mem_map(0x20000000,0x10000)
mu.mem_write(0x20000000,b'\xf4')
SR,DR,SD,DD,RC,PR=0x10000000,0x10001000,0x10002000,0x10010000,0x10003000,0x10003100
def run(sw,sh,dw,dh,xp,yp,rect,src,dst):
    sp=((sw+7)&~7);dp=dw
    mu.mem_write(SD,bytes(src));mu.mem_write(DD,bytes(dst))
    mu.mem_write(SR+0x18,struct.pack('<IIII',sw,sh,sp*4,0));mu.mem_write(SR+0x28,struct.pack('<I',SD))
    mu.mem_write(DR+0x18,struct.pack('<IIII',dw,dh,dp*4,0));mu.mem_write(DR+0x28,struct.pack('<I',DD))
    mu.mem_write(RC,struct.pack('<iiii',*rect));mu.mem_write(PR,struct.pack('<Ii',xp,yp))
    for r,v in ((UC_X86_REG_EAX,PR),(UC_X86_REG_ESI,RC),(UC_X86_REG_ECX,SR),(UC_X86_REG_EDI,DR),(UC_X86_REG_EBX,0),(UC_X86_REG_EDX,0),(UC_X86_REG_EBP,0)):mu.reg_write(r,v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000));mu.reg_write(UC_X86_REG_FPCW,0x7f)
    try:mu.emu_start(0x40c8d0,0x20000000)
    except UcError as e:return None
    return bytes(mu.mem_read(DD,len(dst)))
random.seed(21);out=[]
while len(out)<300:
    sw=random.randrange(8,41);sh=random.randrange(8,49);sp=(sw+7)&~7
    dw=random.choice([32,48,64]);dh=random.choice([24,32,48])
    src=bytearray(sp*(sh+2)+64)
    for y in range(sh):
        for x in range(sw):src[(y+1)*sp+x]=random.choice([0,0,random.randrange(0,128)])    # glyph pixels start one row down? (the blitter reads the row above the first)
    dst=bytearray(dw*dh+64)
    for i in range(dw*dh):dst[i]=random.randrange(0,60)
    rx=random.randrange(-5*65536,(dw+10)*65536);ry=random.randrange(-5*65536,(dh+10)*65536)
    rw=random.randrange(0x8001+1,int(1.5*sw*65536));rh=random.randrange(0x8001+1,int(1.5*sh*65536))
    xp=random.randrange(0,1<<31);yp=random.randrange(0,0x3fff)<<16|random.randrange(0,65536)
    # keep the source accesses inside the padded buffer: only accept if a dry run does not crash
    o=run(sw,sh,dw,dh,xp,yp,(rx,ry,rw,rh),src,dst)
    if o is None:continue
    out.append('%d %d %d %d %d %d %d %d %d %d %s %s %s'%(sw,sh,dw,dh,xp,yp,rx,ry,rw,rh,bytes(src).hex(),bytes(dst).hex(),o.hex()))
open('blit_vectors.txt','w').write('\n'.join(out)+'\n');print(len(out),sum(1 for l in out if l.split()[11]!=l.split()[12]),'changed')
