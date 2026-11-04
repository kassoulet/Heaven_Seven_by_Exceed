#!/usr/bin/env python3
"""unpack_upx_pe.py - unpack a UPX 1.0x PE by running its own stub in Unicorn.
The stub decompresses UPX1 into UPX0 (NRV2B), rebuilds the import table with LoadLibraryA/GetProcAddress and finally jumps to the original entry (OEP)
inside UPX0. We emulate the 3 kernel32 functions it uses (LoadLibraryA, GetProcAddress, VirtualProtect), log every imported name, and stop at the first
instruction fetched from UPX0. Output: image.bin (0x400000.. mapped image after decompression), imports.json (fake address -> 'dll!name'), oep.
usage: unpack_upx_pe.py HEAVEN7W.EXE OUTDIR"""
import sys,struct,json,os
from unicorn import *; from unicorn.x86_const import *
d=open(sys.argv[1],'rb').read();out=sys.argv[2];os.makedirs(out,exist_ok=True)
pe=struct.unpack('<I',d[0x3c:0x40])[0];ns=struct.unpack('<H',d[pe+6:pe+8])[0];opt=struct.unpack('<H',d[pe+20:pe+22])[0];o=pe+24
ep=struct.unpack('<I',d[o+16:o+20])[0];base=struct.unpack('<I',d[o+28:o+32])[0];soi=struct.unpack('<I',d[o+56:o+60])[0]
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(base,(soi+0xfff)&~0xfff)
mu.mem_write(base,d[:struct.unpack('<I',d[o+60:o+64])[0]])
secs=[]
s=o+opt
for i in range(ns):
    n=d[s+i*40:s+i*40+8].rstrip(b'\0').decode();vs,va,rs,rp=struct.unpack('<IIII',d[s+i*40+8:s+i*40+24]);secs.append((n,va,vs,rp,rs))
    mu.mem_write(base+va,d[rp:rp+rs])
u0=[x for x in secs if x[0]=='UPX0'][0];lo,hi=base+u0[1],base+u0[1]+u0[2]
STK=0x1000000;mu.mem_map(STK-0x100000,0x100000);mu.mem_map(0x7f000000,0x10000);mu.mem_map(0x7e000000,0x10000)
mu.reg_write(UC_X86_REG_ESP,STK-0x100)
names={};libs=[];nxt=[0x7f000100]
def cs(a):
    b=b''
    while True:
        c=mu.mem_read(a,1)
        if c==b'\0':return b.decode('latin1')
        b+=bytes(c);a+=1
def code(uc,addr,size,_):
    if addr==0x7f000010:   # LoadLibraryA
        sp=uc.reg_read(UC_X86_REG_ESP);r=struct.unpack('<I',uc.mem_read(sp,4))[0];n=cs(struct.unpack('<I',uc.mem_read(sp+4,4))[0]);libs.append(n);uc.reg_write(UC_X86_REG_EAX,0x7e000000+len(libs)*0x100);uc.reg_write(UC_X86_REG_EIP,r);uc.reg_write(UC_X86_REG_ESP,sp+8)
    elif addr==0x7f000020: # GetProcAddress(h,name)
        sp=uc.reg_read(UC_X86_REG_ESP);r,h,p=struct.unpack('<III',uc.mem_read(sp,12))
        nm=cs(p) if p>0x10000 else 'ord%d'%p;a=nxt[0];nxt[0]+=16;names[a]=libs[((h-0x7e000000)>>8)-1]+'!'+nm
        uc.reg_write(UC_X86_REG_EAX,a);uc.reg_write(UC_X86_REG_EIP,r);uc.reg_write(UC_X86_REG_ESP,sp+12)
    elif addr==0x7f000030: # VirtualProtect(addr,size,prot,old*)
        sp=uc.reg_read(UC_X86_REG_ESP);r,a,sz,pr,op=struct.unpack('<IIIII',uc.mem_read(sp,20));uc.mem_write(op,b'\x40\0\0\0');uc.reg_write(UC_X86_REG_EAX,1);uc.reg_write(UC_X86_REG_EIP,r);uc.reg_write(UC_X86_REG_ESP,sp+20)
    elif lo<=addr<hi:
        print('OEP',hex(addr));json.dump({'oep':addr,'imports':{hex(k):v for k,v in names.items()},'libs':libs},open(out+'/info.json','w'),indent=1)
        open(out+'/image.bin','wb').write(bytes(uc.mem_read(base,soi)));uc.emu_stop()
mu.hook_add(UC_HOOK_CODE,code)
def unm(uc,acc,addr,size,val,_):
    print('unmapped',acc,hex(addr),hex(uc.reg_read(UC_X86_REG_EIP)));return False
mu.hook_add(UC_HOOK_MEM_UNMAPPED,unm)
# thunks the stub's IAT points at: kernel32 entries are resolved by the loader; find them from the import dir and write our fake addrs
imp_rva,_=struct.unpack('<II',d[o+104:o+112]);ia=base+imp_rva
fake={'LoadLibraryA':0x7f000010,'GetProcAddress':0x7f000020,'VirtualProtect':0x7f000030,'ExitProcess':0x7f000040}
p=ia
while True:
    ol,_,_,nm,ft=struct.unpack('<IIIII',bytes(mu.mem_read(p,20)))
    if not nm:break
    t=base+(ol or ft);ft_=base+ft
    while True:
        v=struct.unpack('<I',bytes(mu.mem_read(t,4)))[0]
        if not v:break
        n=cs(base+v+2) if not v&0x80000000 else "ord"
        mu.mem_write(ft_,struct.pack('<I',fake.get(n,0x7f000050)));t+=4;ft_+=4
    p+=20
mu.emu_start(base+ep,0,count=200000000)
