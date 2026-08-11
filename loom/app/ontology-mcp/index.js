#!/usr/bin/env node
// ontology-mcp — standalone, portable stdio MCP server for the NarrativeGoldmine
// ontology. Extracted from the agentbox ontology machinery (ontology-bridge.js +
// lib/ontology-retrieval.js + lib/ontology-budget.js), simplified to a single
// dependency (@modelcontextprotocol/sdk) and two transports:
//
//   PUBLIC mode (default): reads the published site JSON API.
//     ONTOLOGY_SITE=https://narrativegoldmine.com
//       GET /api/search-index.json         (cached in-process, fuzzy-matched locally)
//       GET /api/pages/<slug>.json         (per-class record, cached in-process)
//
//   LOCAL mode: reads a scaffold-index.json from disk (loaded once, served from memory).
//     ONTOLOGY_INDEX=/path/to/scaffold-index.json
//
// Tools: ontology_search, ontology_class_get, ontology_neighbours, ontology_ask.
// Fail-open everywhere: tool results carry { error, message } strings; the
// process never crashes on a bad fetch, bad file, or bad argument.

import { readFileSync } from 'node:fs';
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  ListToolsRequestSchema,
  CallToolRequestSchema,
} from '@modelcontextprotocol/sdk/types.js';

// ──────────────────────────────────────────────────────────────────────────────
// Budget governor (ported from ontology-budget.js, simplified to one knob).
// ──────────────────────────────────────────────────────────────────────────────

const TRUNCATION_MARK = '\n# … [truncated: token budget reached]';
const DEFAULT_BUDGET = 1500;
const MIN_BUDGET = 100;
const MAX_BUDGET = 8000;

/** Cheap deterministic estimate: ~4 chars/token, rounded UP (under-fill, never over). */
function estimateTokens(str) {
  if (!str) return 0;
  return Math.ceil(String(str).length / 4);
}

/** Resolve the single budget knob to a hard ceiling within [MIN, MAX]. */
function resolveBudget(budgetTokens) {
  const n = Number(budgetTokens);
  if (!Number.isFinite(n) || n <= 0) return DEFAULT_BUDGET;
  return Math.max(MIN_BUDGET, Math.min(Math.floor(n), MAX_BUDGET));
}

/**
 * Clamp serialised text to the budget. Truncates on a line boundary where
 * possible so the emitted block stays parseable-ish, then appends a marker.
 * @returns {{text:string, tokens:number, truncated:boolean, budget:number}}
 */
function clampToBudget(text, budgetTokens) {
  const budget = resolveBudget(budgetTokens);
  const src = text == null ? '' : String(text);
  const tokens = estimateTokens(src);
  if (tokens <= budget) return { text: src, tokens, truncated: false, budget };
  const markTokens = estimateTokens(TRUNCATION_MARK);
  const charBudget = Math.max(0, (budget - markTokens) * 4);
  let cut = src.slice(0, charBudget);
  const lastNl = cut.lastIndexOf('\n');
  if (lastNl > charBudget * 0.5) cut = cut.slice(0, lastNl); // prefer a line boundary
  const out = cut + TRUNCATION_MARK;
  return { text: out, tokens: estimateTokens(out), truncated: true, budget };
}

// ──────────────────────────────────────────────────────────────────────────────
// Slug + fuzzy-match helpers.
// ──────────────────────────────────────────────────────────────────────────────

