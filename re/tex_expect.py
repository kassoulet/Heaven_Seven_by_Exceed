#!/usr/bin/env python3
"""tex_expect.py - turn re/tex_capture.pkl (H7TEX=1 h7emu.py 900000000: every texture-script op with the object table before/after) into re/tex_expect.txt:
per op: seq handler esi_idx stream_start stream_end ebp138 post_w post_h post_fnv8 post_fnv32 (FNV-1a of the visible w x h region of the 8-bit (+0x28) / 32-bit (+0x24) surface; '-' = none)"""
import pickle,struct
L,B=pickle.load(open('tex_capture.pkl','rb'))
def fnv(b):
    h=0xcbf29ce484222325
    for c in b:h=((h^c)*0x100000001b3)&0xffffffffffffffff
    return h
def vis(e,nm,bpp):
    o=struct.unpack('<16I',bytes.fromhex(e['o']));w,h,p=o[6],o[7],o[8]
    if nm not in e:return '-'
    d=B[e[nm][1]];st=p//(4 if bpp==1 else 1)
    return str(fnv(b''.join(d[y*st:y*st+w*bpp] for y in range(h))))
out=[]
for i,e in enumerate(L):
    ebp=e['ebp'];esi=int(e['esi'],16);idx=(esi-ebp-0x1e0)//0x200;pe=e['post'].get(e['esi'])
    if pe:
        o=struct.unpack('<16I',bytes.fromhex(pe['o']));w,h=o[6],o[7]
        f8,f32=vis(pe,'d28',1),vis(pe,'d24',4)
    else:w=h=0;f8=f32='-'
    out.append('%d %s %d %x %x %x %d %d %s %s'%(i,e['h'],idx,e['edi'],e['edi_end'],e['g138'],w,h,f8,f32))
open('tex_expect.txt','w').write('\n'.join(out)+'\n');print('\n'.join(out))
