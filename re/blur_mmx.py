#!/usr/bin/env python3
"""blur_mmx.py - literal transcription of op 17 (0x409578): the pipelined MMX 3x3 blur with centre weight w, bit-exact model of the original (checked against glyph_truth.json)."""
import numpy as np
M16=0xffff
def lanes(x):return [(x>>(16*i))&M16 for i in range(4)]
def pk(l):return sum((v&M16)<<(16*i) for i,v in enumerate(l))
def movd(buf,o):return int.from_bytes(bytes(buf[o:o+4]),'little') if o>=0 else int.from_bytes(bytes(buf[0:4]),'little')
def unpcklbw(x):return pk([(x>>(8*i))&0xff for i in range(4)])        # low 4 bytes -> words (mm7/mm4 = 0)
def unpckhbw(x):return pk([(x>>(8*(i+4)))&0xff for i in range(4)])
def paddw(a,b):return pk([x+y for x,y in zip(lanes(a),lanes(b))])
def pmullw(a,b):return pk([(x*y)&M16 for x,y in zip(lanes(a),lanes(b))])
def pmulhw(a,b):
    s=lambda v:v-65536 if v&0x8000 else v
    return pk([((s(x)*s(y))>>16)&M16 for x,y in zip(lanes(a),lanes(b))])
def packsswb(a):                                                       # words -> signed-saturated bytes in the low dword (high dword = mm4 = 0)
    r=0
    for i,v in enumerate(lanes(a)):
        v=v-65536 if v&0x8000 else v;v=max(-128,min(127,v));r|=(v&0xff)<<(8*i)
    return r
def blur(data,W,H,S,w):
    """data: bytearray H*S (+ slack), W = surface width ([esi+0x18]); in place."""
    w=min(w,100);K=65536//(w+9);rep=lambda v:pk([v]*4)
    scr=bytearray(0x1000);buf=data
    def sget(o):return int.from_bytes(scr[o:o+8],'little')
    def sset(o,v):scr[o:o+8]=v.to_bytes(8,'little')
    edi=ebx=0;ecx=0;edx=(W+7)>>2
    # first pass (row 0): 0x40959a..0x4095f1
    mm0=movd(buf,ebx);mm2=movd(buf,edi)
    mm0=(mm0&0xffffffff)|(movd(buf,ebx)<<32);mm2=(mm2&0xffffffff)|(movd(buf,edi)<<32)
    while True:
        mm1=movd(buf,ebx+1);mm4=mm0;mm0=unpcklbw(mm0);mm3=movd(buf,edi+1);mm4=unpckhbw(mm4);mm5=mm2;mm1=unpcklbw(mm1)
        mm0=paddw(mm0,mm4);mm2=unpcklbw(mm2);mm0=paddw(mm0,mm1);mm5=unpckhbw(mm5);mm2=paddw(mm2,mm5);mm3=unpcklbw(mm3)
        sset(ecx,mm0);mm2=paddw(mm2,mm3);sset(ecx+8,mm2)
        edi+=4;ebx+=4;ecx+=0x18
        mm0=movd(buf,ebx-1);mm0=(mm0&0xffffffff)|(movd(buf,ebx)<<32);edx-=1
        mm2=movd(buf,edi-1);mm0=(mm0&0xffffffff)|(movd(buf,edi)<<32)
        if edx==0:break
    mm6=rep(w);mm7=rep(K)
    edi=0;rows=H
    for row in range(H):
        ebx=edi+S if row!=H-1 else edi          # 0x409617..0x409627: next row (ebx) unless the height is 1
        if H==1:ebx=edi
        ecx=0;edx=(W+7)>>3;esi=5;eax=0
        pos=edi;bb=ebx
        while True:
            # chunk A
            mm0=movd(buf,pos);mm1=sget(ecx);mm0=unpcklbw(mm0);mm2=movd(buf,bb+1);mm0=pmullw(mm0,mm6);mm1=paddw(mm1,sget(ecx+8))
            mm5=movd(buf,bb);mm0=paddw(mm0,mm1);mm1=movd(buf,bb+eax);mm2=unpcklbw(mm2);mm5=unpcklbw(mm5);mm1=unpcklbw(mm1)
            mm5=paddw(mm5,mm2);mm5=paddw(mm5,mm1);mm0=paddw(mm0,mm5);mm0=pmulhw(mm0,mm7);mm0=packsswb(mm0);sset(ecx+0x10,mm5)
            # chunk B
            mm1=movd(buf,pos+4);mm2=sget(ecx+0x18);mm1=unpcklbw(mm1);mm3=movd(buf,bb+esi);mm1=pmullw(mm1,mm6);mm2=paddw(mm2,sget(ecx+0x20))
            mm5=movd(buf,bb+4);mm1=paddw(mm1,mm2);mm2=movd(buf,bb+3);mm3=unpcklbw(mm3);mm5=unpcklbw(mm5);mm2=unpcklbw(mm2)
            mm5=paddw(mm5,mm3);mm5=paddw(mm5,mm2);mm1=paddw(mm1,mm5);mm1=pmulhw(mm1,mm7);mm1=packsswb(mm1);sset(ecx+0x28,mm5)
            pos+=8;bb+=8;ecx+=0x30
            res=(mm0&0xffffffff)|((mm1&0xffffffff)<<32);buf[pos-8:pos]=res.to_bytes(8,'little')
            if edx==2:esi=4
            edx-=1;eax=-1
            if edx==0:break
        # shift pipeline 0x4096e1..0x40971b
        ecx=0;edx=(W+7)>>3
        for _ in range(edx):
            m0,m1,m2,m3=sget(ecx+8),sget(ecx+0x10),sget(ecx+0x20),sget(ecx+0x28)
            sset(ecx,m0);sset(ecx+8,m1);sset(ecx+0x18,m2);sset(ecx+0x20,m3);ecx+=0x30
        edi+=S
    return buf