/** kebab-case slug, matching the pipeline: re.sub(r'[^a-z0-9]+','-',s.lower()).strip('-') */
function slugify(s) {
  return String(s == null ? '' : s)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

/** Slug from a ref that may be a slug, an IRI (urn:ngm:class:<slug>), or an object. */
function slugFromRef(ref) {
  if (ref == null) return '';
  if (typeof ref === 'object') ref = ref.id ?? ref.iri ?? ref.slug ?? '';
  const s = String(ref);
  return s.includes(':') ? s.split(':').pop() : s;
}

/**
 * Local fuzzy relevance score for a (query, title, slug[, definition]) pair.
 * Deterministic, allocation-light; 0 means "no match".
 */
function scoreMatch(query, title, slug, definition) {
  const q = String(query || '').toLowerCase().trim();
  if (!q) return 0;
  const t = String(title || '').toLowerCase();
  const sl = String(slug || '');
  const qSlug = slugify(q);
  let score = 0;
  if (t === q || sl === qSlug) return 100;
  if (t.includes(q)) score += 55;
  else if (qSlug && sl.includes(qSlug)) score += 40;
  const qTokens = q.split(/[^a-z0-9]+/).filter((x) => x.length > 1);
  if (qTokens.length) {
    const hay = new Set([...t.split(/[^a-z0-9]+/), ...sl.split('-')].filter(Boolean));
    let hits = 0;
    let prefixHits = 0;
    for (const tok of qTokens) {
      if (hay.has(tok)) { hits++; continue; }
      for (const h of hay) {
        if (h.length > 2 && (h.startsWith(tok) || tok.startsWith(h))) { prefixHits++; break; }
      }
    }
    score += Math.round((hits / qTokens.length) * 35 + (prefixHits / qTokens.length) * 12);
    if (definition && hits === 0 && prefixHits === 0) {
      const d = String(definition).toLowerCase();
      let dHits = 0;
      for (const tok of qTokens) if (d.includes(tok)) dHits++;
      score += Math.round((dHits / qTokens.length) * 8);
    }
  }
  return score;
}

// ──────────────────────────────────────────────────────────────────────────────
// Backends. Both normalise class records to ONE shape:
//   { slug, title, definition, domain, qualityScore, maturity,
//     parents:        [{slug,label}],
//     inferredAncestors:[{slug,label}],
//     relationships:  { <type>: [{slug,label}] },
//     backlinks:      [{slug,label}] }
// ──────────────────────────────────────────────────────────────────────────────

const FETCH_TIMEOUT_MS = parseInt(process.env.ONTOLOGY_TIMEOUT_MS || '10000', 10);

async function fetchJson(url) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
  try {
    const res = await fetch(url, { signal: controller.signal, headers: { Accept: 'application/json' } });
    if (!res.ok) return { error: `http_${res.status}`, message: `${url} → ${res.status} ${res.statusText}` };
    return await res.json();
  } catch (err) {
    if (err && err.name === 'AbortError') {
      return { error: 'ontology_timeout', message: `${url} did not respond within ${FETCH_TIMEOUT_MS}ms` };
    }
    return { error: 'ontology_unavailable', message: `${url}: ${err && err.message ? err.message : String(err)}` };
  } finally {
    clearTimeout(timer);
  }
}

const isErr = (r) => r && typeof r === 'object' && typeof r.error === 'string';

