const $ = (id) => document.getElementById(id);
const css = (v) => getComputedStyle(document.documentElement).getPropertyValue(v).trim();
const SVGNS = 'http://www.w3.org/2000/svg';

const history = { bots: [], lat: [] };
const MAX = 90;
let breakingPoint = null;

function stamp() { return new Date().toLocaleTimeString().slice(0, 8); }

function appendLog(line) {
  const log = $('log');
  if (log.dataset.v !== '1') { log.textContent = ''; log.dataset.v = '1'; }
  const row = document.createElement('div');
  row.innerHTML = line;
  log.appendChild(row);
  log.scrollTop = log.scrollHeight;
}

function renderStats(s) {
  $('k-bots').innerHTML = `${s.alive ?? 0}<small> / ${s.target ?? 0}</small>`;
  const moving = $('k-moving');
  moving.textContent = s.moving ?? 0;
  moving.className = 'sv ' + ((s.moving ?? 0) > 0 ? 'med' : 'ok');
  breakingPoint = s.breakingPoint ?? null;
  const bp = $('k-bp');
  if (breakingPoint != null) { bp.innerHTML = `~${breakingPoint}<small> bot</small>`; bp.className = 'sv crit'; }
  else { bp.textContent = '—'; bp.className = 'sv'; }
  const server = s.server;
  $('k-lat').innerHTML = server && server.latency != null ? `${server.latency}<small> ms</small>` : '—<small> ms</small>';
  $('k-online').textContent = server && server.online != null ? server.online : '—';
  $('k-err').textContent = s.errors ?? 0;
  if (s.version) $('engine').textContent = 'raw · ' + s.version;

  const live = !!s.running;
  $('status').className = 'status' + (live ? ' live' : '');
  $('statusTxt').textContent = live ? 'running' : 'idle';
  $('start').disabled = live;
  $('stop').disabled = !live;
  $('clock').textContent = stamp();

  if (live) {
    history.bots.push(s.alive ?? 0);
    history.lat.push(server && server.latency != null ? server.latency : null);
    if (history.bots.length > MAX) { history.bots.shift(); history.lat.shift(); }
    drawChart();
  }
}

function make(tag, attrs) {
  const node = document.createElementNS(SVGNS, tag);
  for (const key in attrs) node.setAttribute(key, attrs[key]);
  return node;
}

function drawChart() {
  const svg = $('chart');
  while (svg.firstChild) svg.removeChild(svg.firstChild);
  const W = 800, H = 240, pL = 40, pR = 40, pT = 16, pB = 22;
  const plotW = W - pL - pR, plotH = H - pT - pB;
  const n = history.bots.length;
  const latencyValues = history.lat.filter((v) => v != null);
  const latencyMax = Math.ceil(Math.max(100, ...latencyValues) / 50) * 50;
  const botsMax = Math.max(10, ...history.bots);
  const x = (i) => pL + (n <= 1 ? plotW / 2 : plotW * i / (n - 1));
  const yLatency = (v) => pT + plotH * (1 - Math.min(v, latencyMax) / latencyMax);
  const yBots = (v) => pT + plotH * (1 - Math.min(v, botsMax) / botsMax);

  for (let v = 0; v <= latencyMax; v += latencyMax / 4) {
    const y = yLatency(v);
    svg.appendChild(make('line', { x1: pL, y1: y, x2: W - pR, y2: y, stroke: css('--line2') }));
    const label = make('text', { x: pL - 6, y: y + 3.5, 'text-anchor': 'end', fill: css('--muted'), 'font-family': css('--fm'), 'font-size': '11' });
    label.textContent = Math.round(v);
    svg.appendChild(label);
  }
  const botsLabel = make('text', { x: W - pR + 6, y: yBots(botsMax) + 3.5, 'text-anchor': 'start', fill: css('--muted'), 'font-family': css('--fm'), 'font-size': '11' });
  botsLabel.textContent = botsMax;
  svg.appendChild(botsLabel);

  if (n >= 1) {
    let botsPath = `${x(0)},${yBots(history.bots[0])}`;
    for (let i = 1; i < n; i++) botsPath += ` ${x(i)},${yBots(history.bots[i])}`;
    svg.appendChild(make('polygon', { points: `${pL},${pT + plotH} ${botsPath} ${x(n - 1)},${pT + plotH}`, fill: css('--accl'), stroke: css('--acc'), 'stroke-width': 1.5 }));
    let latencyPath = '', open = false;
    for (let i = 0; i < n; i++) {
      if (history.lat[i] == null) { open = false; continue; }
      latencyPath += `${open ? ' L' : 'M'}${x(i)},${yLatency(history.lat[i])}`;
      open = true;
    }
    if (latencyPath) svg.appendChild(make('path', { d: latencyPath, fill: 'none', stroke: css('--high'), 'stroke-width': 2, 'stroke-linejoin': 'round' }));
  }

  if (breakingPoint != null && n > 1) {
    const index = history.bots.findIndex((b) => b >= breakingPoint);
    if (index >= 0) {
      const bx = x(index);
      svg.appendChild(make('line', { x1: bx, y1: pT, x2: bx, y2: pT + plotH, stroke: css('--crit'), 'stroke-width': 1, 'stroke-dasharray': '3 3', opacity: '.7' }));
      const label = make('text', { x: Math.min(bx + 5, W - pR - 48), y: pT + 11, fill: css('--crit'), 'font-family': css('--fm'), 'font-size': '10.5' });
      label.textContent = `~${breakingPoint} bot`;
      svg.appendChild(label);
    }
  }
}
drawChart();

function connect() {
  const ws = new WebSocket(`ws://${location.host}/ws`);
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.type === 'log') appendLog(message.line);
    else if (message.type === 'stats') renderStats(message.stats);
  };
  ws.onclose = () => setTimeout(connect, 1500);
}
connect();

function readConfig() {
  const config = {
    host: $('host').value.trim(),
    port: Number($('port').value),
    nickname: $('nickname').value.trim(),
    target: Number($('target').value),
    step: Number($('step').value),
    intervalMs: Number($('intervalMs').value),
    viewDistance: Number($('viewDistance').value),
    spreadSeconds: Number($('spreadSeconds').value),
    mode: $('mode').value,
    actionIntervalMs: Math.max(500, (Number($('actionSeconds').value) || 5) * 1000),
  };
  if ($('protocolMode').value === 'manual') {
    const protocol = $('protocol').value.trim();
    if (protocol) config.protocol = Number(protocol);
  }
  const chat = $('chatMessage').value.trim();
  if (chat) config.chatMessage = chat;
  const command = $('command').value.trim();
  if (command) config.command = command;
  if ($('swing').value === 'on') config.swing = true;
  if ($('mine').value === 'on') config.mine = true;
  if ($('attack').value === 'on') config.attack = true;
  if ($('interact').value === 'on') config.interact = true;
  return config;
}

$('protocolMode').onchange = () => {
  $('protocol').disabled = $('protocolMode').value !== 'manual';
  if ($('protocol').disabled) $('protocol').value = '';
};

$('start').onclick = async () => {
  history.bots.length = 0; history.lat.length = 0; breakingPoint = null;
  const response = await fetch('/api/start', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(readConfig()) });
  const data = await response.json();
  if (!data.ok) appendLog(`<span class="t">${stamp()}</span>  <span style="color:var(--crit)">ERROR:</span> ${data.error}`);
};

$('stop').onclick = () => fetch('/api/stop', { method: 'POST' });
