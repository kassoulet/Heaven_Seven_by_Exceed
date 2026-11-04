#!/usr/bin/env python3
"""texop_vectors.py - differential vectors for texture ops run on random 8-bit surfaces: crack (0x409c7c) now; each line: op w h pitch before(hex, with zero guard rows) after(hex)"""
import struct,random,sys
from unicorn import *; from unicorn.x86_const import *
img=open('image.bin','rb').read();BASE=0x400000;mem=open('mem_a.bin','rb').read()
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,mem[:len(img)]);mu.mem_map(0x10000000,0x100000);mu.mem_map(0x20000000,0x10000)
mu.mem_write(0x20000000,b'\xf4')
SR,SD=0x10000000,0x10010000
def run(fn,w,h,pitch,data):
    mu.mem_write(SD,bytes(data));mu.mem_write(SR+0x18,struct.pack('<IIII',w,h,pitch*4,0));mu.mem_write(SR+0x28,struct.pack('<I',SD+pitch*2))   # 2 guard rows above
    for r,v in ((UC_X86_REG_ESI,SR),(UC_X86_REG_EDI,0),(UC_X86_REG_EBX,0),(UC_X86_REG_ECX,0),(UC_X86_REG_EDX,0),(UC_X86_REG_EAX,0),(UC_X86_REG_EBP,0x20002000)):mu.reg_write(r,v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000))
    try:mu.emu_start(fn,0x20000000,count=400000000)
    except UcError as e:return None
    if mu.reg_read(UC_X86_REG_EIP)!=0x20000000:return None   # instruction limit hit: the original's recursive search can be very slow on big non-zero regions
    return bytes(mu.mem_read(SD,len(data)))
def run_down2(sw,sh,sp,src,dp):
    SD2=SD+0x20000;DR=SR+0x1000;DD=SD+0x30000
    mu.mem_write(SD,bytes(src));mu.mem_write(SR+0x18,struct.pack('<IIII',sw,sh,sp*4,0));mu.mem_write(SR+0x28,struct.pack('<I',SD))
    mu.mem_write(DR+0x18,struct.pack('<IIII',0,0,dp*4,0));mu.mem_write(DR+0x28,struct.pack('<I',DD));mu.mem_write(DD,b'\xaa'*0x4000)
    for r,v in ((UC_X86_REG_ESI,DR),(UC_X86_REG_ECX,SR),(UC_X86_REG_EDI,0),(UC_X86_REG_EBX,0),(UC_X86_REG_EDX,0),(UC_X86_REG_EAX,0),(UC_X86_REG_EBP,0x20002000)):mu.reg_write(r,v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000))
    try:mu.emu_start(0x409c21,0x20000000,count=50000000)
    except UcError:return None
    return bytes(mu.mem_read(DD,0x4000))
random.seed(31);out=[]
while len(out)<80:
    w=random.randrange(8,28);h=random.randrange(8,28);pitch=(w+7)&~7
    total=pitch*(h+4)
    data=bytearray(total+64)
    kind=random.choice(['blobs','noise','box'])
    for y in range(h):
        for x in range(w):
            if kind=='noise':v=random.choice([0,0,random.randrange(1,256)])
            elif kind=='box':v=random.randrange(1,256) if (3<x<w-4 and 3<y<h-4) else 0
            else:v=random.randrange(1,256) if ((x//5+y//5)%2==0 and random.random()<0.9) else 0
            data[(y+2)*pitch+x]=v
    o=run(0x409c7c,w,h,pitch,data)
    if o is None:continue
    out.append('crack %d %d %d %s %s'%(w,h,pitch,bytes(data[:total]).hex(),o[:total].hex()))
random.seed(77)
while len(out)<80+80:
    sw=random.randrange(4,40);sh=random.randrange(4,40);sp=(sw+7)&~7;dp=((sw//2+7)&~7)
    src=bytearray(sp*(sh+2)+64)
    for i in range(len(src)):src[i]=random.choice([0,random.randrange(256),random.randrange(256),random.randrange(0,128)])
    o=run_down2(sw,sh,sp,src,dp)
    if o is None:continue
    out.append('down2 %d %d %d %d %s %s'%(sw,sh,sp,dp,bytes(src).hex(),o[:dp*(sh//2+1)].hex()))
open('texop_vectors.txt','w').write('\n'.join(out)+'\n');print(len(out))

# ---- ops 26 (0x40a3c3 multiply) and 30 (0x40a42a copy): dst = f(dst, tiled 256x256 source #idx)
def run_tile(fn,idx,w,h,dst,srcimg):
    EB=0x20002000;mu.mem_write(EB+0x1e0+idx*0x200+0x28,struct.pack('<I',SD+0x40000));mu.mem_write(SD+0x40000,bytes(srcimg))
    DR=SR+0x1000;DD=SD+0x30000
    mu.mem_write(DR+0x18,struct.pack('<IIII',w,h,w*4,0));mu.mem_write(DR+0x28,struct.pack('<I',DD));mu.mem_write(DD,bytes(dst))
    mu.mem_write(SD+0x50000,bytes([idx*2,0,0,0]))
    for r,v in ((UC_X86_REG_ESI,DR),(UC_X86_REG_EDI,SD+0x50000),(UC_X86_REG_EBX,0),(UC_X86_REG_ECX,0),(UC_X86_REG_EDX,0),(UC_X86_REG_EAX,0),(UC_X86_REG_EBP,EB)):mu.reg_write(r,v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000))
    try:mu.emu_start(fn,0x20000000,count=50000000)
    except UcError:return None
    return bytes(mu.mem_read(DD,len(dst)))
random.seed(5);extra=[]
for name,fn in (('mul',0x40a3c3),('copy',0x40a42a)):
    for k in range(20):
        w=random.choice([8,16,32,64]);h=random.choice([8,16,24,40]);idx=random.randrange(0,5)
        src=bytes(random.randrange(256) for _ in range(65536));dst=bytes(random.randrange(256) for _ in range(w*h))
        o=run_tile(fn,idx,w,h,dst,src)
        if o is None:continue
        extra.append('%s %d %d %d %s %s %s'%(name,idx,w,h,src.hex(),dst.hex(),o.hex()))
open('texop_vectors.txt','a').write('\n'.join(extra)+'\n');print('extra',len(extra))
