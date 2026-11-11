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

  var W = 400, H = 150, N = 60, MAX_BOTS = 240, MAX_LAT = 400;
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

  var bots = [], lats = [], t = 0, breakAt = null, bpBots = null;

  function latencyFor(b) {
    var base = 18 + b * 0.12;
    if (b > 170) base += Math.pow(b - 170, 1.6) * 0.9;
    return Math.min(MAX_LAT, base + (Math.random() - 0.5) * 10);
  }

  function step() {
    var b;
    var cycle = t % 90;
    if (cycle === 0) { bots = []; lats = []; breakAt = null; bpBots = null; }
    b = Math.min(MAX_BOTS, Math.floor(cycle / 3) * 10);
    if (cycle > 72) b = MAX_BOTS;
    var l = latencyFor(b);
    bots.push(b); lats.push(l);
    if (bots.length > N) { bots.shift(); lats.shift(); if (breakAt !== null) breakAt--; }
    if (bpBots === null && l > 150) { breakAt = bots.length - 1; bpBots = b; }
    t++;
    render(b, l);
  }

  function pathFor(arr, max) {
    var d = '';
    for (var i = 0; i < arr.length; i++) {
      var x = (i / (N - 1)) * W;
      var y = H - (arr[i] / max) * (H - 8) - 4;
      d += (i ? 'L' : 'M') + x.toFixed(1) + ' ' + y.toFixed(1);
    }
    return d;
  }

  function render(b, l) {
    var bp = pathFor(bots, MAX_BOTS);
    botPath.setAttribute('d', bp);
    var lastX = ((bots.length - 1) / (N - 1)) * W;
    botArea.setAttribute('d', bp ? bp + 'L' + lastX.toFixed(1) + ' ' + H + 'L0 ' + H + 'Z' : '');
    latPath.setAttribute('d', pathFor(lats, MAX_LAT));
    elBots.textContent = b;
    elLat.innerHTML = Math.round(l) + '<small style="font-size:12px;color:var(--muted);font-weight:400"> ms</small>';
    if (bpBots !== null) {
      var x = (breakAt / (N - 1)) * W;
      bpLine.setAttribute('x1', x); bpLine.setAttribute('x2', x); bpLine.setAttribute('opacity', breakAt >= 0 ? 1 : 0);
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
    for (var i = 0; i < 80; i++) step();
  } else {
    step();
    setInterval(step, 350);
  }
})();
