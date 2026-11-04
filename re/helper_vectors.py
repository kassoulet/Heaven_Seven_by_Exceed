#!/usr/bin/env python3
"""helper_vectors.py - differential vectors for the image-program helpers. Lines: kind args... (hex/ints)
  sample: esi plane(256x256 dwords) ebx(16-bit) eax(al=dx,ah=dy) ecx edx -> eax"""
import struct,random
from unicorn import *; from unicorn.x86_const import *
img=open('image.bin','rb').read();BASE=0x400000;mem=open('mem_a.bin','rb').read()
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,mem[:len(img)]);mu.mem_map(0x10000000,0x400000);mu.mem_map(0x20000000,0x10000)
mu.mem_write(0x20000000,b'\xf4')
PL=0x10000000
def call(fn,regs,ret=('eax',)):
    for r,v in regs.items():mu.reg_write(getattr(__import__('unicorn.x86_const',fromlist=['x']),'UC_X86_REG_'+r.upper()),v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000));mu.reg_write(UC_X86_REG_FPCW,0x7f)
    try:mu.emu_start(fn,0x20000000,count=100000)
    except UcError as e:return None
    return [mu.reg_read(getattr(__import__('unicorn.x86_const',fromlist=['x']),'UC_X86_REG_'+r.upper())) for r in ret]
random.seed(3);out=[]
plane=bytes(random.randrange(256) for _ in range(256*256*4));mu.mem_write(PL,plane)
open('plane.bin','wb').write(plane)
for i in range(300):
    ebx=random.randrange(65536);eax=random.randrange(65536);ecx=random.randrange(0x8000);edx=random.randrange(0x8000)
    r=call(0x40d4b1,{'esi':PL,'ebx':ebx,'eax':eax,'ecx':ecx,'edx':edx,'edi':0,'ebp':0x20002000})
    if r is None:continue
    out.append('sample %d %d %d %d %d'%(ebx,eax,ecx,edx,r[0]))
open('helper_vectors.txt','w').write('\n'.join(out)+'\n');print(len(out))
