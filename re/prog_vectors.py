#!/usr/bin/env python3
"""prog_vectors.py - whole-interpreter vectors: run a small image program through the ORIGINAL 0x40d43f (op 20's generator) in Unicorn; output plane hashed with FNV-1a.
Line: hex(program bytes) fnv(output 256x256x4). Programs end with a result code (1..4 = plane n)."""
import struct,random,sys
from unicorn import *; from unicorn.x86_const import *
img=open('image.bin','rb').read();BASE=0x400000;mem=open('mem_a.bin','rb').read()
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,mem[:len(img)]);mu.mem_map(0x10000000,0x400000);mu.mem_map(0x20000000,0x10000);mu.mem_map(0x30000000,0x200000)
mu.mem_write(0x20000000,b'\xf4')
WORK=0x30000000;OUT=0x10100000;STREAM=0x10000000
mu.mem_write(0x42a210,struct.pack('<I',WORK))
def fnv(b):
    h=0xcbf29ce484222325
    for c in b:h=((h^c)*0x100000001b3)&0xffffffffffffffff
    return h
def run(prog):
    mu.mem_write(STREAM,bytes(prog)+b'\0'*16);mu.mem_write(0x42a210,struct.pack('<I',WORK));mu.mem_write(OUT,b'\xcc'*0x40000)
    for r,v in (('eax',OUT),('edi',STREAM),('esi',0),('ebx',0),('ecx',0),('edx',0),('ebp',0x20002000)):mu.reg_write(getattr(__import__('unicorn.x86_const',fromlist=['x']),'UC_X86_REG_'+r.upper()),v)
    mu.reg_write(UC_X86_REG_ESP,0x20008000);mu.mem_write(0x20008000,struct.pack('<I',0x20000000));mu.reg_write(UC_X86_REG_FPCW,0x7f)
    try:mu.emu_start(0x40d43f,0x20000000,count=2000000000)
    except UcError as e:return None
    if mu.reg_read(UC_X86_REG_EIP)!=0x20000000:return None
    return bytes(mu.mem_read(OUT,0x40000))
if __name__=='__main__':
    random.seed(2);out=[]
    for i in range(30):
        n=random.randrange(1,5);code=random.choice([0x07,0x17,0x27,0x37,0x47,0x57,0x67,0x77,0x03,0x15]);v=random.randrange(256)
        prog=[0x10,n,code,v,n]
        o=run(prog)
        if o:out.append('%s %d'%(bytes(prog).hex(),fnv(o)))
    for i in range(12):
        n=random.randrange(1,5);code=random.choice([0x07,0x17,0x57,0x67,0x77]);seed=random.randrange(1<<32);amp=random.choice([0x1000000,0x40000000,random.randrange(1<<30),0x7fffffff])
        prog=[0x11,n,code]+list(struct.pack('<I',seed))+list(struct.pack('<i',amp))+[n]
        o=run(prog)
        if o:out.append('%s %d'%(bytes(prog).hex(),fnv(o)))
    for i in range(14):
        n=random.randrange(1,5);code=random.choice([0x07,0x17,0x57,0x67,0x77]);seed=random.randrange(1<<32);k=random.choice([1,2,3,4,5,6,7])
        prog=[0x12,n,code]+list(struct.pack('<I',seed))+[k,n]
        o=run(prog)
        if o:out.append('%s %d'%(bytes(prog).hex(),fnv(o)))
    for i in range(14):
        n=random.randrange(1,5);code=random.choice([0x07,0x17,0x57,0x67,0x77])
        prog=[0x13,n,code]+[random.randrange(0,40) for _ in range(4)]+[n]
        o=run(prog)
        if o:out.append('%s %d'%(bytes(prog).hex(),fnv(o)))
    for i in range(12):
        n=random.randrange(1,5);code=random.choice([0x07,0x17,0x57,0x67,0x77]);seed=random.randrange(1<<32)
        prog=[0x14,n,code]+list(struct.pack('<I',seed))+[random.randrange(0,7),random.randrange(1,70),n]
        o=run(prog)
        if o:out.append('%s %d'%(bytes(prog).hex(),fnv(o)))
    for i in range(12):
        n=random.randrange(1,5);code=random.choice([0x07,0x17,0x57,0x67,0x77])
        prog=[0x15,n,code,random.choice([1,2,17,64,100,128,200,255,random.randrange(256)]),n]
        o=run(prog)
        if o:out.append('%s %d'%(bytes(prog).hex(),fnv(o)))
    # filter handlers: planes 1..3 are first filled with noise (0x12), then one filter op into plane n, then result n
    def pre():
        r=[]
        for pl,cd in ((1,0x07),(2,0x07),(3,0x03)):
            r+=[0x12,pl,cd]+list(struct.pack('<I',random.randrange(1<<32)))+[random.choice([2,3,4,5])]
        return r
    def src():return random.choice([0,1,2,3,1,2,3])
    def u8():return random.randrange(256)
    FIL=[(0x20,lambda n:[n,cd(),src(),u8(),u8()]),
         (0x21,lambda n:[n,cd(),src()]+[random.randrange(0,12),u8(),u8(),random.randrange(0,12),u8(),u8()]),
         (0x24,lambda n:[n,cd(),src(),src(),random.randrange(4),random.randrange(0,64),random.randrange(0,64)]),
         (0x31,lambda n:[n,cd(),src(),u8()]),(0x32,lambda n:[n,cd(),src(),u8()]),(0x33,lambda n:[n,cd(),src(),random.randrange(4),u8()]),
         (0x35,lambda n:[n,cd(),src(),random.randrange(4)]+[u8() for _ in range(8)]),
         (0x36,lambda n:[n,cd(),src(),u8()]),(0x37,lambda n:[n,cd(),src(),u8(),u8()]),(0x40,lambda n:[n,cd(),src(),u8()]),
         (0x41,lambda n:[n,cd(),src(),src(),random.randrange(4),u8()]),(0x50,lambda n:[n,cd(),src(),src(),u8(),u8()]),
         (0x51,lambda n:[n,cd(),src(),src(),src(),random.randrange(4)])]
    def cd():return random.choice([0x07,0x17,0x57,0x67,0x77,0x03])
    only=[int(a,16) for a in sys.argv[1:]]
    for code,gen in FIL:
        if only and code not in only:continue
        for i in range(12):
            n=random.randrange(1,5);prog=pre()+[code]+gen(n)+[n]
            o=run(prog)
            if o:out.append('%s %d'%(bytes(prog).hex(),fnv(o)))
            else:print('FAIL',code)
    open('prog_vectors.txt' if not only else 'prog_vectors_new.txt','w').write('\n'.join(out)+'\n');print(len(out))
