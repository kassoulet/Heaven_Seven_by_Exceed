#!/usr/bin/env python3
"""Static recompiler for the 3D rasteriser (class 13 draw 0x40a680, from 0x401e46 on).
Input: objdump -D -M intel disassembly of the unpacked image (img.dis) + the set of instruction addresses executed by the original (exe_all.pkl, from re/exe3d.py).
Output: h7core/src/raster_gen.rs = one `match pc` arm per basic block; every instruction becomes the Rust statement(s) with the same semantics (u32 registers, eager flags,
MMX as u64 via x86rt, x87 via the f32-rounding X87 stack). Addresses that the original never executed in the recorded frames are NOT translated (panic if reached)."""
import re,sys,pickle
DIS,EXE,OUT=sys.argv[1:4]
MODE=sys.argv[4] if len(sys.argv)>4 else 'raster'   # 'load' = the start-up / texture build code (win32 shim, string ops, more x87)
S=pickle.load(open(EXE,'rb'))
LEAD=set()
if isinstance(S,dict): LEAD=set(S['lead']); S=set(S['S'])   # load mode: also every block entry the emulator saw (indirect call/jump targets in the middle of other blocks)
I={}
for l in open(DIS):
    m=re.match(r'\s*([0-9a-f]+):\t([0-9a-f ]+)\t(.*)',l)
    if m:
        a=int(m.group(1),16); last=a; I[a]=(len(m.group(2).split()),re.sub(r'\s+',' ',m.group(3).strip()))
    else:   # continuation line of a long instruction (objdump wraps after 7 bytes)
        m=re.match(r'\s*([0-9a-f]+):\t([0-9a-f ]+)\s*$',l)
        if m: I[last]=(I[last][0]+len(m.group(2).split()),I[last][1])
R32=['eax','ecx','edx','ebx','esp','ebp','esi','edi']
R16=['ax','cx','dx','bx','sp','bp','si','di']
R8=['al','cl','dl','bl','ah','ch','dh','bh']
def num(s): return int(s,16) if s.startswith(('0x','-0x')) else int(s)
def parse(o):
    o=o.strip()
    if o in R32: return ('r',R32.index(o),32)
    if o in R16: return ('r',R16.index(o),16)
    if o in R8: return ('r8',R8.index(o),8)
    if re.fullmatch(r'mm\d',o): return ('mm',int(o[2]))
    m=re.fullmatch(r'st\((\d)\)',o)
    if m: return ('st',int(m.group(1)))
    if o=='st': return ('st',0)
    if re.fullmatch(r'-?0x[0-9a-f]+|-?\d+',o): return ('i',num(o))
    m=re.fullmatch(r'(?:(DWORD|WORD|BYTE|QWORD) PTR )?(?:[dc]s:)?(.*)',o)
    sz={'DWORD':32,'WORD':16,'BYTE':8,'QWORD':64,None:None}[m.group(1)]
    body=m.group(2)
    mm=re.fullmatch(r'\[(.*)\]',body)
    if mm: body=mm.group(1)
    elif re.fullmatch(r'0x[0-9a-f]+',body): return ('m',None,None,1,num(body),sz)
    else: raise Exception('operand '+o)
    base=idx=None;sc=1;disp=0
    for t in re.findall(r'[+-]?[^+-]+',body):
        sg=-1 if t[0]=='-' else 1; t=t.lstrip('+-')
        if '*' in t:
            r,s=t.split('*'); idx=R32.index(r); sc=int(s)
        elif t in R32:
            if base is None: base=R32.index(t)
            else: idx=R32.index(t)
        else: disp+=sg*num(t)
    return ('m',base,idx,sc,disp,sz)
