#!/usr/bin/env node
// test.js — end-to-end test for ontology-mcp in LOCAL mode.
// Spawns index.js with a tiny fixture scaffold-index.json and speaks raw
// newline-delimited JSON-RPC over the child's stdio (no extra deps).

import { spawn } from 'node:child_process';
import { writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import assert from 'node:assert/strict';

const here = dirname(fileURLToPath(import.meta.url));

// ── fixture ──────────────────────────────────────────────────────────────────
const fixtureDir = join(here, 'fixtures');
mkdirSync(fixtureDir, { recursive: true });
const fixturePath = join(fixtureDir, 'scaffold-index.json');
writeFileSync(fixturePath, JSON.stringify({
  version: 1,
  generated: '2026-08-11T00:00:00Z',
  counts: { classes: 5 },
  classes: {
    'knowledge-graph': {
      t: 'Knowledge Graph',
      d: 'A structured representation of entities and their relationships, used to integrate and reason over heterogeneous knowledge.',
      dom: 'artificial-intelligence',
      q: 0.91,
      m: 'mature',
      sup: ['graph'],
      isup: ['data-structure'],
      rel: { uses: ['ontology'], relatedTo: ['semantic-web'] },
      bl: ['semantic-web'],
    },
    graph: {
      t: 'Graph',
      d: 'A mathematical structure of vertices connected by edges.',
      dom: 'mathematics',
      q: 0.85,
      m: 'mature',
      sup: ['data-structure'],
      isup: [],
      rel: {},
      bl: ['knowledge-graph'],
    },
    ontology: {
      t: 'Ontology',
      d: 'A formal, explicit specification of a shared conceptualisation: classes, properties, and axioms.',
      dom: 'knowledge-representation',
      q: 0.88,
      m: 'established',
      sup: ['knowledge-representation'],
      isup: [],
      rel: { supports: ['knowledge-graph'] },
      bl: ['knowledge-graph', 'semantic-web'],
    },
    'data-structure': {
      t: 'Data Structure',
      d: 'A way of organising data for efficient access and modification.',
      dom: 'computer-science',
      q: 0.8,
      m: 'mature',
      sup: [],
      isup: [],
      rel: {},
      bl: ['graph'],
    },
    'knowledge-representation': {
      t: 'Knowledge Representation',
      d: 'The field of AI dedicated to representing information about the world in a machine-usable form.',
      dom: 'artificial-intelligence',
      q: 0.86,
      m: 'established',
      sup: [],
      isup: [],
      rel: {},
      bl: ['ontology'],
    },
  },
}, null, 2));

// ── minimal MCP stdio client (raw JSON-RPC, newline-delimited) ───────────────
const child = spawn(process.execPath, [join(here, 'index.js')], {
  env: { ...process.env, ONTOLOGY_INDEX: fixturePath },
  stdio: ['pipe', 'pipe', 'pipe'],
});
child.stderr.on('data', (d) => process.stderr.write(`[server] ${d}`));

let buf = '';
const pending = new Map(); // id -> resolve
child.stdout.on('data', (chunk) => {
  buf += chunk.toString('utf8');
  let nl;
  while ((nl = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, nl).trim();
    buf = buf.slice(nl + 1);
    if (!line) continue;
    let msg;
    try { msg = JSON.parse(line); } catch { continue; }
    if (msg.id !== undefined && pending.has(msg.id)) {
      pending.get(msg.id)(msg);
      pending.delete(msg.id);
    }
  }
});

let nextId = 1;
function request(method, params, { timeoutMs = 10000 } = {}) {
  const id = nextId++;
  const frame = JSON.stringify({ jsonrpc: '2.0', id, method, params });
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`timeout waiting for ${method} (id ${id})`));
    }, timeoutMs);
    pending.set(id, (msg) => { clearTimeout(timer); resolve(msg); });
    child.stdin.write(frame + '\n');
  });
}
function notify(method, params) {
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method, params }) + '\n');
}

const toolJson = (res) => {
  assert.ok(res.result, `expected result, got: ${JSON.stringify(res.error || res)}`);
  assert.ok(Array.isArray(res.result.content) && res.result.content[0]?.type === 'text',
    'tool result must carry text content');
  return JSON.parse(res.result.content[0].text);
};

let failures = 0;
const pass = (name) => console.log(`PASS  ${name}`);
const fail = (name, err) => { failures++; console.error(`FAIL  ${name}\n      ${err.message}`); };
async function step(name, fn) {
  try { await fn(); pass(name); } catch (err) { fail(name, err); }
}

