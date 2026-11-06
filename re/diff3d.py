#!/usr/bin/env python3
"""diff3d.py <psnap.bin> <t> <out.bin> [writes.pkl]: run the ORIGINAL 3D draw (0x40a680, first enabled class-13 node under the viewport) in Unicorn from a first-draw snapshot and dump the whole memory
afterwards (image ++ heap); with a 4th argument also log every write (addr,size,value). The Rust twin is cold::tests::diff3d (orig_gen), compare with re/cmpmem.py."""
import struct,sys,pickle
from unicorn import *; from unicorn.x86_const import *
raw=open(sys.argv[1],'rb').read();t=int(sys.argv[2])
IMG=188416;img=raw[:IMG];heap=raw[IMG:]
mu=Uc(UC_ARCH_X86,UC_MODE_32)
mu.mem_map(0x400000,(IMG+0xfff)&~0xfff);mu.mem_write(0x400000,img)
mu.mem_map(0x20000000,(len(heap)+0x4000000+0xfff)&~0xfff);mu.mem_write(0x20000000,heap)
mu.mem_map(0xf00000,0x100000);mu.mem_map(0xdead0000,0x1000);mu.mem_write(0xdead0000,b'\xf4')
r32=lambda a:struct.unpack('<I',bytes(mu.mem_read(a,4)))[0]
ebp0=r32(0x42a3f8);root=r32(ebp0+0x50);vp=r32(root+0x18)
out=[]
def walk(n):
    while n:
        if r32(n+0x30)==0x40a680 and mu.mem_read(n,1)[0]:out.append(n)
        c=r32(n+8)
        if c:walk(c)
        n=r32(n+4)
walk(vp);nd=out[0];print('3D nodes enabled',[hex(x) for x in out],'using',hex(nd),'vp',hex(vp),'ebp',hex(ebp0))
mu.reg_write(UC_X86_REG_ESP,0xfff000);mu.mem_write(0xfff000,struct.pack('<I',0xdead0000))
for r,v in ((UC_X86_REG_EBX,t),(UC_X86_REG_ESI,nd),(UC_X86_REG_EDI,vp),(UC_X86_REG_EBP,ebp0),(UC_X86_REG_EAX,0),(UC_X86_REG_ECX,0),(UC_X86_REG_EDX,0)):mu.reg_write(r,v)
mu.reg_write(UC_X86_REG_FPCW,0x7f)
if len(sys.argv)>4:
    W=[]
    mu.hook_add(UC_HOOK_MEM_WRITE,lambda uc,ac,a,sz,v,_:W.append((a,sz,v&((1<<(8*sz))-1) if sz<=8 else v)),begin=0x400000,end=0x30000000)
mu.emu_start(0x40a680,0xdead0000,count=400000000)
open(sys.argv[3],'wb').write(bytes(mu.mem_read(0x400000,IMG))+bytes(mu.mem_read(0x20000000,len(heap))))
if len(sys.argv)>4:pickle.dump(W,open(sys.argv[4],'wb'))
print('done')
