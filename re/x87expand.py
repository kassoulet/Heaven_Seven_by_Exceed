#!/usr/bin/env python3
# expand.py in.rs out.rs : replace lines "@GEN a b@" by the generated x87 code (comment lines of non-x87 instrs dropped)
import sys,re,subprocess
out=[]
for l in open(sys.argv[1]):
    m=re.match(r'\s*@GEN ([0-9a-f]+) ([0-9a-f]+)@',l)
    if m:
        r=subprocess.run(['python3','/tmp/claude-1000/w44/gen.py',m.group(1),m.group(2)],capture_output=True,text=True).stdout
        ind=re.match(r'\s*',l).group(0)
        for g in r.splitlines():
            g=g.strip()
            if g.startswith('//'): continue
            out.append(ind+g+'\n')
    else: out.append(l)
open(sys.argv[2],'w').write(''.join(out))
