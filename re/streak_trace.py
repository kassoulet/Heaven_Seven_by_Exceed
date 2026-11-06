import struct,sys
from unicorn import *; from unicorn.x86_const import *
k=int(sys.argv[1]) if len(sys.argv)>1 else 1
raw=open('/tmp/claude-1000/w/hs408fab_%d.bin'%k,'rb').read()
IMG=188416;img=raw[:IMG];heap=raw[IMG:]
mu=Uc(UC_ARCH_X86,UC_MODE_32)
mu.mem_map(0x400000,(IMG+0xfff)&~0xfff);mu.mem_write(0x400000,img)
mu.mem_map(0x20000000,(len(heap)+0x100000+0xfff)&~0xfff);mu.mem_write(0x20000000,heap)
mu.mem_map(0xf00000,0x100000)
r32=lambda a:struct.unpack('<I',bytes(mu.mem_read(a,4)))[0]
ebp0=r32(0x42a3f8);root=r32(ebp0+0x50);vp=r32(root+0x18)
node=[n for n in range(0)]
# the class-50 node: first draw node with handler 0x408fab: walk tree
def walk(n,out):
    while n:
        if r32(n+0x30)==0x408fab:out.append(n)
        c=r32(n+8)
        if c:walk(c,out)
        n=r32(n+4)
out=[];walk(vp,out);nd=out[0]
import struct as S
f=lambda a:S.unpack('<f',bytes(mu.mem_read(a,4)))[0]
f34,f38=f(nd+0x34),f(nd+0x38)
import numpy as np
s0=np.float32(f38)*np.float32(65536.0);A=int(np.rint(np.float32(f34)*s0));B=int(np.rint(s0*(np.float32(1)-np.float32(f34))))
f30=f(vp+0x30);x=int(np.rint((np.float32(f(nd+0x28))+np.float32(f(vp+0x34)))*np.float32(f30)));y=int(np.rint((-np.float32(f(nd+0x2c))+np.float32(f(vp+0x38)))*np.float32(f30)))
print('A',hex(A),'B',hex(B),'pos',hex(x),hex(y))
d8=r32(vp+0x28);pw=r32(vp+0x20)>>2;h=r32(vp+0x1c)
STK=0xff0000
mu.mem_write(STK,S.pack('<II',x&0xffffffff,y&0xffffffff))
mu.reg_write(UC_X86_REG_ESP,0xfff000);mu.mem_write(0xfff000,S.pack('<I',0xdead0000))
mu.mem_write(0xdead0000&0xffffffff if False else 0xf80000,b'')
for r,v in ((UC_X86_REG_EAX,A),(UC_X86_REG_EDX,B),(UC_X86_REG_EBX,STK),(UC_X86_REG_ESI,vp),(UC_X86_REG_EDI,0),(UC_X86_REG_EBP,0)):mu.reg_write(r,v&0xffffffff)
W=[]
def hk(uc,acc,addr,size,val,_):
    if d8<=addr<d8+pw*h+64:W.append((addr-d8,size,val&((1<<(8*size))-1)))
mu.hook_add(UC_HOOK_MEM_WRITE,hk)
# return address: map a page with 'hlt'
mu.mem_map(0xdead0000,0x1000);mu.mem_write(0xdead0000,b'\xf4')
mu.emu_start(0x40ac69,0xdead0000,count=50000000)
print('writes',len(W))
import pickle;pickle.dump(W,open('/tmp/claude-1000/w/streak_writes_%d.pkl'%k,'wb'))
print(W[:20])
