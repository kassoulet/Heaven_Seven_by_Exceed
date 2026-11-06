import pickle,re,sys
cov=pickle.load(open(sys.argv[1],'rb'))
I={}
for l in open(sys.argv[2]):
    m=re.match(r'\s*([0-9a-f]+):\t([0-9a-f ]+)\t(.*)',l)
    if m: I[int(m.group(1),16)]=m.group(3)
S=set()
for a,sz in cov.items():
    x=a
    while x<a+sz:
        assert x in I,hex(x); S.add(x); 
        # length: next key
        n=[k for k in range(x+1,x+16) if k in I][0]; x=n
pickle.dump({'S':S,'lead':set(cov)},open(sys.argv[3],'wb'))
import collections
c=collections.Counter(I[a].split()[0] for a in S);print(len(S),dict(c))
