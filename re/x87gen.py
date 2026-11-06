#!/usr/bin/env python3
# gen.py START END : translate x87 instructions to Rust X87 calls; other instrs emitted as // comments
import sys,re
s=int(sys.argv[1],16);e=int(sys.argv[2],16)
OPS={'add':'Add','sub':'Sub','subr':'SubR','mul':'Mul','div':'Div','divr':'DivR'}
def mem(o):
    m=re.search(r'\[(\w+)(?:([+-])(0x[0-9a-f]+))?\]',o)
    if m:
        r=m.group(1)
        if m.group(3): return '%s.wrapping_%s(%s)'%(r,'add' if m.group(2)=='+' else 'sub',m.group(3))
        return r
    m=re.search(r'ds:(0x[0-9a-f]+)',o)
    if m: return m.group(1)
    return None
def sti(o):
    m=re.match(r'st\((\d)\)$',o); 
    if m: return int(m.group(1))
    if o=='st': return 0
    return None
for l in open('/tmp/claude-1000/w/img.dis'):
    m=re.match(r'\s*([0-9a-f]+):\t([0-9a-f ]+)\t(.*)',l)
    if not m: continue
    a=int(m.group(1),16)
    if not s<=a<=e: continue
    ins=m.group(3).strip(); mn,_,rest=ins.partition(' '); ops=[x.strip() for x in rest.split(',')] if rest else []
    c='// %x %s'%(a,ins); out=None
    if mn=='fld':
        if sti(ops[0]) is not None and 'PTR' not in ops[0]: out='x.fldst(%d);'%sti(ops[0])
        else: out='x.push(m.rf(%s));'%mem(ops[0])
    elif mn=='fld1': out='x.fld1();'
    elif mn=='fldz': out='x.push(0.0);'
    elif mn=='fild': out='x.fild(m.r32(%s) as i32);'%mem(ops[0])
    elif mn=='fxch': out='x.fxch(%d);'%(sti(ops[0]) if ops else 1)
    elif mn=='fptan': out='x.ptan();'
    elif mn=='fsincos': out='x.sincos();'
    elif mn=='fsin': out='x.sin();'
    elif mn=='fpatan': out='x.patan();'
    elif mn in('fidiv','fimul','fiadd','fisub') : out='x.arith_m(Op::%s, m.r32(%s) as i32 as f32);'%(OPS[mn[2:]],mem(ops[0]))
    elif mn=='fchs': out='x.fchs();'
    elif mn=='fabs': out='x.fabs();'
    elif mn=='fsqrt': out='x.sqrt();'
    elif mn in('fstp','fst'):
        if 'PTR' in ops[0]: out='m.wf(%s, x.%s());'%(mem(ops[0]),'pop' if mn=='fstp' else 'top')
        elif mn=='fstp' and sti(ops[0])==0: out='x.pop();'
        else: out='/*TODO %s*/'%ins
    elif mn=='fistp' and 'DWORD' in ops[0]: out='m.w32(%s, fistp(x.pop()) as u32);'%mem(ops[0])
    else:
        mm=re.match(r'f(add|sub|subr|mul|div|divr)(p?)$',mn)
        if mm:
            op=OPS[mm.group(1)];p=mm.group(2)=='p'
            if len(ops)==1 and 'PTR' in ops[0]: out='x.arith_m(Op::%s, m.rf(%s));'%(op,mem(ops[0]))
            elif len(ops)==2: out='x.arith(Op::%s, %d, %d, %s);'%(op,sti(ops[0]),sti(ops[1]),str(p).lower())
    print(('    '+out+'  '+c) if out else '    '+c)
