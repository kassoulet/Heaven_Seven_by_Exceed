# usage: runto.py k t addr1[,addr2..]  -> dumps full memory at the first reach of each addr as /tmp/claude-1000/w/s3d_<k>_<addr>.bin ; 'end' = return of 0x40a680
import struct,sys
k=int(sys.argv[1]);t=int(sys.argv[2]);stops=[int(x,16) if x!='end' else 0xdead0000 for x in sys.argv[3].split(',')]
src=open('/tmp/claude-1000/w/prof3d.py').read().split("C=collections.Counter()")[0].replace("sys.argv[1]","'%d'"%k).replace("int(sys.argv[2]) if len(sys.argv)>2 else 0",str(t))
exec(src)
mu.reg_write(UC_X86_REG_FPCW,0x7f)
done=set()
def dump(a):
    o='/tmp/claude-1000/w/s3d_%d_%x.bin'%(k,a)
    open(o,'wb').write(bytes(mu.mem_read(0x400000,IMG))+bytes(mu.mem_read(0x20000000,len(heap))));print('dumped',o)
def hk(uc,a,sz,_):
    if a in stops and a not in done and a!=0xdead0000: done.add(a);dump(a)
mu.hook_add(UC_HOOK_CODE,hk,begin=0x401000,end=0x41d000)
try: mu.emu_start(0x40a680,0xdead0000,count=400000000)
except Exception as e: print('exc',e)
if 0xdead0000 in stops: dump(0xdead0000)
