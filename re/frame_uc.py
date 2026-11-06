#!/usr/bin/env python3
"""frame_uc.py <snapshot.bin> <t> <fb_buf_hex> <out.bin> [writes.pkl]: run ONE full frame (evaluator 0x409a50 incl. draws + present) of the original in Unicorn from a snapshot (image ++ heap), with the h7emu.py Win32/COM fakes
(the COM objects' tags are rebuilt from the context: dd = [0x20001158], surfaces = [0x2000115c], [0x20001160]; fb_buf = the Lock-allocated 320x240 buffer). Dumps image ++ heap afterwards; the Rust twin is cold::tests::diff3d (H7MODE=frame)."""
import sys,os
a=sys.argv[1:];sys.argv=[sys.argv[0]]
src=open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'h7emu.py')).read()
src=src[:src.index("W(UC_X86_REG_ESP,STK-0x100)")]
os.environ.setdefault('H7SND','2');os.chdir(os.path.dirname(os.path.abspath(__file__)))
exec(src)
raw=open(a[0],'rb').read();t=int(a[1]);fbb=int(a[2],16)
IMG=len(img);mu.mem_write(BASE,raw[:IMG]);mu.mem_write(0x20000000,raw[IMG:]);heap[0]=0x20000000+len(raw)-IMG
dd=rd(0x20001158);com[dd]='dd'
for p in (0x2000115c,0x20001160):com[rd(p)]='surf'
FB['buf']=fbb
mu.mem_map(0xdead0000,0x1000);mu.mem_write(0xdead0000,b'\xf4')
W(UC_X86_REG_ESP,STK-0x100);mu.mem_write(STK-0x100,struct.pack('<I',0xdead0000))
W(UC_X86_REG_EBX,t);mu.reg_write(UC_X86_REG_FPCW,0x7f)
if len(a)>4:
    WR=[];mu.hook_add(UC_HOOK_MEM_WRITE,lambda uc,ac,ad,sz,v,_:WR.append((ad,sz,v&((1<<(8*sz))-1))),begin=0x400000,end=0x30000000)
mu.emu_start(0x409a50,0xdead0000,count=400000000)
open(a[3],'wb').write(bytes(mu.mem_read(BASE,IMG))+bytes(mu.mem_read(0x20000000,heap[0]-0x20000000 if False else len(raw)-IMG)))
if len(a)>4:
    import pickle;pickle.dump(WR,open(a[4],'wb'))
print('frames presented',len(FB['frames']))
