#!/usr/bin/env python3
"""eval_vectors.py - differential vectors for the whole channel evaluator 0x407e05 with pre-populated keys (the lazy key reader [edx+0x14] and the writer [edx+0x10] are `ret` stubs).
Line: n len t cur_idx keys(hex of the key array incl. pad key 0) out(n floats hex) cur_after_idx coef(first 16*n bytes of [edx+0x68..])"""
import struct,random
from unicorn import *; from unicorn.x86_const import *
img=open('image.bin','rb').read();BASE=0x400000
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,img);mu.mem_map(0x10000000,0x10000);mu.mem_map(0x20000000,0x10000)
ED,KB=0x10000000,0x10001000
mu.mem_write(0x20000000,b'\xf4')
RET=0x40695a
def run(n,length,t,cur,keys):
    st=bytearray(0x100);struct.pack_into('<IIII',st,0,0,n,0,0)
    struct.pack_into('<I',st,0x10,RET);struct.pack_into('<I',st,0x14,RET);struct.pack_into('<I',st,0x18,0);struct.pack_into('<I',st,0x1c,length)
    struct.pack_into('<I',st,0x20,KB);struct.pack_into('<I',st,0x24,KB+0x40*cur if cur else 0)
    mu.mem_write(ED,bytes(st));mu.mem_write(KB,keys)
    for r,v in ((UC_X86_REG_EDX,ED),(UC_X86_REG_EBX,t),(UC_X86_REG_ESI,0),(UC_X86_REG_EDI,0),(UC_X86_REG_ECX,3),(UC_X86_REG_EAX,0)):mu.reg_write(r,v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000));mu.reg_write(UC_X86_REG_FPCW,0x7f)
    try:mu.emu_start(0x407e05,0x20000000)
    except UcError as e:print('err',e)
    o=bytes(mu.mem_read(ED+0x2c,4*n));ca=(struct.unpack('<I',bytes(mu.mem_read(ED+0x24,4)))[0]-KB)//0x40;co=bytes(mu.mem_read(ED+0x68,16*n))
    return o,ca,co
random.seed(9);out=[]
for i in range(500):
    n=random.randrange(1,5);K=7
    ts=sorted(random.sample(range(1,600),K))
    keys=bytearray(0x40*(K+2))
    for k in range(K+2):
        tt=ts[min(max(k-1,0),K-1)] if 1<=k<=K else (0 if k==0 else ts[-1]+100)
        struct.pack_into('<I',keys,k*0x40,tt)
        for j in range(1,5):struct.pack_into('<f',keys,k*0x40+4*j,random.uniform(-100,100))
        for off in (0x20,0x24,0x28,0x2c,0x30):struct.pack_into('<f',keys,k*0x40+off,random.choice([0.0,random.uniform(-1,1),random.uniform(0,1),random.uniform(0,3)]))
    length=ts[-1]+100
    t=random.randrange(ts[0]+1,ts[-2]) if random.random()<0.9 else random.choice([0,-5,ts[0],ts[1]])
    cur=random.choice([0,0,0,random.randrange(1,K-2)])
    o,ca,co=run(n,length,t,cur,bytes(keys))
    out.append('%d %d %d %d %s %s %d %s'%(n,length,t,cur,bytes(keys).hex(),o.hex(),ca,co.hex()))
open('eval_vectors.txt','w').write('\n'.join(out)+'\n');print(len(out),out[0][-120:])