// ── the test sequence ────────────────────────────────────────────────────────
try {
  await step('initialize', async () => {
    const res = await request('initialize', {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'ontology-mcp-test', version: '1.0.0' },
    });
    assert.equal(res.result?.serverInfo?.name, 'ontology-mcp');
    notify('notifications/initialized', {});
  });

  await step('tools/list exposes the 4 tools', async () => {
    const res = await request('tools/list', {});
    const names = (res.result?.tools || []).map((t) => t.name).sort();
    assert.deepEqual(names, ['ontology_ask', 'ontology_class_get', 'ontology_neighbours', 'ontology_search']);
    for (const t of res.result.tools) assert.ok(t.inputSchema?.type === 'object', `${t.name} missing inputSchema`);
  });

  await step('ontology_search finds knowledge-graph', async () => {
    const res = await request('tools/call', {
      name: 'ontology_search',
      arguments: { query: 'knowledge graph', limit: 5 },
    });
    const out = toolJson(res);
    assert.ok(!out.error, `unexpected error: ${out.message}`);
    assert.ok(out.results.length >= 1, 'expected at least one result');
    assert.equal(out.results[0].slug, 'knowledge-graph', `top hit was ${out.results[0].slug}`);
    assert.ok(out.results[0].score > 0);
  });

  await step('ontology_class_get returns the full record', async () => {
    const res = await request('tools/call', {
      name: 'ontology_class_get',
      arguments: { slug: 'urn:ngm:class:knowledge-graph' }, // IRI form must resolve too
    });
    const out = toolJson(res);
    assert.equal(out.slug, 'knowledge-graph');
    assert.equal(out.title, 'Knowledge Graph');
    assert.equal(out.domain, 'artificial-intelligence');
    assert.equal(out.maturity, 'mature');
    assert.deepEqual(out.parents.map((p) => p.slug), ['graph']);
    assert.deepEqual(out.inferredAncestors.map((p) => p.slug), ['data-structure']);
    assert.deepEqual(out.relationships.uses.map((x) => x.slug), ['ontology']);
    assert.deepEqual(out.backlinks.map((b) => b.slug), ['semantic-web']);
  });

  await step('ontology_neighbours depth=2 returns rings', async () => {
    const res = await request('tools/call', {
      name: 'ontology_neighbours',
      arguments: { slug: 'knowledge-graph', depth: 2 },
    });
    const out = toolJson(res);
    assert.equal(out.slug, 'knowledge-graph');
    assert.deepEqual(out.parents.map((p) => p.slug), ['graph']);
    assert.deepEqual(out.relationTargets.uses, ['ontology']);
    assert.ok(Array.isArray(out.ring2) && out.ring2.length >= 2, 'expected a second ring');
    const ringSlugs = out.ring2.map((n) => n.slug);
    assert.ok(ringSlugs.includes('graph') && ringSlugs.includes('ontology'), `ring2 was ${ringSlugs}`);
  });

  await step('ontology_ask returns a budget-clamped context block', async () => {
    const res = await request('tools/call', {
      name: 'ontology_ask',
      arguments: { question: 'what is a knowledge graph and how does it relate to ontology', budget_tokens: 300 },
    });
    const out = toolJson(res);
    assert.ok(!out.error, `unexpected error: ${out.message}`);
    assert.ok(out.context.length > 0, 'context must be non-empty');
    assert.ok(out.context.includes('ngm:knowledge-graph'), 'context must mention the top seed');
    assert.ok(out.context.includes('owl:Class'), 'context must be Turtle-ish');
    assert.ok(out.seed_slugs.includes('knowledge-graph'));
    assert.equal(out.budget, 300);
    assert.ok(out.tokens_used <= 300, `tokens_used ${out.tokens_used} exceeds budget`);
  });

  await step('ontology_ask respects a large budget without truncation', async () => {
    const res = await request('tools/call', {
      name: 'ontology_ask',
      arguments: { question: 'ontology', budget_tokens: 4000 },
    });
    const out = toolJson(res);
    assert.equal(out.truncated, false);
    assert.ok(out.tokens_used > 0 && out.tokens_used <= 4000);
  });

  await step('fail-open: unknown slug yields a structured error, no crash', async () => {
    const res = await request('tools/call', {
      name: 'ontology_class_get',
      arguments: { slug: 'no-such-class' },
    });
    const out = toolJson(res);
    assert.equal(out.error, 'not_found');
    assert.equal(res.result.isError, true);
    // Server must still answer afterwards:
    const res2 = await request('tools/list', {});
    assert.ok(res2.result?.tools?.length === 4, 'server died after error');
  });
} finally {
  child.kill('SIGTERM');
}

if (failures) {
  console.error(`\n${failures} test(s) failed`);
  process.exit(1);
}
console.log('\nAll tests passed.');
process.exit(0);
