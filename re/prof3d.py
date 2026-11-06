import struct,sys,collections
from unicorn import *; from unicorn.x86_const import *
k=int(sys.argv[1]) if len(sys.argv)>1 else 1
raw=open('/tmp/claude-1000/w/psnap%d.bin'%k,'rb').read()
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
        if r32(n+0x30)==0x40a680:out.append(n)
        c=r32(n+8)
        if c:walk(c)
        n=r32(n+4)
walk(vp);nd=out[0]
t=int(sys.argv[2]) if len(sys.argv)>2 else 0
mu.reg_write(UC_X86_REG_ESP,0xfff000);mu.mem_write(0xfff000,struct.pack('<I',0xdead0000))
for r,v in ((UC_X86_REG_EBX,t),(UC_X86_REG_ESI,nd),(UC_X86_REG_EDI,vp),(UC_X86_REG_EBP,ebp0),(UC_X86_REG_EAX,0),(UC_X86_REG_ECX,0),(UC_X86_REG_EDX,0)):mu.reg_write(r,v)
C=collections.Counter()
def hk(uc,a,sz,_):C[a]+=1
mu.hook_add(UC_HOOK_CODE,hk,begin=0x401000,end=0x41d000)
mu.emu_start(0x40a680,0xdead0000,count=400000000)
print('instr',sum(C.values()),'eip',hex(mu.reg_read(UC_X86_REG_EIP)))
# call-target entry points: group addresses by 0x100 pages
P=collections.Counter()
for a,n in C.items():P[a>>8]+=n
for p,n in P.most_common(25):print(hex(p<<8),n)
import pickle;pickle.dump(C,open('/tmp/claude-1000/w/prof3d_%d.pkl'%k,'wb'))
