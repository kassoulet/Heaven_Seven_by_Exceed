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
    try:mu.emu_start(fn,0x20000000,count=50000000)
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
for i in range(400):
    v=random.randrange(1<<24) if i%5 else random.choice([0,0x010101,0x808080,0xffffff,random.randrange(256)<<16])
    r=call(0x40d536,{'eax':v,'ebx':0,'ecx':0,'edx':0,'esi':0,'edi':0,'ebp':0x20002000})
    if r:out.append('rgb2hsv %d %d'%(v,r[0]))
    r=call(0x40d604,{'eax':v,'ebx':0,'ecx':0,'edx':0,'esi':0,'edi':0,'ebp':0x20002000})
    if r:out.append('hsv2rgb %d %d'%(v,r[0]))
BL={'add':0x40d7b0,'sub':0x40d7bd,'diff':0x40d7ca,'mulinv':0x40d811,'screen':0x40d833,'mul':0x40d866,'overlay':0x40d883}
for name,fn in BL.items():
    for i in range(150):
        a=random.randrange(1<<32);b=random.randrange(1<<32)
        r=call(fn,{'eax':a,'ebx':b,'ecx':0,'edx':0,'esi':0,'edi':0,'ebp':0x20002000},ret=('eax','ebx'))
        if r:out.append('blend_%s %d %d %d %d'%(name,a,b,r[0],r[1]))
open('helper_vectors.txt','w').write('\n'.join(out)+'\n');print(len(out))

# ---- plane operations: auto-levels 0x40d684 and layer combine 0x40d758+0x40d77a. Planes come from an xorshift32 generator (same in Rust); outputs are compared by FNV-1a 64.
def xs(seed):
    x=seed
    while True:
        x^=(x<<13)&0xffffffff;x^=x>>17;x^=(x<<5)&0xffffffff;yield x
def plane_from(seed,lo=(0,0,0),span=(256,256,256)):
    g=xs(seed);out=bytearray()
    for _ in range(65536):
        x=next(g);out+=bytes([lo[0]+(x&0xff)%span[0],lo[1]+((x>>8)&0xff)%span[1],lo[2]+((x>>16)&0xff)%span[2],(x>>24)&0xff])
    return bytes(out)
def fnv(b):
    h=0xcbf29ce484222325
    for c in b:h=((h^c)*0x100000001b3)&0xffffffffffffffff
    return h
extra=[]
for seed,lo,span in ((1,(0,0,0),(256,256,256)),(2,(10,40,0),(100,30,200)),(3,(0,0,0),(1,50,255)),(4,(100,100,100),(5,5,5)),(5,(0,200,20),(256,56,60))):
    pl=plane_from(seed,lo,span);mu.mem_write(PL,pl)
    r=call(0x40d684,{'esi':PL,'edi':PL+0x100000,'ebx':0,'ecx':0,'edx':0,'eax':0,'ebp':0x20002000})
    if r is None:continue
    o=bytes(mu.mem_read(PL+0x100000,0x40000))
    extra.append('autolevels %d %d %d %d %d %d %d %d'%(seed,*lo,*span,fnv(o)))
for seed in range(1,5):
  for code in (0x01,0x12,0x23,0x34,0x45,0x56,0x67,0x77,0x70,0x17):
    s=plane_from(seed*7);d=plane_from(seed*7+1)
    mu.mem_write(PL,s);mu.mem_write(PL+0x100000,d)
    r=call(0x40d758,{'eax':code,'ebx':0,'ecx':0,'edx':0,'esi':0,'edi':0,'ebp':0x20002000})
    r=call(0x40d77a,{'esi':PL,'edi':PL+0x100000,'eax':0,'ebx':0,'ecx':0,'edx':0,'ebp':0x20002000})
    if r is None:continue
    o=bytes(mu.mem_read(PL+0x100000,0x40000))
    extra.append('combine %d %d %d'%(seed*7,code,fnv(o)))
open('plane_vectors.txt','w').write('\n'.join(extra)+'\n');print('plane vectors',len(extra))
