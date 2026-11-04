#!/usr/bin/env python3
"""handler_vectors.py - differential vectors for the simple operator handlers (0x408xxx): random operator state (edx, 0x60 bytes), stream (edi), out buffer (ebx); records
state/out/stream use after running the ORIGINAL handler in Unicorn.  Line: name | edx_before | stream | edx_after | out(16 bytes) | used"""
import struct,random
from unicorn import *; from unicorn.x86_const import *
img=open('image.bin','rb').read();BASE=0x400000
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,img);mu.mem_map(0x10000000,0x10000);mu.mem_map(0x20000000,0x10000)
ED,SB,OB=0x10000000,0x10001000,0x10002000
def run(fn,edx,stream,ebx_ofs=0):
    mu.mem_write(ED,edx);mu.mem_write(SB,stream+b'\0'*16);mu.mem_write(OB,b'\xaa'*32)
    for r,v in ((UC_X86_REG_EDX,ED),(UC_X86_REG_EDI,SB),(UC_X86_REG_EBX,OB),(UC_X86_REG_ESI,0),(UC_X86_REG_ECX,0x77),(UC_X86_REG_EAX,0x55),(UC_X86_REG_EBP,0x30000000)):mu.reg_write(r,v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000));mu.mem_write(0x20000000,b'\xf4')
    try:mu.emu_start(fn,0x20000000)
    except UcError:pass
    return bytes(mu.mem_read(ED,len(edx))),bytes(mu.mem_read(OB,16)),mu.reg_read(UC_X86_REG_EDI)-SB,mu.reg_read(UC_X86_REG_EBX)-OB
random.seed(11);out=[]
def fl(x):return struct.pack('<f',x)
def rstate(n_in=4):
    s=bytearray(0x60)
    struct.pack_into('<I',s,0,random.randrange(1,5));struct.pack_into('<I',s,4,random.randrange(1,5))
    for k in range(0x20,0x60,4):
        if k in(0x20,0x24):struct.pack_into('<I',s,k,random.randrange(0,1<<16))
        else:struct.pack_into('<f',s,k,random.choice([random.uniform(-300,300),random.uniform(0,255),random.randrange(-5,300)+0.5,random.randrange(0,256)]))
    return bytes(s)
for name,fn,n in (('cmp40',0x408040,200),('f2i51',0x408051,200),('byte2f57',0x408057,100),('f2b64',0x408064,200),('int2f6f',0x40806f,200),('quad4f7d',0x40807d,300),('copy1b0',0x4081b0,100),('fill1c1',0x4081c1,100)):
    for i in range(n):
        st=rstate();sm=bytes(random.randrange(256) for _ in range(24))
        ea,oo,used,eb=run(fn,st,sm)
        out.append('%s %s %s %s %s %d %d'%(name,st.hex(),sm.hex(),ea.hex(),oo.hex(),used,eb))
open('handler_vectors.txt','w').write('\n'.join(out)+'\n');print(len(out),out[0][:200])
