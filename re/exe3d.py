import sys,pickle
exec(open('/tmp/claude-1000/w/prof3d.py').read().split("C=collections.Counter()")[0])
mu.reg_write(UC_X86_REG_FPCW,0x7f)
S=set();ST=[0]
def hk(uc,a,sz,_):
    if a==0x401e46: ST[0]=1
    if ST[0]: S.add(a)
mu.hook_add(UC_HOOK_CODE,hk,begin=0x401000,end=0x41d000)
mu.emu_start(0x40a680,0xdead0000,count=400000000)
pickle.dump(S,open('/tmp/claude-1000/w/exe_%d.pkl'%k,'wb'));print(k,len(S))
