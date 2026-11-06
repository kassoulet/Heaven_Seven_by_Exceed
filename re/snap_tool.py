#!/usr/bin/env python3
"""snap_tool.py - regenerate the scene snapshots used by h7core/src/scene.rs tests (not committed: 13 MB each).
  H7EVAL=1 H7SND=2 H7STEP=40 H7SNAP=1,2,3,31,32 H7PSNAP=1,2,3,31 H7MAXFR=40 python3 h7emu.py 60000000000   (~80 s)
writes /tmp/claude-1000/snap<k>.z (state at the evaluator entry of frame k) + snap<k>.json (regs, ebx = t), psnap<k>.z (state when the first draw handler of frame k is
called = after the evaluation phase), ev.pkl (evaluator/draw trace), frames.pkl (the presented frames).  Then: python3 snap_tool.py  -> raw .bin files next to the tests' H7SNAP_DIR."""
import zlib,glob,os,sys
out=sys.argv[1] if len(sys.argv)>1 else '/tmp/claude-1000/w'
for f in glob.glob('/tmp/claude-1000/*snap[0-9]*.z'):
    o=os.path.join(out,os.path.basename(f)[:-2]+'.bin');open(o,'wb').write(zlib.decompress(open(f,'rb').read()));print(o)
