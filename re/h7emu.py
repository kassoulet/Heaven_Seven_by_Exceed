#!/usr/bin/env python3
"""h7emu.py - run the unpacked HEAVEN7W.EXE (image.bin) in Unicorn with stubbed Win32/DirectX; logs API calls.
usage: h7emu.py [max_instructions]"""
import sys,struct,json
from unicorn import *; from unicorn.x86_const import *
BASE=0x400000;info=json.load(open('info.json'));img=open('image.bin','rb').read()
NAMES={int(k,16):v for k,v in info['imports'].items()}
mu=Uc(UC_ARCH_X86,UC_MODE_32);mu.mem_map(BASE,(len(img)+0xfff)&~0xfff);mu.mem_write(BASE,img)
STK=0x1000000;mu.mem_map(STK-0x400000,0x400000);mu.mem_map(0x7f000000,0x100000);mu.mem_map(0x20000000,0x10000000)  # heap
R=lambda r:mu.reg_read(r);W=lambda r,v:mu.reg_write(r,v&0xffffffff)
def rd(a,n=4):return struct.unpack('<I',bytes(mu.mem_read(a,4)))[0] if n==4 else bytes(mu.mem_read(a,n))
def cs(a):
    b=b''
    while a:
        c=bytes(mu.mem_read(a,1))
        if c==b'\0':break
        b+=c;a+=1
    return b.decode('latin1')
heap=[0x20000000]
def alloc(n):
    a=heap[0];heap[0]+=(n+0xfff)&~0xf;return a
NARGS={'EndDialog':2,'CloseHandle':1,'CreateThread':6,'DeleteCriticalSection':1,'EnterCriticalSection':1,'LeaveCriticalSection':1,'InitializeCriticalSection':1,'GlobalFree':1,'SetThreadPriority':2,'Sleep':1,'TerminateThread':2,'ClientToScreen':2,'CreateWindowExA':12,'DefWindowProcA':4,'DestroyWindow':1,'DispatchMessageA':1,'GetClientRect':2,'MessageBoxA':4,'OffsetRect':3,'RegisterClassA':1,'SetCursor':1,'ShowWindow':2,'SystemParametersInfoA':4,'ord1':3,'DirectDrawCreate':3}
log=[];t=[0];DLG={'res':0,'snd':int(__import__('os').environ.get('H7SND','2')),'checks':()}
VT=[0];STEP=int(__import__('os').environ.get('H7STEP','40'))
def ret(n,v=0):   # stdcall return, pop n args
    sp=R(UC_X86_REG_ESP);r=rd(sp);W(UC_X86_REG_EAX,v);W(UC_X86_REG_EIP,r);W(UC_X86_REG_ESP,sp+4+n*4)
# COM objects: vtable of fake method addresses 0x7f080000+idx*16, per-interface tag
com={}
def mkcom(tag,nm=64):
    vt=alloc(nm*4);obj=alloc(16);mu.mem_write(obj,struct.pack('<I',vt))
    for i in range(nm):mu.mem_write(vt+i*4,struct.pack('<I',0x7f080000+i*16))
    com[obj]=tag;return obj