/** PUBLIC mode: published site JSON API. */
function createPublicBackend(site) {
  const base = String(site).replace(/\/$/, '');
  let indexPromise = null; // cached in-process; entries have slug/title fields
  const pageCache = new Map();

  async function loadIndex() {
    if (!indexPromise) {
      indexPromise = (async () => {
        const raw = await fetchJson(`${base}/api/search-index.json`);
        if (isErr(raw)) { indexPromise = null; return raw; } // retry on next call
        // Tolerate shape drift: bare array, or wrapped in entries/classes/pages,
        // or a {slug: title-ish} map.
        let entries = Array.isArray(raw) ? raw
          : Array.isArray(raw && raw.entries) ? raw.entries
          : Array.isArray(raw && raw.classes) ? raw.classes
          : Array.isArray(raw && raw.pages) ? raw.pages
          : null;
        if (!entries && raw && typeof raw === 'object') {
          entries = Object.entries(raw).map(([slug, v]) => ({
            slug,
            title: typeof v === 'string' ? v : (v && (v.title || v.t)) || slug,
          }));
        }
        if (!entries) return { error: 'bad_index', message: 'search-index.json has an unrecognised shape' };
        return entries
          .map((e) => ({ slug: e.slug || slugFromRef(e.id) || slugify(e.title), title: e.title || e.slug || '' }))
          .filter((e) => e.slug);
      })();
    }
    return indexPromise;
  }

  return {
    mode: 'public',
    describe: () => `public site API at ${base}`,

    async search(query, limit) {
      const entries = await loadIndex();
      if (isErr(entries)) return entries;
      const scored = [];
      for (const e of entries) {
        const s = scoreMatch(query, e.title, e.slug);
        if (s > 0) scored.push({ slug: e.slug, title: e.title, score: s });
      }
      scored.sort((a, b) => b.score - a.score || a.slug.localeCompare(b.slug));
      return scored.slice(0, limit);
    },

    async getClass(slug) {
      const clean = slugFromRef(slug) || slugify(slug);
      if (!clean) return { error: 'bad_slug', message: 'empty slug' };
      if (pageCache.has(clean)) return pageCache.get(clean);
      const raw = await fetchJson(`${base}/api/pages/${encodeURIComponent(clean)}.json`);
      if (isErr(raw)) {
        return raw.error === 'http_404'
          ? { error: 'not_found', message: `no class page for slug "${clean}"` }
          : raw; // transient errors are NOT cached
      }
      const refList = (arr) => (Array.isArray(arr) ? arr : [])
        .map((x) => ({ slug: slugFromRef(x), label: (x && x.label) || slugFromRef(x) }))
        .filter((x) => x.slug);
      const rels = {};
      if (raw.relationships && typeof raw.relationships === 'object') {
        for (const [type, arr] of Object.entries(raw.relationships)) {
          const list = refList(arr);
          if (list.length) rels[type] = list;
        }
      }
      const rec = {
        slug: raw.slug || clean,
        title: raw.title || clean,
        definition: raw.definition || '',
        domain: raw.domain || '',
        qualityScore: typeof raw.qualityScore === 'number' ? raw.qualityScore : null,
        maturity: raw.maturity || '',
        parents: refList(raw.subClassOf),
        // inferredSuperClasses may be absent on older deploys — tolerate.
        inferredAncestors: refList(raw.inferredSuperClasses),
        relationships: rels,
        backlinks: (Array.isArray(raw.backlinks) ? raw.backlinks : [])
          .map((b) => ({ slug: (b && b.slug) || slugFromRef(b), label: (b && b.label) || (b && b.slug) || '' }))
          .filter((b) => b.slug),
      };
      pageCache.set(clean, rec);
      return rec;
    },
  };
}

/** LOCAL mode: scaffold-index.json loaded once, served from memory. */
function createLocalBackend(indexPath) {
  let state = null; // { classes } | { error, message }

  function load() {
    if (state) return state;
    try {
      const raw = JSON.parse(readFileSync(indexPath, 'utf8'));
      if (!raw || typeof raw.classes !== 'object' || raw.classes === null) {
        state = { error: 'bad_index', message: `${indexPath}: missing "classes" map` };
      } else {
        state = { classes: raw.classes, version: raw.version, generated: raw.generated };
      }
    } catch (err) {
      state = { error: 'index_unavailable', message: `${indexPath}: ${err && err.message ? err.message : String(err)}` };
    }
    return state;
  }

  const titleOf = (classes, slug) => (classes[slug] && classes[slug].t) || slug;
  const refList = (classes, slugs) => (Array.isArray(slugs) ? slugs : [])
    .map((s) => ({ slug: s, label: titleOf(classes, s) }));

  return {
    mode: 'local',
    describe: () => `local scaffold index at ${indexPath}`,

    async search(query, limit) {
      const st = load();
      if (isErr(st)) return st;
      const scored = [];
      for (const [slug, c] of Object.entries(st.classes)) {
        const s = scoreMatch(query, c.t, slug, c.d);
        if (s > 0) scored.push({ slug, title: c.t || slug, score: s });
      }
      scored.sort((a, b) => b.score - a.score || a.slug.localeCompare(b.slug));
      return scored.slice(0, limit);
    },

    async getClass(slug) {
      const st = load();
      if (isErr(st)) return st;
      const clean = slugFromRef(slug) || slugify(slug);
      const c = st.classes[clean];
      if (!c) return { error: 'not_found', message: `no class "${clean}" in local index` };
      const rels = {};
      if (c.rel && typeof c.rel === 'object') {
        for (const [type, slugs] of Object.entries(c.rel)) {
          const list = refList(st.classes, slugs);
          if (list.length) rels[type] = list;
        }
      }
      return {
        slug: clean,
        title: c.t || clean,
        definition: c.d || '',
        domain: c.dom || '',
        qualityScore: typeof c.q === 'number' ? c.q : null,
        maturity: c.m || '',
        parents: refList(st.classes, c.sup),
        inferredAncestors: refList(st.classes, c.isup),
        relationships: rels,
        backlinks: refList(st.classes, c.bl),
      };
    },
  };
}