def addr(o):
    _,b,i,s,d,_z=o
    e=[]
    if b is not None: e.append('c.r[%d]'%b)
    if i is not None: e.append('c.r[%d]%s'%(i,'' if s==1 else '.wrapping_mul(%d)'%s))
    t=e[0] if e else '0u32'
    for x in e[1:]: t='%s.wrapping_add(%s)'%(t,x)
    if d: t='%s.wrapping_add(%#x)'%(t,d&0xffffffff)
    return t
def rd(o,w=None):
    k=o[0]
    if k=='r':
        return 'c.r[%d]'%o[1] if o[2]==32 else '(c.r[%d]&0xffff)'%o[1]
    if k=='r8': return '(c.r[%d]&0xff)'%o[1] if o[1]<4 else '((c.r[%d]>>8)&0xff)'%(o[1]-4)
    if k=='i': return '%#x'%(o[1]&0xffffffff if (w or 32)==32 else o[1]&((1<<w)-1))
    if k=='m':
        a=addr(o); w=o[5] or w
        return {8:'(m.r8(%s) as u32)',16:'(m.r16(%s) as u32)',32:'m.r32(%s)'}[w]%a
    raise Exception(o)
def wr(o,v,w=None):
    k=o[0]
    if k=='r':
        return 'c.r[%d]=%s;'%(o[1],v) if o[2]==32 else 'c.r[%d]=(c.r[%d]&0xffff0000)|(%s&0xffff);'%(o[1],o[1],v)
    if k=='r8':
        if o[1]<4: return 'c.r[%d]=(c.r[%d]&0xffffff00)|(%s&0xff);'%(o[1],o[1],v)
        return 'c.r[%d]=(c.r[%d]&0xffff00ff)|((%s&0xff)<<8);'%(o[1]-4,o[1]-4,v)
    if k=='m':
        a=addr(o); w=o[5] or w
        return {8:'m.w8(%s,(%s) as u8);',16:'m.w16(%s,(%s) as u16);',32:'m.w32(%s,%s);'}[w]%(a,v)
    raise Exception(o)
def width(*ops):
    for o in ops:
        if o[0]=='r': return o[2]
        if o[0]=='r8': return 8
        if o[0]=='m' and o[5]: return o[5]
    return 32
def rd64(o):
    if o[0]=='mm': return 'c.mm[%d]'%o[1]
    a=addr(o)
    return 'm.r64(%s)'%a
CC={'je':'c.zf','jne':'!c.zf','jb':'c.cf','jae':'!c.cf','js':'c.sf','jns':'!c.sf','jl':'(c.sf!=c.of)','jge':'(c.sf==c.of)','jg':'(!c.zf&&c.sf==c.of)','jle':'(c.zf||c.sf!=c.of)','ja':'(!c.cf&&!c.zf)','jbe':'(c.cf||c.zf)','jo':'c.of','jno':'!c.of'}
ALU={'add':'ADD','sub':'SUB','cmp':'SUB','and':'AND','or':'OR','xor':'XOR','test':'AND','adc':'ADC','sbb':'SBB'}
MMX2={'paddw':'paddw','paddd':'paddd','paddsw':'paddsw','paddusw':'paddusw','psubw':'psubw','psubd':'psubd','psubsw':'psubsw','pmulhw':'pmulhw','pmullw':'pmullw','pcmpgtw':'pcmpgtw',
 'pand':'pand','por':'por','pxor':'pxor','punpcklbw':'punpcklbw','punpckhbw':'punpckhbw','punpcklwd':'punpcklwd','punpckhwd':'punpckhwd','punpckldq':'punpckldq','punpckhdq':'punpckhdq',
 'packuswb':'packuswb','packsswb':'packsswb','packssdw':'packssdw','paddb':'paddb','psubb':'psubb','paddsb':'paddsb','paddusb':'paddusb','psubusb':'psubusb','psubusw':'psubusw','pcmpgtd':'pcmpgtd','pcmpgtb':'pcmpgtb','pcmpeqw':'pcmpeqw','pcmpeqd':'pcmpeqd','pcmpeqb':'pcmpeqb','pmaddwd':'pmaddwd','pandn':'pandn'}
