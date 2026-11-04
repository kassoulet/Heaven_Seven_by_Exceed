#!/usr/bin/env python3
"""pack_vectors.py - run the original 0x407fc0 (varint) and 0x407fd0 (packed float) in Unicorn on random byte strings; write re/pack_vectors.txt: fn hex(bytes) eax consumed"""
import struct,random
from unicorn import *; from unicorn.x86_const import *
img=open('image.bin','rb').read();BASE=0x400000
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,img);mu.mem_map(0x10000000,0x10000);mu.mem_map(0x20000000,0x10000)
def run(fn,data):
    mu.mem_write(0x10000000,data+b'\0'*8);mu.reg_write(UC_X86_REG_EDI,0x10000000);mu.reg_write(UC_X86_REG_ESP,0x20008000)
    mu.mem_write(0x20008000,struct.pack('<I',0x20000000));mu.mem_write(0x20000000,b'\xf4')   # return to hlt
    mu.reg_write(UC_X86_REG_EAX,0);mu.reg_write(UC_X86_REG_ECX,0x1234)
    try:mu.emu_start(fn,0x20000000)
    except UcError:pass
    return mu.reg_read(UC_X86_REG_EAX),mu.reg_read(UC_X86_REG_EDI)-0x10000000
random.seed(7);out=[]
for fn,name in ((0x407fc0,'int'),(0x407fd0,'float')):
    for i in range(300):
        d=bytes(random.randrange(256) for _ in range(4))
        if i%7==0:d=bytes([0])+d[1:]
        eax,used=run(fn,d)
        out.append('%s %s %08x %d'%(name,d.hex(),eax,used))
open('pack_vectors.txt','w').write('\n'.join(out)+'\n');print(len(out),out[:3],out[-3:])