// ──────────────────────────────────────────────────────────────────────────────
// Backend selection (env).
// ──────────────────────────────────────────────────────────────────────────────

const DEFAULT_SITE = 'https://narrativegoldmine.com';
const backend = process.env.ONTOLOGY_INDEX
  ? createLocalBackend(process.env.ONTOLOGY_INDEX)
  : createPublicBackend(process.env.ONTOLOGY_SITE || DEFAULT_SITE);

// ──────────────────────────────────────────────────────────────────────────────
// Tool logic (backend-agnostic).
// ──────────────────────────────────────────────────────────────────────────────

async function toolSearch({ query, limit }) {
  const q = String(query || '').trim();
  if (!q) return { error: 'bad_query', message: 'query must be a non-empty string' };
  const lim = Math.max(1, Math.min(Number(limit) || 8, 50));
  const results = await backend.search(q, lim);
  if (isErr(results)) return results;
  return { query: q, count: results.length, results };
}

async function toolClassGet({ slug }) {
  const s = String(slug || '').trim();
  if (!s) return { error: 'bad_slug', message: 'slug must be a non-empty string' };
  return backend.getClass(s);
}

const NEIGHBOUR_EXPANSION_CAP = 12; // bounded fan-out at depth > 1

async function toolNeighbours({ slug, depth }) {
  const s = String(slug || '').trim();
  if (!s) return { error: 'bad_slug', message: 'slug must be a non-empty string' };
  const d = Math.max(1, Math.min(Number(depth) || 1, 3));
  const rec = await backend.getClass(s);
  if (isErr(rec)) return rec;

  const relationTargets = {};
  for (const [type, list] of Object.entries(rec.relationships)) {
    relationTargets[type] = list.map((x) => x.slug);
  }
  const out = {
    slug: rec.slug,
    title: rec.title,
    depth: d,
    parents: rec.parents,
    inferredAncestors: rec.inferredAncestors,
    relationTargets,
    backlinks: rec.backlinks,
  };

  if (d > 1) {
    // Bounded second ring: fetch a capped set of first-ring neighbours and
    // include their own summary (parents + relation target slugs).
    const seen = new Set([rec.slug]);
    const frontier = [];
    const push = (sl) => { if (sl && !seen.has(sl)) { seen.add(sl); frontier.push(sl); } };
    for (const p of rec.parents) push(p.slug);
    for (const list of Object.values(rec.relationships)) for (const x of list) push(x.slug);
    for (const b of rec.backlinks.slice(0, 5)) push(b.slug);

    const ring = [];
    for (const sl of frontier.slice(0, NEIGHBOUR_EXPANSION_CAP)) {
      const n = await backend.getClass(sl); // sequential: gentle on the public site
      if (isErr(n)) { ring.push({ slug: sl, error: n.error }); continue; }
      const rt = {};
      for (const [type, list] of Object.entries(n.relationships)) rt[type] = list.map((x) => x.slug);
      ring.push({
        slug: n.slug,
        title: n.title,
        domain: n.domain,
        parents: n.parents.map((p) => p.slug),
        relationTargets: rt,
      });
    }
    out.ring2 = ring;
    out.ring2_truncated = frontier.length > NEIGHBOUR_EXPANSION_CAP;
  }
  return out;
}

