#!/usr/bin/env python3
"""snap_tool.py - regenerate the scene snapshots used by the h7core/src/scene.rs tests (not committed: 13 MB each).
  H7EVAL=1 H7SND=2 H7STEP=40 H7SNAP=1,2,3,31,100,240 H7PSNAP=1,2,3,31,100,240 H7TSNAP=1,2,3,31,45,60,100,180,240 H7BSNAP=... H7MAXFR=245 python3 h7emu.py 60000000000   (~100 s)
snap<k>  = state at the evaluator entry of frame k      psnap<k> = state when the first draw handler is called (after the evaluation phase)
tsnap<k> = state at the first text draw (0x409cda)       bsnap<k> = state at the class-5 sprite draw (0x408d8c)
ev.pkl = evaluator/draw trace, frames.pkl = the presented frames.  Then `python3 snap_tool.py [outdir]` writes <name>.bin (raw: image ++ heap), fb<k>.bin (presented frame k, BGRA 320x240) and times.txt (frame k, t)."""
import zlib,glob,os,sys,pickle
out=sys.argv[1] if len(sys.argv)>1 else '/tmp/claude-1000/w'
for f in glob.glob('/tmp/claude-1000/*snap[0-9]*.z'):
    o=os.path.join(out,os.path.basename(f)[:-2]+'.bin');open(o,'wb').write(zlib.decompress(open(f,'rb').read()));print(o)
for k,(t,b) in enumerate(pickle.load(open('/tmp/claude-1000/frames.pkl','rb'))):open(os.path.join(out,'fb%d.bin'%k),'wb').write(b)
F=[e for e in pickle.load(open('/tmp/claude-1000/ev.pkl','rb')) if e[0]=='F']
open(os.path.join(out,'times.txt'),'w').write('\n'.join('%d %d'%(k,e[1]) for k,e in enumerate(F)))