DDN=[2,0,0,0,3,4,3,2,4,4,0,2,1,2,1,1,1,1,1,0,2,3]
SFN=[2,0,0,1,1,5,3,5,2,2,3,2,2,1,1,1,2,1,1,2,1,1,1,2,0,4,1,0,1,2,2,1,1,5,2,2,2,2,2,2]
FB={'buf':None,'w':320,'h':240,'frames':[]}
def api(uc,addr,size,_):
    sp=R(UC_X86_REG_ESP);a=[rd(sp+4+4*i) for i in range(6)]
    if 0x7f080000<=addr<0x7f090000:
        idx=(addr-0x7f080000)//16;this=a[0];tag=com.get(this,'?')
        n=(DDN if tag=='dd' else SFN)[idx] if idx<(len(DDN) if tag=='dd' else len(SFN)) else 0
        print('COM',tag,idx,[hex(x) for x in a[1:1+n]])
        v=0
        if tag=='dd' and idx==6:
            s=mkcom('surf');mu.mem_write(a[2],struct.pack('<I',s));com[s]='surf'
        elif tag=='surf' and idx==22:
            d0=a[1];mu.mem_write(d0+4,struct.pack('<I',0x100f));mu.mem_write(d0+8,struct.pack('<II',FB['h'],FB['w']));mu.mem_write(d0+72,struct.pack('<IIIIIII',32,0x40,0,32,0xff0000,0xff00,0xff))
        elif tag=='surf' and idx==12:
            s=mkcom('surf');mu.mem_write(a[2],struct.pack('<I',s))
        elif tag=='surf' and idx==25:   # Lock(rect,&desc,flags,0)
            if FB['buf'] is None:FB['buf']=alloc(FB['w']*FB['h']*4+4096)
            mu.mem_write(a[2]+4,struct.pack('<I',0x100f))     # flags
            mu.mem_write(a[2]+8,struct.pack('<II',FB['h'],FB['w']));mu.mem_write(a[2]+16,struct.pack('<I',FB['w']*4));mu.mem_write(a[2]+36,struct.pack('<I',FB['buf']));mu.mem_write(a[2]+72,struct.pack('<IIIIIII',32,0x40,0,32,0xff0000,0xff00,0xff))
        elif tag=='surf' and idx==5 and a[2]!=0 and FB['buf'] is not None:   # Blt(rect,src,..): present
            FB['frames'].append((VT[0],bytes(mu.mem_read(FB['buf'],FB['w']*FB['h']*4))));VT[0]+=STEP
        ret(n+1,v);return
    n=NAMES.get(addr)
    if n is None:return
    print(n,[hex(x) for x in a[:4]])
    if n.endswith('DirectDrawCreate'):
        o=mkcom('dd');mu.mem_write(a[1],struct.pack('<I',o));ret(3,0)
    elif n.endswith('GlobalAlloc'):ret(2,alloc(a[1]))
    elif n.endswith('GetCommandLineA'):
        s=alloc(16);mu.mem_write(s,b'h7\0');ret(0,s)
    elif n.endswith('timeGetTime'):ret(0,VT[0])
    elif n.endswith('GetModuleHandleA'):ret(1,BASE)
    elif n.endswith('GetSystemMetrics'):ret(1,1024 if a[0]==0 else 768)
    elif n.endswith('DialogBoxIndirectParamA'):
        proc=a[3];ra=rd(sp)
        code=b'\x6a\x00\x6a\x00\x68\x10\x01\x00\x00\x6a\x01'+b'\xb8'+struct.pack('<I',proc)+b'\xff\xd0'
        code+=b'\x6a\x00\x6a\x01\x68\x11\x01\x00\x00\x6a\x01'+b'\xb8'+struct.pack('<I',proc)+b'\xff\xd0'
        code+=b'\xb8\x01\x00\x00\x00\xc2\x14\x00'
        mu.mem_write(0x7f001000,code);W(UC_X86_REG_EIP,0x7f001000)
    elif n.endswith('SendDlgItemMessageA'):
        item,msg=a[1],a[2];v={0x3eb:DLG.get('res',0),0x3ec:DLG.get('snd',0)}.get(item,1 if msg==0xf0 and item in DLG.get('checks',()) else 0);ret(5,v)
    elif n.endswith('PeekMessageA'):ret(5,0)
    elif n.endswith('ExitProcess'):print('exit',a[0]);uc.emu_stop()
    else:ret(NARGS.get(n.split('!')[1],0),1)
mu.hook_add(UC_HOOK_CODE,api,begin=0x7f000000,end=0x7f0fffff)
import collections
RING=collections.deque(maxlen=40)
import os
if os.environ.get('H7RING'):mu.hook_add(UC_HOOK_CODE,lambda uc,a,sz,_:RING.append(a),begin=BASE,end=BASE+0x30000)
if os.environ.get('H7PROF'):
    import collections
    PROF=collections.Counter()
    mu.hook_add(UC_HOOK_BLOCK,lambda uc,a,sz,_:PROF.update({a:sz}),begin=BASE,end=BASE+0x30000)
if os.environ.get('H7CALLS'):
    from capstone import Cs,CS_ARCH_X86,CS_MODE_32
    from capstone.x86 import X86_OP_REG,X86_OP_MEM
    import collections
    CT=collections.Counter()
    cs_=Cs(CS_ARCH_X86,CS_MODE_32);cs_.detail=True
    code_=img[0x1000:0x1d000];RM={}
    from unicorn.x86_const import UC_X86_REG_EAX
    def reg(r):return cs_.reg_name(r)
    regmap={'eax':UC_X86_REG_EAX,'ebx':UC_X86_REG_EBX,'ecx':UC_X86_REG_ECX,'edx':UC_X86_REG_EDX,'esi':UC_X86_REG_ESI,'edi':UC_X86_REG_EDI,'ebp':UC_X86_REG_EBP,'esp':UC_X86_REG_ESP}
    sites={}
    for i in cs_.disasm(code_,0x401000):
        if i.mnemonic=='call' and i.operands and i.operands[0].type in (X86_OP_REG,X86_OP_MEM):sites[i.address]=i
    def mk(i):
        o=i.operands[0]
        if o.type==X86_OP_REG:return lambda:R(regmap[reg(o.reg)])
        m=o.mem
        def f():
            a=m.disp
            if m.base:a+=R(regmap[reg(m.base)])
            if m.index:a+=R(regmap[reg(m.index)])*m.scale
            return rd(a&0xffffffff)
        return f
    ev={a:mk(i) for a,i in sites.items()}
    def hk(uc,a,sz,_):
        try:CT[(a,ev[a]())]+=1
        except Exception:pass
    for a in ev:mu.hook_add(UC_HOOK_CODE,hk,begin=a,end=a)
    print('indirect call sites',len(ev))
