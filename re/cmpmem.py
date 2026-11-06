#!/usr/bin/env python3
"""cmpmem.py a.bin b.bin [maxruns]: list the differing byte ranges (image ++ heap dumps; address = 0x400000 + offset for the image, 0x20000000 + offset - 188416 for the heap)."""
import sys,numpy as np
a=np.fromfile(sys.argv[1],np.uint8);b=np.fromfile(sys.argv[2],np.uint8);n=min(len(a),len(b))
d=np.nonzero(a[:n]!=b[:n])[0];print(len(d),'differing bytes (sizes %d %d)'%(len(a),len(b)))
IMG=188416
def addr(o): return 0x400000+o if o<IMG else 0x20000000+o-IMG
runs=[];s=None
for o in d:
    if s is not None and o-p<=16: p=o;continue
    if s is not None: runs.append((s,p))
    s=p=o
if s is not None: runs.append((s,p))
for s,e in runs[:int(sys.argv[3]) if len(sys.argv)>3 else 40]:print('%x..%x (%d bytes)'%(addr(s),addr(e),e-s+1),a[s:s+8].tobytes().hex(),b[s:s+8].tobytes().hex())
print(len(runs),'runs')
