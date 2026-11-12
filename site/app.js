(function () {
  var reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  var models = document.querySelector('.models');
  if (models) {
    if ('IntersectionObserver' in window && !reduce) {
      var io = new IntersectionObserver(function (entries) {
        entries.forEach(function (e) {
          if (e.isIntersecting) { models.classList.add('in'); io.disconnect(); }
        });
      }, { threshold: 0.3 });
      io.observe(models);
    } else {
      models.classList.add('in');
    }
  }

  document.querySelectorAll('.copy').forEach(function (button) {
    button.addEventListener('click', function () {
      var text = button.getAttribute('data-copy');
      var done = function () {
        button.textContent = 'Copied';
        setTimeout(function () { button.textContent = 'Copy'; }, 1400);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).then(done, function () {});
      }
    });
  });

  var svg = document.getElementById('demoChart');
  if (!svg) return;
  var elBots = document.getElementById('dBots');
  var elLat = document.getElementById('dLat');
  var elBp = document.getElementById('dBp');
  var elState = document.getElementById('demoState');
  var status = elState.parentNode;

  var W = 400, H = 150, MAX_BOTS = 240, MAX_LAT = 400;
  var WINDOW = 16;   // seconds of history visible
  var CYCLE = 24;    // seconds per ramp-to-breaking-point loop
  var NS = 'http://www.w3.org/2000/svg';

  function el(name, attrs) {
    var node = document.createElementNS(NS, name);
    for (var k in attrs) node.setAttribute(k, attrs[k]);
    return node;
  }

  for (var g = 1; g < 4; g++) {
    svg.appendChild(el('line', { x1: 0, x2: W, y1: (H / 4) * g, y2: (H / 4) * g, stroke: '#2c2124', 'stroke-width': 1, 'vector-effect': 'non-scaling-stroke' }));
  }
  var bpLine = el('line', { y1: 0, y2: H, stroke: '#ff5c61', 'stroke-width': 1.2, 'stroke-dasharray': '4 4', 'vector-effect': 'non-scaling-stroke', opacity: 0 });
  var botArea = el('path', { fill: 'rgba(240,67,74,.12)' });
  var botPath = el('path', { fill: 'none', stroke: '#f0434a', 'stroke-width': 2, 'stroke-linejoin': 'round', 'vector-effect': 'non-scaling-stroke' });
  var latPath = el('path', { fill: 'none', stroke: '#f0883e', 'stroke-width': 1.8, 'stroke-linejoin': 'round', 'vector-effect': 'non-scaling-stroke' });
  svg.appendChild(botArea); svg.appendChild(bpLine); svg.appendChild(botPath); svg.appendChild(latPath);

  var samples = [];         // { t, b, l }
  var dispBots = 0, dispLat = 0;
  var latNoise = 0;
  var bpBots = null, bpTime = null;
  var prevCycle = 0, lastSample = -1;

  function botsAt(cycle) {
    var up = CYCLE * 0.72;
    return cycle < up ? MAX_BOTS * (cycle / up) : MAX_BOTS;
  }

  function latencyAt(b, dt) {
    var base = 18 + b * 0.12;
    if (b > 170) base += Math.pow(b - 170, 1.6) * 0.9;
    // Mean-reverting noise so the line wiggles smoothly rather than jumping.
    latNoise += (Math.random() - 0.5) * 60 * dt;
    latNoise *= Math.max(0, 1 - 1.5 * dt);
    return Math.max(4, Math.min(MAX_LAT, base + latNoise));
  }

  function xFor(t, now) { return (1 - (now - t) / WINDOW) * W; }
  function yFor(v, max) { return H - (v / max) * (H - 8) - 4; }

  function buildPath(now, key, max) {
    var d = '';
    for (var i = 0; i < samples.length; i++) {
      var s = samples[i];
      var x = xFor(s.t, now);
      if (x < -4) continue;
      d += (d ? 'L' : 'M') + x.toFixed(1) + ' ' + yFor(s[key], max).toFixed(1);
    }
    return d;
  }

  function advance(now, dt) {
    var cycle = now % CYCLE;
    if (cycle < prevCycle) { samples = []; bpBots = null; bpTime = null; latNoise = 0; }
    prevCycle = cycle;
    var b = botsAt(cycle);
    var l = latencyAt(b, dt);
    if (now - lastSample >= 0.05) { samples.push({ t: now, b: b, l: l }); lastSample = now; }
    while (samples.length > 2 && samples[0].t < now - WINDOW - 0.5) samples.shift();
    if (bpBots === null && l > 150) { bpBots = Math.round(b); bpTime = now; }
    var ease = Math.min(1, dt * 6);
    dispBots += (b - dispBots) * ease;
    dispLat += (l - dispLat) * ease;
    paint(now);
  }

  function paint(now) {
    var bp = buildPath(now, 'b', MAX_BOTS);
    botPath.setAttribute('d', bp);
    if (bp) {
      var lastX = xFor(samples[samples.length - 1].t, now);
      botArea.setAttribute('d', bp + 'L' + lastX.toFixed(1) + ' ' + H + 'L0 ' + H + 'Z');
    } else {
      botArea.setAttribute('d', '');
    }
    latPath.setAttribute('d', buildPath(now, 'l', MAX_LAT));
    elBots.textContent = Math.round(dispBots);
    elLat.innerHTML = Math.round(dispLat) + '<small style="font-size:12px;color:var(--muted);font-weight:400"> ms</small>';
    if (bpTime !== null) {
      var x = xFor(bpTime, now);
      bpLine.setAttribute('x1', x); bpLine.setAttribute('x2', x);
      bpLine.setAttribute('opacity', x > 0 ? 1 : 0);
      elBp.textContent = '~' + bpBots + ' bots';
      elBp.className = 'sv crit';
      elState.textContent = 'breaking point';
      status.classList.add('crit');
    } else {
      bpLine.setAttribute('opacity', 0);
      elBp.textContent = '—';
      elBp.className = 'sv';
      elState.textContent = 'ramping';
      status.classList.remove('crit');
    }
  }

  if (reduce) {
    // Static representative frame, no animation.
    for (var k = 0; k <= WINDOW; k += 0.1) {
      var b = botsAt(k), base = 18 + b * 0.12;
      if (b > 170) base += Math.pow(b - 170, 1.6) * 0.9;
      samples.push({ t: k, b: b, l: Math.min(MAX_LAT, base) });
      if (bpBots === null && base > 150) { bpBots = Math.round(b); bpTime = k; }
    }
    dispBots = samples[samples.length - 1].b;
    dispLat = samples[samples.length - 1].l;
    paint(WINDOW);
    return;
  }

  var start = performance.now() / 1000;
  var last = start;
  function tick(ms) {
    var now = ms / 1000 - start;
    var dt = Math.min(0.1, now - (last - start));
    last = ms / 1000;
    advance(now, dt);
    requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);
})();