if os.environ.get('H7SCRIPT'):
    SCR=[];cur=[None]
    def pre(uc,a,sz,_):
        cur[0]=[R(UC_X86_REG_EDI),R(UC_X86_REG_EAX)]
    def post(uc,a,sz,_):
        if cur[0]:SCR.append((cur[0][0],cur[0][1],R(UC_X86_REG_EDI)));cur[0]=None
    mu.hook_add(UC_HOOK_CODE,pre,begin=0x409a35,end=0x409a35)
    mu.hook_add(UC_HOOK_CODE,post,begin=0x409a37,end=0x409a37)
if os.environ.get('H7FONT'):
    FEV=[]
    def fh(uc,a,sz,_):FEV.append((hex(a),R(UC_X86_REG_EAX)&0xffffffff,hex(R(UC_X86_REG_EDI))))
    for a in (0x408e8e,0x408e94,0x408eaa,0x408ebd):mu.hook_add(UC_HOOK_CODE,fh,begin=a,end=a)
if os.environ.get('H7RD'):
    RD=[]
    def rh(uc,a,sz,_):
        RD.append((a,rd(R(UC_X86_REG_ESP)),R(UC_X86_REG_EDI)))
    for a in (0x407fc0,0x407fd0):mu.hook_add(UC_HOOK_CODE,rh,begin=a,end=a)
if os.environ.get('H7OBJ'):
    OBJ={}
    def oh(uc,a,sz,_):
        pc=R(UC_X86_REG_EDI)
        if pc==0x41a400 and 'x' not in OBJ:
            esi=R(UC_X86_REG_ESI);OBJ['x']=1;OBJ['esi']=esi;OBJ['mem']=bytes(mu.mem_read(esi,0x200));OBJ['ebp']=bytes(mu.mem_read(rd(0x42a3f8),0x100))
            print('esi',hex(esi),'edi',hex(pc))
    mu.hook_add(UC_HOOK_CODE,oh,begin=0x409a37,end=0x409a37)
if os.environ.get('H7TEXT'):
    TX={'n':0}
    def surf(r):
        w,h,p,dp=rd(r+0x18),rd(r+0x1c),rd(r+0x20),rd(r+0x28);st=p>>2
        return {'w':w,'h':h,'pitch_raw':p,'data':bytes(mu.mem_read(dp,st*h+16)).hex() if 0<w<400 and 0<h<400 and dp else ''}
    def tin(uc,a,sz,_):
        if TX['n']>=1:return
        esi=R(UC_X86_REG_ESI);edi=R(UC_X86_REG_EDI)
        # layer = edi ; only the first call whose object has a glyph list
        g=[rd(esi+0x44+4*i) for i in range(40)];gl=[]
        for x in g:
            if x==0:break
            gl.append(x)
        if not gl or rd(esi+0x3c)<=1:return
        TX['n']+=1;TX['esi']=esi;TX['edi']=edi
        TX['obj']=bytes(mu.mem_read(esi,0x44+4*len(gl))).hex();TX['view']=bytes(mu.mem_read(edi,0x60)).hex();TX['before']=surf(edi);TX['gl']=gl
        TX['glyphs']={hex(x):[surf(x),surf(x+0x200),surf(x+0x400)] for x in set(gl)}
        TX['ebp18']=rd(rd(0x42a3f8)+0x18)
    def tout(uc,a,sz,_):
        if TX['n']==1 and 'after' not in TX and R(UC_X86_REG_ESP)>0:TX['after']=surf(TX['edi']);print('text captured',len(TX['gl']),'glyphs')
    mu.hook_add(UC_HOOK_CODE,tin,begin=0x409cda,end=0x409cda)
    mu.hook_add(UC_HOOK_CODE,tout,begin=0x409deb,end=0x409deb)
W(UC_X86_REG_ESP,STK-0x100)
# return address -> ExitProcess stub
mu.mem_write(STK-0x100,struct.pack('<I',0x7f000040))
try:mu.emu_start(info['oep'],0,count=int(sys.argv[1]) if len(sys.argv)>1 else 3000000)
except UcError as e:print('ERR',e,hex(R(UC_X86_REG_EIP)))
print('eip',hex(R(UC_X86_REG_EIP)));print([hex(x) for x in list(RING)[-16:]])

