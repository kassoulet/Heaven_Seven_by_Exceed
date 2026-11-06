#!/usr/bin/env python3
"""mk_full.py <dis> <snapshot.bin> <cov_load.pkl> <out.pkl> [roots.txt]: instruction set + block leaders for the complete static recompile (recomp.py ... full).
Roots = every dword of the cold image+heap that is the address of an instruction in the code section (node handlers, vtables, call-back pointers) + the 0x409a50 / 0x401e46 entries + every block entry the emulator saw during the load (jump-table targets);
S = recursive descent over direct jumps / calls / fall-through from the roots (no linear sweep: data inside the code section is never translated)."""
import re,sys,pickle,numpy as np
DIS,SNAP,COV,OUT=sys.argv[1:5]
CODE=(0x401000,0x40f000)
I={}
for l in open(DIS):
    m=re.match(r'\s*([0-9a-f]+):\t([0-9a-f ]+)\t(.*)',l)
    if m: a=int(m.group(1),16);last=a;I[a]=[len(m.group(2).split()),re.sub(r'\s+',' ',m.group(3).strip())]
    else:
        m=re.match(r'\s*([0-9a-f]+):\t([0-9a-f ]+)\s*$',l)
        if m: I[last][0]+=len(m.group(2).split())
if len(sys.argv)>5: roots={int(x,16) for x in open(sys.argv[5]).read().split()}
else:
    raw=np.fromfile(SNAP,dtype=np.uint8);w=raw[:len(raw)//4*4].view('<u4')
    roots={int(x) for x in np.unique(w[(w>=CODE[0])&(w<CODE[1])]) if int(x) in I}
    open('roots_full.txt','w').write('\n'.join('%x'%r for r in sorted(roots))+'\n')
cov=pickle.load(open(COV,'rb'))
roots|={0x409a50,0x401e46,0x40168c}|set(cov)
BR=re.compile(r'^(call|jmp|j[a-z]+|loop[a-z]*) 0x([0-9a-f]+)$')
IMM=re.compile(r'(?<![0-9a-fx])0x(4[0-9a-f]{5})\b')
S=set();work=list(roots)
while work:
    a=work.pop()
    while a in I and a not in S and CODE[0]<=a<CODE[1]:
        S.add(a);ln,t=I[a];mn=t.split(' ')[0]
        m=BR.match(t)
        if m:
            tt=int(m.group(2),16)
            if tt in I and tt not in S: work.append(tt)
        if mn in('ret','retf','iret','hlt','jmp'): break
        for x in IMM.findall(t):      # code addresses used as immediates (handler pointers stored into objects at run time: push 0x409ac1, mov [esi+0xb4], 0x406f3c ...) are roots too
            v=int(x,16)
            if v in I and v not in S and CODE[0]<=v<CODE[1] and not m: roots.add(v);work.append(v)
        a+=ln
# the instructions the emulator executed during the load are in by construction (their block entries are roots)
pickle.dump({'S':S,'lead':roots},open(OUT,'wb'));print(len(roots),'roots',len(S),'instructions')