MSH={'psllw','pslld','psllq','psrlw','psrld','psrlq','psraw','psrad'}
FOP={'add':'Add','sub':'Sub','subr':'SubR','mul':'Mul','div':'Div','divr':'DivR'}
def tr(a):
    ln,txt=I[a]; nxt=a+ln
    mn,_,rest=txt.partition(' ')
    sm=re.fullmatch(r'(rep )?(stos|movs|lods) (.*)',txt)
    if sm:
        rep,kind,body=sm.groups(); w=32 if 'DWORD' in body else 16 if 'WORD' in body else 8
        cast='' if w==32 else ' as u%d'%w
        if kind=='stos': reg={32:'c.r[0]',16:'(c.r[0]&0xffff)',8:'(c.r[0]&0xff)'}[w]; st='m.w%d(c.r[7],%s%s);c.r[7]=c.r[7].wrapping_add(%d);'%(w,reg,cast,w//8)
        elif kind=='movs': st='m.w%d(c.r[7],m.r%d(c.r[6])%s);c.r[7]=c.r[7].wrapping_add(%d);c.r[6]=c.r[6].wrapping_add(%d);'%(w,w,cast,w//8,w//8)
        else:
            assert not rep
            st={32:'c.r[0]=m.r32(c.r[6]);',16:'c.r[0]=(c.r[0]&0xffff0000)|m.r16(c.r[6]) as u32;',8:'c.r[0]=(c.r[0]&0xffffff00)|m.r8(c.r[6]) as u32;'}[w]+'c.r[6]=c.r[6].wrapping_add(%d);'%(w//8)
        return ['while c.r[1]!=0 {'+st+'c.r[1]-=1;}'] if rep else [st]
    if mn=='mov' and rest.endswith(',ds'): return [wr(parse(rest[:-3]),'0',16)]   # Unicorn's flat ds selector reads 0
    ops=[parse(x) for x in re.split(r',(?![^\[]*\])',rest)] if rest else []
    o=ops
    if mn=='mov': w=width(*o); return [wr(o[0],rd(o[1],w),w)]
    if mn=='movzx': return [wr(o[0],rd(o[1]))]
    if mn=='lea': return [wr(o[0],addr(o[1]))]
    if mn in ALU:
        w=width(*o); mask={8:'0xff',16:'0xffff',32:'0xffffffff'}[w]
        s='c.alu(%s,%s,%s,%d)'%(ALU[mn],rd(o[0],w),rd(o[1],w),w)
        return ['let r=%s;'%s]+([] if mn in('cmp','test') else [wr(o[0],'r',w)])
    if mn in('inc','dec'): w=width(*o); return ['let r=c.%s(%s,%d);'%(mn,rd(o[0],w),w),wr(o[0],'r',w)]
    if mn in('shl','shr','sar'):
        w=width(*o); n=rd(o[1],8) if o[1][0]=='i' else '(c.r[1]&0xff)'
        return ['let r=c.%s(%s,%s,%d);'%(mn,rd(o[0],w),n,w),wr(o[0],'r',w)]
    if mn=='shld': return ['let r=c.shld(%s,%s,%s);'%(rd(o[0]),rd(o[1]),rd(o[2],8)),wr(o[0],'r')]
    if mn=='mul' and width(*o)==32: return ['c.mul(%s);'%rd(o[0])]
    if mn in('mul','imul','div','idiv') and len(o)==1: return ['c.md(%d,%s,%d);'%(['mul','imul','div','idiv'].index(mn),rd(o[0]),width(*o))]
    if mn=='imul':
        if len(o)==3: return ['let r=c.imul(%s,%s);'%(rd(o[1]),rd(o[2])),wr(o[0],'r')]
        return ['let r=c.imul(%s,%s);'%(rd(o[0]),rd(o[1])),wr(o[0],'r')]
    if mn=='neg': return ['let r=c.alu(SUB,0,%s,32);'%rd(o[0]),wr(o[0],'r')]
    if mn=='not': return [wr(o[0],'!%s'%rd(o[0]))]
    if mn=='clc': return ['c.cf=false;']
    if mn=='stc': return ['c.cf=true;']
    if mn=='push': return ['c.push(m,%s);'%rd(o[0])]
    if mn=='pop': return ['let v=c.pop(m);',wr(o[0],'v')]
    if mn=='pusha': return ['c.pusha(m);']
    if mn=='popa': return ['c.popa(m);']
    if mn=='emms': return []
    if mn=='ret': return ['pc=c.pop(m);'+('c.r[4]=c.r[4].wrapping_add(%d);'%o[0][1] if o else '')+'continue;']
    if mn=='call':
        t=o[0]
        if t[0]=='i': return ['c.push(m,%#x);pc=%#x;continue;'%(nxt,t[1])]
        return ['let t=%s;c.push(m,%#x);pc=t;continue;'%(rd(t,32),nxt)]
    if mn=='jmp':
        if o[0][0]=='i': return ['pc=%#x;continue;'%o[0][1]]
        return ['pc=%s;continue;'%rd(o[0],32)]
    if mn in CC: return ['if %s {pc=%#x;continue;}'%(CC[mn],o[0][1])]
    # MMX
    if mn=='movq':
        d,s=o
        if d[0]=='mm': return ['c.mm[%d]=%s;'%(d[1],rd64(s))]
        return ['m.w64(%s,c.mm[%d]);'%(addr(d),s[1])]
    if mn=='movd':
        d,s=o
        if d[0]=='mm': return ['c.mm[%d]=%s as u64;'%(d[1],rd(s,32))]
        return [wr(d,'c.mm[%d] as u32'%s[1])]
    if mn in MMX2:
        d,s=o
        if mn.startswith('punpckl') and s[0]=='m': sv='(m.r32(%s) as u64)'%addr(s)
        else: sv=rd64(s)
        return ['c.mm[%d]=x86rt::%s(c.mm[%d],%s);'%(d[1],MMX2[mn],d[1],sv)]
    if mn in MSH: return ['c.mm[%d]=x86rt::%s(c.mm[%d],%d);'%(o[0][1],mn,o[0][1],o[1][1])]
    # x87
    if mn=='fld1': return ['c.x.fld1();']
    if mn=='fldz': return ['c.x.push(0.0);']
    if mn=='fld':
        if o[0][0]=='st': return ['c.x.fldst(%d);'%o[0][1]]
        if o[0][5]==64: return ['c.x.push64(f64::from_bits(m.r64(%s)));'%addr(o[0])]
        return ['c.x.push(m.rf(%s));'%addr(o[0])]
    if mn=='fxch': return ['c.x.fxch(%d);'%(o[0][1] if o else 1)]
    if mn=='fchs': return ['c.x.fchs();']
    if mn=='fabs': return ['c.x.fabs();']
    if mn=='fsqrt': return ['c.x.sqrt();']
    if mn=='fpatan': return ['c.x.patan();']
    if mn=='fptan': return ['c.x.ptan();']
    if mn=='fild':
        if o[0][5]==16: return ['c.x.fild(m.r16(%s) as i16 as i32);'%addr(o[0])]
        return ['c.x.fild(m.r32(%s) as i32);'%addr(o[0])]
    if mn=='fldpi': return ['c.x.push64(std::f64::consts::PI);']
    if mn=='fsin': return ['c.x.sin();']
    if mn=='fsincos': return ['c.x.sincos();']
    if mn in('finit','fninit'): return ['c.x = X87::new();']
    if mn in('fwait','wait'): return []
    if mn=='fldcw': return []     # the precision control is modelled as always 24 bit (set by 0x4099bd before any x87 arithmetic)
    if mn=='fsave': return ['c.fsave();']
    if mn=='frstor': return ['c.frstor();']
    if mn in('fcom','fcomp'):
        if o[0][0]=='st': v='c.x.st64(%d)'%o[0][1]
        elif o[0][5]==64: v='f64::from_bits(m.r64(%s))'%addr(o[0])
        else: v='m.rf(%s) as f64'%addr(o[0])
        return ['c.fcom(%s);'%v]+(['c.x.pop64();'] if mn=='fcomp' else [])
    if mn=='fnstsw': return ['c.r[0]=(c.r[0]&0xffff0000)|c.fsw as u32;']
    if mn=='sahf': return ['c.sahf();']
    if mn=='fist':
        if o[0][5]==16: return ['m.w16(%s,x86rt::fist16(c.x.st64(0)));'%addr(o[0])]
        return ['m.w32(%s,fistp64(c.x.st64(0)) as u32);'%addr(o[0])]
    mi=re.fullmatch(r'fi(add|sub|subr|mul|div|divr)',mn)
    if mi:
        v='(m.r16(%s) as i16 as f64)'%addr(o[0]) if o[0][5]==16 else '(m.r32(%s) as i32 as f64)'%addr(o[0])
        return ['c.x.arith_v(Op::%s,%s);'%(FOP[mi.group(1)],v)]
    if mn in('fstp','fst'):
        if o[0][0]=='st':
            assert mn=='fstp' and o[0][1]==0; return ['c.x.pop();']
        if o[0][5]==64: return ['m.w64(%s,c.x.%s().to_bits());'%(addr(o[0]),'pop64' if mn=='fstp' else 'top64')]
        return ['m.wf(%s,c.x.%s());'%(addr(o[0]),'pop' if mn=='fstp' else 'top')]
    if mn=='fistp':
        if o[0][5]==16: return ['m.w16(%s,x86rt::fist16(c.x.pop64()));'%addr(o[0])]
        return ['m.w32(%s,fistp64(c.x.pop64()) as u32);'%addr(o[0])]
    mm=re.fullmatch(r'f(add|sub|subr|mul|div|divr)(p?)',mn)
    if mm:
        op=FOP[mm.group(1)]
        if len(o)==1 and o[0][5]==64: return ['c.x.arith_v(Op::%s,f64::from_bits(m.r64(%s)));'%(op,addr(o[0]))]
        if len(o)==1: return ['c.x.arith_m(Op::%s,m.rf(%s));'%(op,addr(o[0]))]
        return ['c.x.arith(Op::%s,%d,%d,%s);'%(op,o[0][1],o[1][1],'true' if mm.group(2) else 'false')]
    if mn=='xchg':
        w=width(*o); return ['let (x,y)=(%s,%s);'%(rd(o[0],w),rd(o[1],w)),wr(o[0],'y',w),wr(o[1],'x',w)]
    if mn=='movsx':
        w=o[1][5] if o[1][0]=='m' else (8 if o[1][0]=='r8' else 16)
        return [wr(o[0],'(%s as %s as i32 as u32)'%(rd(o[1]),'u8 as i8' if w==8 else 'u16 as i16'))]
    if mn=='cdq': return ['c.r[2]=((c.r[0] as i32)>>31) as u32;']
    if mn=='cwde': return ['c.r[0]=(c.r[0] as u16 as i16 as i32) as u32;']
    if mn=='cld': return []
    if mn=='bswap': return ['c.r[%d]=c.r[%d].swap_bytes();'%(o[0][1],o[0][1])]
    if mn in('rol','ror','rcl','rcr'):
        w=width(*o); n=rd(o[1],8) if o[1][0]=='i' else '(c.r[1]&0xff)'
        return ['let r=c.%s(%s,%s,%d);'%(mn,rd(o[0],w),n,w),wr(o[0],'r',w)]
    if mn=='shrd': return ['let r=c.shrd(%s,%s,%s);'%(rd(o[0]),rd(o[1]),rd(o[2],8) if o[2][0]=='i' else '(c.r[1]&0xff)'),wr(o[0],'r')]
    if mn=='pushf': return ['c.pushf(m);']
    if mn=='popf': return ['c.popf(m);']
    if mn=='cpuid': return ['c.cpuid();']
    if mn=='nop': return []
    if mn in('loop','jecxz','jcxz'):
        if mn=='loop': return ['c.r[1]=c.r[1].wrapping_sub(1); if c.r[1]!=0 {pc=%#x;continue;}'%o[0][1]]
        return ['if %s==0 {pc=%#x;continue;}'%('c.r[1]' if mn=='jecxz' else '(c.r[1]&0xffff)',o[0][1])]
    raise Exception('unsupported %x %s'%(a,txt))
addrs=sorted(S)
bad={}
for a in addrs:
    try: tr(a)
    except Exception as e: bad.setdefault(I[a][1].split(' ')[0]+' '+str(e)[:50],[]).append(a)
if bad:
    for k,v in bad.items(): print(k,len(v),hex(v[0]),I[v[0]][1])
    if MODE!='full': sys.exit(1)      # full mode: unsupported instructions become panic stubs (only reached if executed)
tgt=set(); 
isterm=lambda t:t.split(' ')[0] in('ret','jmp')
for a in addrs:
    mn,_,rest=I[a][1].partition(' ')
    if mn in CC or mn in('jmp','call','loop','jecxz','jcxz'):
        if rest.startswith('0x'): tgt.add(int(rest,16))
leaders=set()
for a in addrs:
    ln=I[a][0]
    prev=None
    # a is a leader if the instruction physically before it is not an executed non-terminator, or it is a branch target
for a in addrs:
    p=[b for b in (a-k for k in range(1,16)) if b in I and b in S and b+I[b][0]==a]
    if a in tgt or a in LEAD or not p or I[p[0]][1].split(' ')[0] in('ret','jmp') or I[p[0]][1].split(' ')[0]=='call': leaders.add(a)
out=['// GENERATED by re/recomp.py from the original image (do not edit). One arm per basic block; see the module doc of recomp.py.','#![allow(unused_variables, unused_mut, unreachable_code, clippy::all)]',
'use crate::scene::{Mem, Op, X87, fistp64};','use crate::x86rt::{self, Cpu, ADD, SUB, AND, OR, XOR, ADC, SBB};','','pub fn run(c: &mut Cpu, m: &mut Mem, mut pc: u32) {','    loop {','        if pc == 0xdead0000 { return; }']+(['        if c.stop_pc == pc { c.stop_n -= 1; if c.stop_n == 0 { return; } }'] if MODE in('load','full') else [])+['        if let Some(t) = &mut c.trace { t.push(pc); }','        match pc {']
for a in addrs:
    if a not in leaders: continue
    out.append('            %#x => {'%a)
    b=a
    while True:
        out.append('                // %x %s'%(b,I[b][1]))
        try: stm=tr(b)
        except Exception as e:
            out.append('                panic!("recompiler: unsupported instruction at %x: %s");'%(b,I[b][1].replace('"',"'"))); break
        for s in stm: out.append('                '+s)
        n=b+I[b][0]
        mn=I[b][1].split(' ')[0]
        if mn in('ret','jmp') or (mn=='call'):
            break
        if n in leaders or n not in S:
            out.append('                pc = %#x; continue;'%n); break
        b=n
    out.append('            }')
if MODE in('load','full'): out.append('            _ if pc >= 0x7f000000 => { pc = crate::win32::call(c, m, pc); continue; }')
out.append('            _ => panic!("recompiled code reached {:#x}, which the original never executed in the recorded run", pc),')
out+=['        }','    }','}']
open(OUT,'w').write('\n'.join(out)+'\n')
print(len(leaders),'blocks',len(addrs),'instrs')