import numpy as np
from PIL import Image
print('frames',len(FB['frames']))
for i,(tm,f) in enumerate(FB['frames'][:400]):
    a=np.frombuffer(f,np.uint8).reshape(FB['h'],FB['w'],4)[:,:,[2,1,0]];Image.fromarray(a).save('/tmp/claude-1000/h7/f%03d_%06d.png'%(i,tm))

if os.environ.get('H7PROF'):
    json.dump({hex(k):v for k,v in PROF.items()},open('prof_%s.json'%os.environ['H7PROF'],'w'))
if os.environ.get('H7DUMP'):
    open('mem_%s.bin'%os.environ['H7DUMP'],'wb').write(bytes(mu.mem_read(BASE,len(img))))
    ebp=rd(0x42a3f8);print('ebp base',hex(ebp))
    open('ebp_%s.bin'%os.environ['H7DUMP'],'wb').write(bytes(mu.mem_read(ebp,0x400)))

if os.environ.get('H7CALLS'):
    json.dump([[hex(a),hex(t),n] for (a,t),n in CT.most_common()],open('calls_%s.json'%os.environ['H7CALLS'],'w'))

if os.environ.get('H7SCRIPT'):
    json.dump([[hex(a),hex(b),hex(c)] for a,b,c in SCR],open('script_trace.json','w'));print('script ops',len(SCR))
if os.environ.get('H7GLYPH'):
    ctx=rd(0x42a3f8);base=rd(ctx+0x4c);print('font base',hex(base),'pool',hex(rd(ctx+0x44)))
    import numpy as np
    from PIL import Image
    N=(rd(ctx+0x44)-base)//0x600;print('records',N)
    sheet=[]
    for i in range(min(N,120)):
        r=base+i*0x600;w,h,pitch,dp=rd(r+0x18),rd(r+0x1c),rd(r+0x20),rd(r+0x28)
        if i<5:print(i,hex(r),w,h,pitch,hex(dp),bytes(mu.mem_read(r,0x30)).hex())
        if not(0<w<200 and 0<h<200 and dp>0x1000):continue
        st=pitch>>2;buf=bytes(mu.mem_read(dp,st*h)) if st*h<100000 else b''
        if len(buf)<st*h:continue
        a=np.frombuffer(buf,np.uint8).reshape(h,st)[:,:w]
        sheet.append(a)
    print('glyphs',len(sheet))
    if sheet:
        W=max(a.shape[1] for a in sheet);H=max(a.shape[0] for a in sheet);cols=16;rows=(len(sheet)+cols-1)//cols
        img=np.zeros((rows*(H+2),cols*(W+2)),np.uint8)
        for k,a in enumerate(sheet):
            y,x=(k//cols)*(H+2),(k%cols)*(W+2);img[y:y+a.shape[0],x:x+a.shape[1]]=np.minimum(255,a.astype(int)*2)
        Image.fromarray(img).resize((img.shape[1]*3,img.shape[0]*3),Image.NEAREST).save('/tmp/claude-1000/h7/glyphs.png');print(img.shape)

if os.environ.get('H7FONT'):
    print(len(FEV));print(FEV[:60])

if os.environ.get('H7GLYPH2'):
    import numpy as np,json
    ctx=rd(0x42a3f8);base=rd(ctx+0x4c);out={}
    for code in range(32,124):
        r=base+code*0x600;w,h,pitch,dp=rd(r+0x18),rd(r+0x1c),rd(r+0x20),rd(r+0x28)
        if 0<w<300 and 0<h<300 and dp>0x1000:
            st=pitch>>2;out[code]=[w,h,st,bytes(mu.mem_read(dp,st*h)).hex()]
    json.dump(out,open('glyph_truth.json','w'));print(len(out),{k:v[:3] for k,v in list(out.items())[:6]})

if os.environ.get('H7RD'):
    json.dump([[hex(a),hex(b),hex(c)] for a,b,c in RD if 0x41903c<=c<0x42b000],open('rd_trace.json','w'));print('reads',len(RD))

if os.environ.get('H7OBJ') and 'mem' in OBJ:
    import struct
    m=OBJ['mem'];f=struct.unpack('<128I',m)
    for i in range(0,128,8):print(hex(i*4),' '.join('%08x'%x for x in f[i:i+8]))
    # follow pointers to channel nodes
    for i,v in enumerate(f):
        if 0x20000000<=v<0x30000000 and v!=OBJ['esi']:
            print('ptr at +%#x ->'%(i*4),hex(v),bytes(mu.mem_read(v,0x80)).hex(' ')[:200])

if os.environ.get('H7TEXT'):
    json.dump(TX,open('text_capture.json','w'));print({k:(v if not isinstance(v,(dict,list)) else '...') for k,v in TX.items()})