/**
 * Terse Turtle-ish serialisation of class records (ported from
 * ontology-retrieval.js serialiseTurtle; prefix-once, 2-9x cheaper than JSON).
 */
function serialiseContext(question, records) {
  const str = (s) => '"' + String(s == null ? '' : s).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, ' ') + '"';
  const iri = (slug) => `ngm:${slug}`;
  const lines = [
    `# ontology context for: ${String(question).replace(/\n/g, ' ')}`,
    '@prefix ngm: <urn:ngm:class:> .',
    '@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .',
    '@prefix owl: <http://www.w3.org/2002/07/owl#> .',
    '@prefix vc: <https://narrativegoldmine.com/ns/v1#> .',
    '',
  ];
  for (const c of records) {
    const parts = [`${iri(c.slug)} a owl:Class`];
    parts.push(`  rdfs:label ${str(c.title)}`);
    if (c.domain) parts.push(`  vc:sourceDomain ${str(c.domain)}`);
    if (c.maturity) parts.push(`  vc:maturity ${str(c.maturity)}`);
    if (typeof c.qualityScore === 'number') parts.push(`  vc:qualityScore ${c.qualityScore}`);
    if (c.parents.length) {
      parts.push(`  rdfs:subClassOf ${c.parents.map((p) => iri(p.slug)).join(', ')}`);
    }
    if (c.inferredAncestors.length) {
      parts.push(`  vc:inferredSuperClass ${c.inferredAncestors.slice(0, 8).map((p) => iri(p.slug)).join(', ')}`);
    }
    for (const [type, list] of Object.entries(c.relationships)) {
      if (list.length) parts.push(`  vc:${type} ${list.slice(0, 6).map((x) => iri(x.slug)).join(', ')}`);
    }
    lines.push(parts.join(' ;\n') + ' .');
    if (c.definition) {
      lines.push(`# ${String(c.definition).slice(0, 300).replace(/\n/g, ' ')}`);
    }
    lines.push('');
  }
  return lines.join('\n');
}

const ASK_SEED_LIMIT = 8;   // search fan-out
const ASK_DETAIL_LIMIT = 5; // full records serialised into the block

async function toolAsk({ question, budget_tokens }) {
  const q = String(question || '').trim();
  if (!q) return { error: 'bad_question', message: 'question must be a non-empty string' };
  const budget = resolveBudget(budget_tokens);

  const seeds = await backend.search(q, ASK_SEED_LIMIT);
  if (isErr(seeds)) return { ...seeds, degraded: true };
  if (!seeds.length) {
    return { question: q, context: '', seed_slugs: [], tokens_used: 0, truncated: false, budget, message: 'no matching classes' };
  }

  // Fetch full records for the top seeds; tolerate individual failures (fail-open
  // to whatever subset resolved).
  const records = [];
  const failed = [];
  for (const seed of seeds.slice(0, ASK_DETAIL_LIMIT)) {
    const rec = await backend.getClass(seed.slug);
    if (isErr(rec)) failed.push({ slug: seed.slug, error: rec.error });
    else records.push(rec);
  }
  if (!records.length) {
    return { error: 'retrieval_unavailable', message: 'matched classes but could not fetch any record', failed, degraded: true };
  }

  const clamped = clampToBudget(serialiseContext(q, records), budget);
  const out = {
    question: q,
    context: clamped.text,
    seed_slugs: seeds.map((s) => s.slug),
    detailed_slugs: records.map((r) => r.slug),
    tokens_used: clamped.tokens,
    truncated: clamped.truncated,
    budget: clamped.budget,
    mode: backend.mode,
  };
  if (failed.length) out.failed = failed;
  return out;
}

