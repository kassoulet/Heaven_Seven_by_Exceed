import struct,sys,collections
k=int(sys.argv[1]) if len(sys.argv)>1 else 1
src=open('/tmp/claude-1000/w/prof3d.py').read().split("C=collections.Counter()")[0].replace("sys.argv[1]","'%d'"%k)
exec(src)
code=img
stack=[]  # return addresses
ev=[]
def hk(uc,a,sz,_):
    while stack and stack[-1][0]==a: stack.pop()
    b=bytes(uc.mem_read(a,7))
    tgt=None
    if b[0]==0xe8: tgt=(a+5+struct.unpack('<i',b[1:5])[0])&0xffffffff; ln=5
    elif b[0]==0xff and (b[1]>>3)&7==2:
        # indirect call: evaluate via regs
        mod=b[1]>>6; rm=b[1]&7
        R=[UC_X86_REG_EAX,UC_X86_REG_ECX,UC_X86_REG_EDX,UC_X86_REG_EBX,UC_X86_REG_ESP,UC_X86_REG_EBP,UC_X86_REG_ESI,UC_X86_REG_EDI]
        if mod==3: tgt=uc.reg_read(R[rm]); ln=2
        elif mod==1 and rm!=4: d=struct.unpack('b',b[2:3])[0]; tgt=r32(uc.reg_read(R[rm])+d); ln=3
        elif mod==2 and rm!=4: d=struct.unpack('<i',b[2:6])[0]; tgt=r32((uc.reg_read(R[rm])+d)&0xffffffff); ln=6
        elif mod==0 and rm==5: tgt=r32(struct.unpack('<I',b[2:6])[0]); ln=6
        elif mod==0 and rm!=4: tgt=r32(uc.reg_read(R[rm])); ln=2
        else: tgt=-1; ln=3
    if tgt is not None:
        ev.append((len(stack),a,tgt,uc.reg_read(UC_X86_REG_ESI),uc.reg_read(UC_X86_REG_EBX)))
        stack.append((a+ln,))
mu.hook_add(UC_HOOK_CODE,hk,begin=0x401000,end=0x41d000)
mu.emu_start(0x40a680,0xdead0000,count=400000000)
import pickle;pickle.dump(ev,open('ev%d.pkl'%k,'wb'))
md=int(sys.argv[2]) if len(sys.argv)>2 else 4
last=None;rep=0
for d,a,t,esi,ebx in ev:
    if d<=md: print('  '*d+'%x -> %x esi=%x ebx=%x'%(a,t,esi,ebx))
