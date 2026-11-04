#!/usr/bin/env python3
"""tangent_vectors.py - differential vectors for 0x407d84 (spline tangent builder): 3 key blocks (esi-0x40, esi, esi+0x40; time u32 at +0, floats from +0x24..), n=[edx+4], flag cx.
Line: n cx keys(0xc0 bytes hex) out(n floats hex)"""
import struct,random
from unicorn import *; from unicorn.x86_const import *
img=open('image.bin','rb').read();BASE=0x400000
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,img);mu.mem_map(0x10000000,0x10000);mu.mem_map(0x20000000,0x10000)
ED,KB,OB=0x10000000,0x10001000,0x10002000
mu.mem_write(0x20000000,b'\xf4')
def run(n,cx,keys):
    mu.mem_write(ED,struct.pack('<II',0,n)+b'\0'*0x60);mu.mem_write(KB,keys);mu.mem_write(OB,b'\xaa'*0x40)
    for r,v in ((UC_X86_REG_EDX,ED),(UC_X86_REG_ESI,KB+0x40),(UC_X86_REG_EDI,OB),(UC_X86_REG_ECX,cx),(UC_X86_REG_EAX,0),(UC_X86_REG_EBX,0)):mu.reg_write(r,v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000))
    # the program runs with FPU control word 0x7f (single precision)
    mu.reg_write(UC_X86_REG_FPCW,0x7f)
    try:mu.emu_start(0x407d84,0x20000000)
    except UcError as e:print('err',e)
    return bytes(mu.mem_read(OB+4,4*n))
random.seed(5);out=[]
for i in range(400):
    n=random.randrange(1,5);cx=random.choice([0,1,5])
    t0=random.randrange(0,500);t1=t0+random.randrange(1,300);t2=t1+random.randrange(1,300)
    keys=bytearray(0xc0)
    for k,t in enumerate((t0,t1,t2)):
        struct.pack_into('<I',keys,k*0x40,t)
        for j in range(1,5):struct.pack_into('<f',keys,k*0x40+4*j,random.uniform(-100,100))   # values at +4.. (indexed [ecx*4] with ecx=1..n)
        struct.pack_into('<f',keys,k*0x40+0x28,random.uniform(-1,1));struct.pack_into('<f',keys,k*0x40+0x2c,random.uniform(-1,1));struct.pack_into('<f',keys,k*0x40+0x30,random.uniform(-1,1))
    o=run(n,cx,bytes(keys))
    out.append('%d %d %s %s'%(n,cx,bytes(keys).hex(),o.hex()))
open('tangent_vectors.txt','w').write('\n'.join(out)+'\n');print(len(out),out[0][-40:])
