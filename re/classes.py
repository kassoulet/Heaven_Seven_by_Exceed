#!/usr/bin/env python3
"""classes.py - decode the operator CLASS table: word offsets at 0x42994f (60 entries) -> records at 0x42993c+off.
Record: +0 class id, +1 parent class id (single inheritance; the root points to itself), +2 ?, +3 handler offset (word, + [ebp+0] = 0x40695a), +5 ? (word),
+7.. six (parameter type, slot) byte pairs (types = the parameter table 0x429cd0, 0 = unused). 0x409a6b instantiates a class by walking the parent chain to the root and running the
handlers root-first (0x409ae0 creates the declared parameters of each level via 0x407be1 + 0x409ace)."""
import struct
d=open('mem_a.bin','rb').read()
at=lambda a,n:d[a-0x400000:a-0x400000+n]
tab=struct.unpack('<60H',at(0x42994f,120));base=0x40695a
byid={}
for i,o in enumerate(tab):
    if o==0 and i>0:continue
    r=at(0x42993c+o,0x19);byid[i]=(r[0],r[1],r[2],base+struct.unpack('<H',r[3:5])[0],struct.unpack('<H',r[5:7])[0],[(r[7+2*k],r[8+2*k]) for k in range(6) if r[7+2*k]])
PT={1:'tex',2:'time',3:'int',4:'f1',5:'f2',6:'f3',7:'matrix',8:'color',9:'byte'}
def chain(i):
    c=[i]
    while byid[c[-1]][1] not in (c[-1],) and byid[c[-1]][1] in byid and len(c)<10:c.append(byid[c[-1]][1])
    return c
with open('classes.txt','w') as f:
    for i,(cid,par,x2,h,w5,ps) in sorted(byid.items()):
        f.write('class %3d parent %3d flags %02x handler %#x w5 %04x chain %s params %s\n'%(i,par,x2,h,w5,chain(i),[(PT.get(t,t),s) for t,s in ps]))
print(open('classes.txt').read())
