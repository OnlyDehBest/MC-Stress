const $ = (id) => document.getElementById(id);
const css = (v) => getComputedStyle(document.documentElement).getPropertyValue(v).trim();
const SVGNS = 'http://www.w3.org/2000/svg';

const hist = { bots: [], lat: [] };
const MAX = 90;
let bp = null;

function stamp() { return new Date().toLocaleTimeString().slice(0, 8); }

function logLine(line) {
  const el = $('log');
  if (el.dataset.v !== '1') { el.textContent = ''; el.dataset.v = '1'; }
  const row = document.createElement('div');
  row.innerHTML = line;
  el.appendChild(row);
  el.scrollTop = el.scrollHeight;
}

function renderStats(s) {
  $('k-bots').innerHTML = `${s.alive ?? 0}<small> / ${s.target ?? 0}</small>`;
  const mv = $('k-moving'); mv.textContent = s.moving ?? 0;
  mv.className = 'sv ' + ((s.moving ?? 0) > 0 ? 'med' : 'ok');
  bp = s.breakingPoint ?? null;
  const bpe = $('k-bp');
  if (bp != null) { bpe.innerHTML = `~${bp}<small> bot</small>`; bpe.className = 'sv crit'; }
  else { bpe.textContent = '—'; bpe.className = 'sv'; }
  const srv = s.server;
  $('k-lat').innerHTML = srv && srv.latency != null ? `${srv.latency}<small> ms</small>` : '—<small> ms</small>';
  $('k-online').textContent = srv && srv.online != null ? srv.online : '—';
  $('k-err').textContent = s.errors ?? 0;

  const live = !!s.running;
  $('status').className = 'status' + (live ? ' live' : '');
  $('statusTxt').textContent = live ? 'in esecuzione' : 'fermo';
  $('start').disabled = live;
  $('stop').disabled = !live;
  $('clock').textContent = stamp();

  if (live) {
    hist.bots.push(s.alive ?? 0);
    hist.lat.push(srv && srv.latency != null ? srv.latency : null);
    if (hist.bots.length > MAX) { hist.bots.shift(); hist.lat.shift(); }
    drawChart();
  }
}

function el(tag, attrs) { const n = document.createElementNS(SVGNS, tag); for (const k in attrs) n.setAttribute(k, attrs[k]); return n; }

function drawChart() {
  const svg = $('chart'); while (svg.firstChild) svg.removeChild(svg.firstChild);
  const W = 800, H = 240, pL = 40, pR = 40, pT = 16, pB = 22;
  const plotW = W - pL - pR, plotH = H - pT - pB;
  const n = hist.bots.length;
  const latVals = hist.lat.filter((v) => v != null);
  const latMax = Math.max(100, ...latVals);
  const lMax = Math.ceil(latMax / 50) * 50;
  const bMax = Math.max(10, ...hist.bots);
  const X = (i) => pL + (n <= 1 ? plotW / 2 : plotW * i / (n - 1));
  const Yl = (v) => pT + plotH * (1 - Math.min(v, lMax) / lMax);
  const Yb = (v) => pT + plotH * (1 - Math.min(v, bMax) / bMax);

  for (let v = 0; v <= lMax; v += lMax / 4) {
    const y = Yl(v);
    svg.appendChild(el('line', { class: 'grid', x1: pL, y1: y, x2: W - pR, y2: y, stroke: css('--line2') }));
    const t = el('text', { x: pL - 6, y: y + 3.5, 'text-anchor': 'end', fill: css('--muted'), 'font-family': css('--fm'), 'font-size': '11' });
    t.textContent = Math.round(v); svg.appendChild(t);
  }
  const tb = el('text', { x: W - pR + 6, y: Yb(bMax) + 3.5, 'text-anchor': 'start', fill: css('--muted'), 'font-family': css('--fm'), 'font-size': '11' });
  tb.textContent = bMax; svg.appendChild(tb);

  if (n >= 1) {
    // Area bot (scala destra)
    let bpts = `${X(0)},${Yb(hist.bots[0])}`;
    for (let i = 1; i < n; i++) bpts += ` ${X(i)},${Yb(hist.bots[i])}`;
    svg.appendChild(el('polygon', { points: `${pL},${pT + plotH} ${bpts} ${X(n - 1)},${pT + plotH}`, fill: css('--accl'), stroke: css('--acc'), 'stroke-width': 1.5 }));
    // Linea latenza
    let lp = '', started = false;
    for (let i = 0; i < n; i++) {
      if (hist.lat[i] == null) { started = false; continue; }
      lp += `${started ? ' L' : 'M'}${X(i)},${Yl(hist.lat[i])}`;
      started = true;
    }
    if (lp) svg.appendChild(el('path', { d: lp, fill: 'none', stroke: css('--high'), 'stroke-width': 2, 'stroke-linejoin': 'round' }));
  }

  // Marcatore breaking point
  if (bp != null && n > 1) {
    let bi = hist.bots.findIndex((b) => b >= bp);
    if (bi >= 0) {
      const bx = X(bi);
      svg.appendChild(el('line', { x1: bx, y1: pT, x2: bx, y2: pT + plotH, stroke: css('--crit'), 'stroke-width': 1, 'stroke-dasharray': '3 3', opacity: '.7' }));
      const t = el('text', { x: Math.min(bx + 5, W - pR - 48), y: pT + 11, fill: css('--crit'), 'font-family': css('--fm'), 'font-size': '10.5' });
      t.textContent = `~${bp} bot`; svg.appendChild(t);
    }
  }
}
drawChart();

// WebSocket
function connect() {
  const ws = new WebSocket(`ws://${location.host}/ws`);
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.type === 'log') logLine(msg.line);
    else if (msg.type === 'stats') renderStats(msg.stats);
  };
  ws.onclose = () => setTimeout(connect, 1500);
}
connect();

function readConfig() {
  const cfg = {
    host: $('host').value.trim(),
    port: Number($('port').value),
    prefix: $('prefix').value.trim(),
    target: Number($('target').value),
    step: Number($('step').value),
    intervalMs: Number($('intervalMs').value),
    viewDistance: Number($('viewDistance').value),
    spreadSeconds: Number($('spreadSeconds').value),
    mode: $('mode').value,
  };
  const p = $('protocol').value.trim();
  if (p) cfg.protocol = Number(p);
  return cfg;
}

$('start').onclick = async () => {
  hist.bots.length = 0; hist.lat.length = 0; bp = null;
  const res = await fetch('/api/start', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(readConfig()) });
  const data = await res.json();
  if (!data.ok) logLine(`<span class="t">${stamp()}</span>  <span style="color:var(--crit)">ERRORE:</span> ${data.error}`);
};

$('stop').onclick = () => fetch('/api/stop', { method: 'POST' });
