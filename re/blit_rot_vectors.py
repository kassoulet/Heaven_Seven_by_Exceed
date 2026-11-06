#!/usr/bin/env python3
"""blit_rot_vectors.py - differential vectors for the rotated sprite blitter 0x40ccc5 (angle f32 in ebx; the source is addressed with a fixed pitch of 64).
Line: srcw srch dstw dsth angle_bits yparam rx ry rw rh src seed (LCG x*1664525+1013904223, byte = (x>>24)&0x7f, 64*256+0x100 bytes) dstbefore(hex) dstafter(hex)"""
import struct,random,math
from unicorn import *; from unicorn.x86_const import *
img=open('image.bin','rb').read();BASE=0x400000;mem=open('mem_a.bin','rb').read()
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,mem[:len(img)]);mu.mem_map(0x10000000,0x100000);mu.mem_map(0x20000000,0x10000)
mu.mem_write(0x20000000,b'\xf4')
SR,DR,SD,DD,RC,PR=0x10000000,0x10001000,0x10002000,0x10030000,0x10000800,0x10000900
SRCN=64*256+0x100
def run(sw,sh,dw,dh,ab,yp,rect,src,dst):
    dp=dw
    mu.mem_write(SD,bytes(src));mu.mem_write(DD,bytes(dst))
    mu.mem_write(SR+0x18,struct.pack('<IIII',sw,sh,64*4,0));mu.mem_write(SR+0x28,struct.pack('<I',SD))
    mu.mem_write(DR+0x18,struct.pack('<IIII',dw,dh,dp*4,0));mu.mem_write(DR+0x28,struct.pack('<I',DD))
    mu.mem_write(RC,struct.pack('<iiii',*rect));mu.mem_write(PR,struct.pack('<Ii',0,yp))
    for r,v in ((UC_X86_REG_EAX,PR),(UC_X86_REG_ESI,RC),(UC_X86_REG_ECX,SR),(UC_X86_REG_EDI,DR),(UC_X86_REG_EBX,ab),(UC_X86_REG_EDX,0),(UC_X86_REG_EBP,0)):mu.reg_write(r,v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000));mu.reg_write(UC_X86_REG_FPCW,0x7f)
    try:mu.emu_start(0x40ccc5,0x20000000)
    except UcError as e:return None
    return bytes(mu.mem_read(DD,len(dst)))
random.seed(5);out=[]
while len(out)<300:
    sw=random.randrange(8,57);sh=random.randrange(8,49)
    dw=random.choice([32,48,64,80]);dh=random.choice([24,32,48,64])
    seed=random.randrange(1<<32);x=seed;sb=bytearray()
    for _ in range(SRCN):
        x=(x*1664525+1013904223)&0xffffffff;sb.append((x>>24)&0x7f)
    src=bytes(sb)
    dst=bytearray(dw*dh+64)
    for i in range(dw*dh):dst[i]=random.randrange(0,60)
    rx=random.randrange(-3*65536,(dw+10)*65536);ry=random.randrange(-3*65536,(dh+10)*65536)
    rw=random.randrange(0x8001+1,int(1.0*sw*65536));rh=random.randrange(0x8001+1,int(1.0*sh*65536))
    ang=random.choice([0.0,math.pi/2,-math.pi/2,math.pi,random.uniform(-math.pi,math.pi),random.uniform(-math.pi,math.pi),random.uniform(-0.5,0.5)])
    ab=struct.unpack('<I',struct.pack('<f',ang))[0];yp=random.choice([random.randrange(0,0x100),random.randrange(0,0x40),random.randrange(0,0x3fff)])<<16|random.randrange(0,65536)
    o=run(sw,sh,dw,dh,ab,yp,(rx,ry,rw,rh),src,dst)
    if o is None:continue
    out.append('%d %d %d %d %d %d %d %d %d %d %s %s %s'%(sw,sh,dw,dh,ab,yp,rx,ry,rw,rh,seed,bytes(dst).hex(),o.hex()))
open('blit_rot_vectors.txt','w').write('\n'.join(out)+'\n');print(len(out),sum(1 for l in out if l.split()[11]!=l.split()[12]),'changed')
