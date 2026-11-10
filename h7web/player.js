// Heaven 7 player: h7web.wasm (exact original frames, 40 ms grid) -> canvas 2D (exact 320x240); optional CSS CRT overlay (display only).
const $ = id => document.getElementById(id), FPS = 25, RES = (() => { try { return Math.min(3, +localStorage.h7res || 0); } catch (e) { return 0; } })(), [W, H] = [[320, 240], [512, 384], [640, 480], [800, 600]][RES];
let wasm, N, cur = 1, playing = false, t0 = 0, k0 = 1, crtOn = false;
$('c').width = W; $('c').height = H; if (RES) $('c').style.imageRendering = 'auto';
$('hq').value = RES; $('hq').onchange = e => { try { localStorage.h7res = e.target.value; } catch (_) {} location.reload(); };
const ctx = $('c').getContext('2d'), img = ctx.createImageData(W, H);

function frame(k) {                         // draw grid frame k (1-based); returns ms spent in wasm
  const t = performance.now(), p = wasm.frame(k), ms = performance.now() - t;
  img.data.set(new Uint8ClampedArray(wasm.memory.buffer, p, W * H * 4)); cur = k;
  ctx.putImageData(img, 0, 0);
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

// ---- CRT: CSS overlay (scanlines + faint vignette). Stage height = k*240 device px so one source row = k whole device pixels (no moire).
const fit = () => {
  const s = $('scr'), cb = $('crt'), dpr = devicePixelRatio, k = Math.floor(Math.min(innerHeight - 90, (innerWidth * .96) * 3 / 4) * dpr / H), ok = k >= 2;
  cb.disabled = !ok; cb.title = ok ? '' : 'window too small for the CRT effect';
  const on = crtOn && ok; s.classList.toggle('crt', on);
  s.style.height = on ? k * H / dpr + 'px' : ''; s.style.width = on ? k * W / dpr + 'px' : '';
  s.style.setProperty('--p', k / dpr + 'px'); $('bar').style.width = on ? s.style.width : '';
};
const setCrt = v => { crtOn = v; $('crt').checked = v; try { localStorage.crt = v ? 1 : 0; } catch (e) {} fit(); };
try { crtOn = localStorage.crt === '1'; } catch (e) {} $('crt').checked = crtOn;
$('crt').onchange = e => setCrt(e.target.checked);
addEventListener('keydown', e => { if ((e.key === 'c' || e.key === 'C') && !e.ctrlKey && !e.metaKey && !e.altKey && !$('crt').disabled) setCrt(!crtOn); });
addEventListener('resize', fit); document.addEventListener('fullscreenchange', fit);
matchMedia(`(resolution: ${devicePixelRatio}dppx)`).onchange = fit;   // dpr change (zoom / monitor move)
fit();

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
  await new Promise(r => setTimeout(r, 50)); const t = performance.now(); wasm.set_res(RES); wasm.init(RES ? 384 : 192);
  N = wasm.frames(); $('seek').max = N; ['pp', 'seek'].forEach(i => $(i).disabled = false);
  console.log('init ms', performance.now() - t);
  window.demo = { frame, N: () => N, rgba: () => Array.from(img.data), ctx };
  frame(1); window.demoReady = true;
})();
