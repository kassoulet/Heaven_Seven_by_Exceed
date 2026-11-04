#!/usr/bin/env python3
"""script_dis.py - list the build script from re/script_trace.json (stream positions recorded while the original runs it): tree indentation from op 1 (create, +1 level) / op 2 (pop),
class names from classes.txt, the raw argument bytes of every op (decoding per class comes with the handler ports)."""
import json,re
t=json.load(open('script_trace.json'));d=open('image.bin','rb').read()
at=lambda a,n:d[a-0x400000:a-0x400000+n]
cls={}
for l in open('classes.txt'):
    m=re.match(r'class\s+(\d+) parent\s+(\d+) flags (\w+) handler (\w+) w5 (\w+) chain (\[.*?\]) params (.*)',l)
    if m:cls[int(m.group(1))]=(int(m.group(2)),m.group(4),m.group(7))
OPN={1:'new',2:'pop',3:'rand',4:'table',5:'op5',6:'op6',7:'op7',17:'op17',18:'seed',19:'op19',20:'to8',23:'down2',24:'crack',25:'f25',26:'f26',27:'f27',28:'f28',29:'f29',30:'f30'}
depth=0;out=[]
for a,h,b in t:
    a=int(a,16);b=int(b,16);op=at(a-1,1)[0];args=at(a,b-a)
    if op==2:depth=max(0,depth-1)
    name=OPN.get(op,'param%d'%(op-7) if 8<=op<=16 else 'op%d'%op)
    extra=''
    if op==1:extra=' class %d %s'%(args[0],cls.get(args[0],('?','?','?'))[2])
    out.append('%06x %s%-6s %s%s'%(a-1,'  '*depth,name,args.hex(' ')[:70],extra))
    if op==1:depth+=1
open('script.txt','w').write('\n'.join(out)+'\n');print(len(out))