// ──────────────────────────────────────────────────────────────────────────────
// MCP server wiring.
// ──────────────────────────────────────────────────────────────────────────────

const TOOLS = [
  {
    name: 'ontology_search',
    description: 'Fuzzy-search ontology classes by name. Returns relevance-ranked {slug, title, score}.',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Search text (class name or keywords)' },
        limit: { type: 'number', description: 'Max results (default 8, cap 50)', default: 8 },
      },
      required: ['query'],
      additionalProperties: false,
    },
  },
  {
    name: 'ontology_class_get',
    description: 'Fetch the full record for one ontology class: definition, domain, quality, maturity, parents, inferred ancestors, typed relationships, backlinks.',
    inputSchema: {
      type: 'object',
      properties: {
        slug: { type: 'string', description: 'Class slug (kebab-case) or IRI urn:ngm:class:<slug>' },
      },
      required: ['slug'],
      additionalProperties: false,
    },
  },
  {
    name: 'ontology_neighbours',
    description: 'Graph neighbourhood of a class: direct parents, inferred ancestors, relation targets by type, backlinks. depth=2 adds a bounded second ring of neighbour summaries.',
    inputSchema: {
      type: 'object',
      properties: {
        slug: { type: 'string', description: 'Class slug or IRI' },
        depth: { type: 'number', description: 'Neighbourhood depth 1-3 (default 1)', default: 1 },
      },
      required: ['slug'],
      additionalProperties: false,
    },
  },
  {
    name: 'ontology_ask',
    description: 'Retrieval recipe: fuzzy-match the question to seed classes, fetch full records for the best matches, serialise a terse Turtle context block, clamp to a token budget. Use the returned "context" to ground reasoning.',
    inputSchema: {
      type: 'object',
      properties: {
        question: { type: 'string', description: 'Natural-language question or topic' },
        budget_tokens: { type: 'number', description: `Token budget for the context block (default ${DEFAULT_BUDGET}, range ${MIN_BUDGET}-${MAX_BUDGET})`, default: DEFAULT_BUDGET },
      },
      required: ['question'],
      additionalProperties: false,
    },
  },
];

const HANDLERS = {
  ontology_search: toolSearch,
  ontology_class_get: toolClassGet,
  ontology_neighbours: toolNeighbours,
  ontology_ask: toolAsk,
};

const server = new Server(
  { name: 'ontology-mcp', version: '1.0.0' },
  { capabilities: { tools: {} } },
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools: TOOLS }));

server.setRequestHandler(CallToolRequestSchema, async (req) => {
  const name = req.params.name;
  const args = req.params.arguments || {};
  const handler = HANDLERS[name];
  let result;
  if (!handler) {
    result = { error: 'unknown_tool', message: `no such tool: ${name}` };
  } else {
    try {
      result = await handler(args);
    } catch (err) {
      // Fail-open: an unexpected throw becomes a structured error result.
      result = { error: 'internal', message: err && err.message ? err.message : String(err) };
    }
  }
  return {
    content: [{ type: 'text', text: JSON.stringify(result, null, 2) }],
    isError: isErr(result),
  };
});

// Never crash the transport on stray async failures — log to stderr and carry on.
process.on('uncaughtException', (err) => {
  console.error('[ontology-mcp] uncaught:', err && err.stack ? err.stack : err);
});
process.on('unhandledRejection', (err) => {
  console.error('[ontology-mcp] unhandled rejection:', err && err.stack ? err.stack : err);
});

const transport = new StdioServerTransport();
await server.connect(transport);
console.error(`[ontology-mcp] ready (${backend.describe()})`);
