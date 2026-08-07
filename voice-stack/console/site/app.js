// agentbox voice console — behaviour.
// One origin (:8444): /bridge/* + /feed → tab0-bridge, /api/* → Unmute
// backend, /embed → compact voice strip (same-origin iframe).

const $ = (id) => document.getElementById(id);

const KINDS = {
  user: 'you → tab 0',
  assistant: 'tab 0',
  'voice-user': 'you (heard by STT)',
  'voice-inject': 'voice → tab 0',
  'voice-reply': 'voice',
  'nostr-inject': 'nostr → tab 0',
  'nostr-out': 'console → nostr',
};

// ── status chips ──────────────────────────────────────────────────────────

function setChip(el, state, text) {
  el.className = `chip ${state}`;
  if (text) el.title = text;
}

async function pollHealth() {
  try {
    const h = await (await fetch('/bridge/health')).json();
    setChip($('chip-bridge'), h.ok ? 'ok' : 'bad');
    $('sys-facts').textContent =
      `backend ${h.backend} · model ${h.model}\ntmux tabs ${h.tabs} · turns ${h.turns}`;
  } catch {
    setChip($('chip-bridge'), 'bad');
  }
  try {
    const v = await (await fetch('/api/v1/health')).json();
    setChip($('chip-voice'), v.ok ? 'ok' : 'bad');
  } catch {
    setChip($('chip-voice'), 'bad');
  }
  try {
    const n = await (await fetch('/bridge/nostr/status')).json();
    const ok = n.gateway === 'armed';
    setChip($('chip-nostr'), ok ? 'ok' : 'bad',
      `gateway ${n.gateway} · mirror key ${n.mirrorKey ? 'present' : 'missing'}`);
    $('nostr-state').textContent = n.gateway;
  } catch {
    setChip($('chip-nostr'), 'bad');
    $('nostr-state').textContent = 'unreachable';
  }
}

// ── conversation feed (websocket) ─────────────────────────────────────────

const log = $('log');
const seen = new Map();

function atBottom(el) {
  return el.scrollHeight - el.scrollTop - el.clientHeight < 60;
}

function renderTurn(turn) {
  const pinned = atBottom(log);
  let el = seen.get(turn.id);
  if (!el) {
    el = document.createElement('div');
    seen.set(turn.id, el);
    log.appendChild(el);
  }
  const brief = turn.summary ||
    (turn.text.length > 400 ? turn.text.slice(0, 400) + ' …' : turn.text);
  const hasMore = turn.summary || turn.text.length > 400;
  el.className = `turn ${turn.kind}${hasMore ? ' has-more' : ''}`;
  el.innerHTML =
    `<div class="meta">${KINDS[turn.kind] || turn.kind} · ${turn.ts.slice(11, 19)}</div>` +
    '<pre class="brief"></pre><pre class="full"></pre>';
  el.querySelector('.brief').textContent = brief;
  el.querySelector('.full').textContent = turn.text;
  if (hasMore) el.onclick = () => el.classList.toggle('expanded');
  if (pinned) log.scrollTop = log.scrollHeight;
}

function connectFeed() {
  const ws = new WebSocket(`wss://${location.host}/feed`);
  ws.onmessage = (e) => {
    const msg = JSON.parse(e.data);
    if (msg.type === 'snapshot') msg.turns.forEach(renderTurn);
    else if (msg.type === 'turn' || msg.type === 'turn-update') renderTurn(msg.turn);
  };
  ws.onclose = () => setTimeout(connectFeed, 2000);
}

// ── tmux tabs (read-only) ─────────────────────────────────────────────────

let tabs = [];
let active = 'feed';           // 'feed' | window index (number)
let paneTimer = null;

function selectTab(id) {
  active = id;
  document.querySelectorAll('#tabbar .tab').forEach((b) => {
    b.classList.toggle('active', b.dataset.id === String(id));
  });
  const isFeed = id === 'feed';
  $('feed-view').hidden = !isFeed;
  $('pane-view').hidden = isFeed;
  clearInterval(paneTimer);
  if (!isFeed) {
    const tab = tabs.find((t) => t.index === id);
    $('pane-title').textContent = `tmux ${id}:${tab ? tab.name : ''}`;
    const poll = async () => {
      try {
        const d = await (await fetch(`/bridge/tabs/${id}?lines=160`)).json();
        const out = $('pane-output');
        const pinned = atBottom(out);
        out.textContent = d.output || '(empty pane)';
        if (pinned) out.scrollTop = out.scrollHeight;
      } catch { /* keep last capture on transient errors */ }
    };
    poll();
    paneTimer = setInterval(poll, 1500);
  } else {
    log.scrollTop = log.scrollHeight;
  }
}

function renderTabbar() {
  const bar = $('tabbar');
  bar.innerHTML = '';
  const mk = (id, label) => {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'tab' + (String(active) === String(id) ? ' active' : '');
    b.dataset.id = String(id);
    b.textContent = label;
    b.onclick = () => selectTab(id);
    bar.appendChild(b);
  };
  mk('feed', 'conversation');
  tabs.forEach((t) => mk(t.index, `${t.index}:${t.name}`));
}

async function pollTabs() {
  try {
    const d = await (await fetch('/bridge/tabs')).json();
    const sig = JSON.stringify(d.tabs);
    if (sig !== JSON.stringify(tabs)) {
      tabs = d.tabs;
      renderTabbar();
    }
  } catch { /* bridge down — chips already show it */ }
}

// ── nostr panel ───────────────────────────────────────────────────────────

async function pollNostrEvents() {
  try {
    const d = await (await fetch('/bridge/nostr/events?n=15')).json();
    const box = $('nostr-events');
    box.innerHTML = '';
    d.events.slice().reverse().forEach((e) => {
      const div = document.createElement('div');
      div.className = 'evt';
      const ts = new Date(e.ts * 1000).toISOString().slice(5, 16).replace('T', ' ');
      div.innerHTML = '<span class="ts"></span><span class="cmd"></span>';
      div.querySelector('.ts').textContent = ts;
      div.querySelector('.cmd').textContent = e.cmd || JSON.stringify(e);
      box.appendChild(div);
    });
  } catch { /* panel header already reflects status */ }
}

// ── composers ─────────────────────────────────────────────────────────────

function wireForm(formId, inputId, url) {
  $(formId).addEventListener('submit', async (e) => {
    e.preventDefault();
    const input = $(inputId);
    const text = input.value.trim();
    if (!text) return;
    input.disabled = true;
    try {
      await fetch(url, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ text }),
      });
      input.value = '';
    } finally {
      input.disabled = false;
      input.focus();
    }
  });
}

// ── boot ──────────────────────────────────────────────────────────────────

connectFeed();
renderTabbar();
selectTab('feed');
pollHealth();
pollTabs();
pollNostrEvents();
setInterval(pollHealth, 12000);
setInterval(pollTabs, 10000);
setInterval(pollNostrEvents, 20000);
wireForm('send', 'text', '/bridge/tab0/send');
wireForm('nostr-send', 'nostr-text', '/bridge/nostr/send');
