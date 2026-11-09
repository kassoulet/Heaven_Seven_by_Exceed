// Heaven 7 player: h7web.wasm (exact original frames, 40 ms grid) -> canvas 2D (exact 320x240) or WebGL CRT post-process (display only).
const $ = id => document.getElementById(id), W = 320, H = 240, FPS = 25;
let wasm, N, cur = 1, playing = false, t0 = 0, k0 = 1, crtOn = false, gl, tex;
const ctx = $('c').getContext('2d'), img = ctx.createImageData(W, H);

function frame(k) {                         // draw grid frame k (1-based); returns ms spent in wasm
  const t = performance.now(), p = wasm.frame(k), ms = performance.now() - t;
  img.data.set(new Uint8ClampedArray(wasm.memory.buffer, p, W * H * 4)); cur = k;
  if (crtOn) crtDraw(); else ctx.putImageData(img, 0, 0);
  $('seek').value = k; $('tm').textContent = `${(k - 1) / FPS | 0}s / ${(N - 1) / FPS | 0}s  (${ms.toFixed(1)} ms)`;
  return ms;
}
function loop() {
  if (!playing) return;
  if (aWait) return void requestAnimationFrame(loop);        // audio not started yet
  const k = Math.min(N, k0 + Math.floor((performance.now() - t0) * FPS / 1000));
  if (k !== cur) frame(k);
  if (k >= N) setPlay(false); else requestAnimationFrame(loop);
}
function setPlay(p) { playing = p; $('pp').textContent = p ? 'pause' : 'play'; audioStop(); if (p) { if (cur >= N) cur = 1; audioStart(); k0 = cur; t0 = performance.now(); requestAnimationFrame(loop); } }
$('pp').onclick = () => setPlay(!playing);
$('seek').oninput = e => { const k = +e.target.value; frame(k); if (playing) audioStart(); k0 = k; t0 = performance.now(); };
$('crt').onchange = e => { crtOn = e.target.checked; $('crtc').style.display = crtOn ? 'block' : 'none'; if (crtOn) crtInit(); frame(cur); };

// ---- CRT pass (post-process only): barrel curvature, scanlines, RGB shadow mask, cheap bloom, vignette
const FS = `precision mediump float; uniform sampler2D u; varying vec2 v;
vec2 curve(vec2 p){ p = p*2.-1.; p *= 1. + dot(p.yx,p.yx)*vec2(.03,.04); return p*.5+.5; }
vec3 s(vec2 p){ return texture2D(u,p).rgb; }
void main(){
  vec2 p = curve(v); if(p.x<0.||p.x>1.||p.y<0.||p.y>1.){ gl_FragColor=vec4(0,0,0,1); return; }
  vec3 c = s(p); vec2 d = vec2(1./320., 1./240.)*1.5;
  vec3 b = (s(p+vec2(d.x,0.))+s(p-vec2(d.x,0.))+s(p+vec2(0.,d.y))+s(p-vec2(0.,d.y)))*.25;
  c = c*.85 + b*b*.6;                                           // bloom
  float sc = .6 + .4*pow(abs(sin(p.y*240.*3.14159)), .6);       // scanline per source row
  float m = mod(floor(gl_FragCoord.x), 3.); vec3 mask = vec3(.82) + vec3(.36)*vec3(m<1.,m>=1.&&m<2.,m>=2.); // shadow mask
  vec2 q = p*2.-1.; float vig = 1. - .25*dot(q,q);
  gl_FragColor = vec4(pow(c*sc*mask*vig*1.1, vec3(1.1)), 1.);
}`;
function crtInit() {
  if (gl) return; gl = $('crtc').getContext('webgl'); const mk = (t, s) => { const o = gl.createShader(t); gl.shaderSource(o, s); gl.compileShader(o); return o; };
  const pr = gl.createProgram(); gl.attachShader(pr, mk(gl.VERTEX_SHADER, 'attribute vec2 a;varying vec2 v;void main(){v=vec2(a.x*.5+.5,.5-a.y*.5);gl_Position=vec4(a,0,1);}'));
  gl.attachShader(pr, mk(gl.FRAGMENT_SHADER, FS)); gl.linkProgram(pr); gl.useProgram(pr);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer()); gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  tex = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D, tex);
  for (const [k, v] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, v);
}
function crtDraw() { gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, W, H, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(img.data.buffer)); gl.viewport(0, 0, 960, 720); gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4); }

// ---- audio: the original's synth PCM, generated in worker.js (own wasm instance) in 0.5 s chunks, scheduled as AudioBuffers on the audio clock.
// (Re)started from (cur-1)/FPS on play / seek; the video waits (aWait) until the first chunk is scheduled, then restarts from `cur` so both begin together.
// A far seek costs ~0.06 s of worker time per second skipped (the synth is sequential) but never blocks the page.
const aw = new Worker('worker.js'); let ac, aSrc = [], aNext = 0, aT = null, aGen = 0, aBusy = false, aWait = false, aTimer = 0;
const muted = () => $('mute').checked, aTotal = () => wasm.pcm_frames();
function audioStop() { aGen++; aBusy = false; aWait = false; clearInterval(aTimer); aSrc.forEach(s => { try { s.stop(); } catch (e) {} }); aSrc = []; }
function audioStart() {
  audioStop(); if (muted()) return;
  ac ||= new AudioContext({ sampleRate: 44100 }); ac.resume();
  aNext = Math.round((cur - 1) / FPS * 44100); aT = null; aWait = true; aTimer = setInterval(pump, 200); pump();
}
function pump() {
  if (!playing || muted() || aBusy || aNext >= aTotal() || (aT !== null && aT - ac.currentTime >= 1)) return;
  aBusy = true; aw.postMessage({ id: aGen, from: aNext, n: Math.min(22050, aTotal() - aNext) });
}
aw.onmessage = e => {
  if (e.data.ready || e.data.id !== aGen) return;               // synth ready note / stale chunk (seek, pause, mute since the request)
  aBusy = false; const d = e.data.d, n = d.length / 2, b = ac.createBuffer(2, n, 44100), l = b.getChannelData(0), r = b.getChannelData(1);
  for (let i = 0; i < n; i++) { l[i] = d[2 * i] / 32768; r[i] = d[2 * i + 1] / 32768; }
  if (aT === null || aT < ac.currentTime + .02) { aT = ac.currentTime + .05; if (playing) { k0 = cur; t0 = performance.now(); } }   // first chunk, or underrun: realign the video
  aWait = false; const s = ac.createBufferSource(); s.buffer = b; s.connect(ac.destination); s.start(aT); aSrc.push(s); aT += n / 44100; aNext += n;
  s.onended = () => { aSrc = aSrc.filter(x => x !== s); }; pump();
};
$('mute').onchange = () => { audioStop(); if (playing && !muted()) audioStart(); };

(async () => {
  const r = await fetch('h7web.wasm'); wasm = (await WebAssembly.instantiate(await r.arrayBuffer(), {})).instance.exports;
  await new Promise(r => setTimeout(r, 50)); const t = performance.now(); wasm.init(192);
  N = wasm.frames(); $('seek').max = N; ['pp', 'seek'].forEach(i => $(i).disabled = false);
  console.log('init ms', performance.now() - t);
  window.demo = { frame, N: () => N, rgba: () => Array.from(img.data), ctx };
  frame(1); window.demoReady = true;
})();
