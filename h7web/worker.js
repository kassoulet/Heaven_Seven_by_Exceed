// audio synth worker: own instance of h7web.wasm, only pcm() is used (the synth is sequential: forward generation, cached in the wasm heap)
let w; const ready = fetch('h7web.wasm').then(r => r.arrayBuffer()).then(b => WebAssembly.instantiate(b, {})).then(i => { w = i.instance.exports; w.pcm(0, 1); postMessage({ ready: 1 }); });
onmessage = async e => {
  await ready; const { id, from, n } = e.data, p = w.pcm(from, n), d = new Int16Array(w.memory.buffer, p, n * 2).slice();
  postMessage({ id, d }, [d.buffer]);
};
