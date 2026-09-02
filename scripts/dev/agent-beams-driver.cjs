// Dev driver — embody a swarm in VisionClaw and stream agent-action beams.
// See docs/how-to/operations/agent-beams-dev-driver.md. Dev/demo only: it
// mints a throwaway Nostr identity and uses the session realm.
// Drive agent-action beams into VisionClaw: login (NIP-42-style signed event) → session
// token → register the swarm as agent nodes → stream notifications/agent_action over
// /wss/agent-events. usage: node beam-driver.cjs <minutes> [flagTargets=0|1]
const fs = require('fs');
const NM = process.env.BEAMS_NODE_MODULES || require('path').join(__dirname, '..', '..', 'agentbox', 'management-api', 'node_modules');
const { finalizeEvent, generateSecretKey, getPublicKey } = require(NM + '/nostr-tools');
const WebSocket = require(NM + '/ws');
const BASE = process.env.VISIONCLAW_URL || 'http://visionclaw_container:4000';
const WS = BASE.replace(/^http/, 'ws') + '/wss/agent-events';
const minutes = Number(process.argv[2] || 15); const flagTargets = process.argv[3] === '1';
const S = process.env.BEAMS_STATE_DIR || require('os').tmpdir();
const keyFile = S + '/beam-agent.key';
let sk = fs.existsSync(keyFile) ? Buffer.from(fs.readFileSync(keyFile, 'utf8').trim(), 'hex') : generateSecretKey();
if (!fs.existsSync(keyFile)) fs.writeFileSync(keyFile, Buffer.from(sk).toString('hex'), { mode: 0o600 });
const AGENTS = JSON.parse(fs.readFileSync(process.env.BEAMS_SWARM || require('path').join(__dirname, 'agent-beams-swarm.json'), 'utf8')).nodes;
const VERBS = [[0,'query',0.5],[4,'link',0.15],[1,'update',0.15],[5,'transform',0.1],[2,'create',0.05],[3,'delete',0.05]];
const SPARQL = [4558, 6449, 3805, 2104];
const pick = (arr) => arr[Math.floor(Math.random() * arr.length)];
const verb = () => { let r = Math.random(); for (const [n, name, p] of VERBS) { if ((r -= p) <= 0) return [n, name]; } return [0, 'query']; };
(async () => {
  const ev = finalizeEvent({ kind: 22242, created_at: Math.floor(Date.now() / 1000), tags: [['relay', BASE], ['challenge', 'beam-driver']], content: 'login' }, sk);
  let r = await fetch(BASE + '/api/auth/nostr', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(ev) });
  const loginRaw = await r.json(); const login = loginRaw.data || loginRaw; const token = login.token; if (!token) { console.error('login failed', r.status, JSON.stringify(login).slice(0, 200)); process.exit(1); }
  console.log('login ok pubkey', getPublicKey(sk), 'features', (login.features || []).join(','));
  r = await fetch(BASE + '/api/bots/update', { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + token, 'X-Nostr-Pubkey': getPublicKey(sk), 'X-Nostr-Token': token }, body: JSON.stringify({ nodes: AGENTS, edges: [] }) });
  console.log('bots/update', r.status, (await r.text()).slice(0, 160));
  const g = await (await fetch(BASE + '/api/graph/data')).json();
  const onto = g.data.nodes.filter(n => (n.metadata || {}).type === 'ontology_node' && Math.hypot(n.position.x, n.position.y, n.position.z) < 600).map(n => n.id);
  const pages = g.data.nodes.filter(n => (n.metadata || {}).type === 'page').map(n => n.id);
  console.log('targets: sparql', SPARQL.length, 'ontology pool', onto.length, 'pages', pages.length);
  const ws = new WebSocket(WS + '?token=' + encodeURIComponent(token), { headers: { Authorization: 'Bearer ' + token, 'X-Nostr-Pubkey': getPublicKey(sk) } });
  await new Promise((res, rej) => { ws.on('open', res); ws.on('error', rej); });
  console.log('ws open');
  let id = 1, sent = 0; const deadline = Date.now() + minutes * 60000;
  const flag = (t) => flagTargets ? ((t | 0x40000000) >>> 0) : t;
  const timer = setInterval(() => {
    if (Date.now() > deadline) { clearInterval(timer); ws.close(); console.log('done, sent', sent); process.exit(0); }
    const a = Math.floor(Math.random() * AGENTS.length); const [vt, vn] = verb();
    const roll = Math.random(); const target = roll < 0.45 ? pick(SPARQL) : roll < 0.9 ? pick(onto) : pick(pages);
    const event = { version: 3, id: id++, source_agent_id: 1000 + a, target_node_id: flag(target), action_type: vt, action_type_name: vn, timestamp: Date.now(), duration_ms: 900 + Math.floor(Math.random() * 1500), intent: `${AGENTS[a].name}: ${vn} on node ${target}`, metadata: { tool: vn === 'query' ? 'ontology_graph_query' : 'ontology_axiom_add', swarm: 'obsidian-migration' } };
    const notif = { jsonrpc: '2.0', method: 'notifications/agent_action', params: { type: 'agent_action', event, message_type: 0x23, protocol_version: 2, timestamp: new Date().toISOString() } };
    ws.send(JSON.stringify(notif)); sent++;
    if (sent % 50 === 0) console.log(new Date().toISOString(), 'sent', sent);
  }, 1200);
  ws.on('message', (d) => { const s = d.toString(); if (sent < 3) console.log('server:', s.slice(0, 200)); });
  ws.on('close', (c) => { console.log('ws closed', c, 'sent', sent); process.exit(0); });
})().catch(e => { console.error('ERR', e.message); process.exit(1); });
