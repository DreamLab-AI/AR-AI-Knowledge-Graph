#!/usr/bin/env node
'use strict';

/*
 * diagram-index-gen.js — walk the diagrams-as-code tree (docs/diagrams), parse
 * per-file YAML frontmatter, validate every fenced ```mermaid block, and
 * (re)generate the machine-readable coverage indexes.
 *
 * Usage:  node scripts/diagram-index-gen.js <dir> [--check] [--render] [--jobs N] [--only <substr>]
 *
 *   <dir>       diagrams root (docs/diagrams). README.md / COVERAGE.md and the
 *               hero/ + archive/ subtrees are skipped.
 *   --check     validate frontmatter, heading ids and block structure only; do
 *               not write the indexes (still exits 1 on error).
 *   --cite-check resolve every `path:line` citation inside a diagram against the
 *               file's `sources:` list and assert the file is long enough; warns
 *               when the cited line is blank or a lone closing brace. Warnings
 *               only \u2014 it never fails the run.
 *   --render    additionally render every mermaid block through `mmdc` (the
 *               Mermaid CLI) into <dir>/rendered/<file>/<id>.svg; any parse
 *               error fails the run and is reported as file:block-id:line, and
 *               any render wider than 4500px fails as illegible.
 *   --jobs N    render concurrency (default 6).
 *   --only S    restrict to files whose relative path contains S.
 *
 * Exit codes: 0 ok, 1 validation/render error, 2 usage / IO error.
 *
 * Diagram-file contract (one file = one topic, many diagrams):
 *
 *   ---
 *   id: VC-03                      # area prefix + 2-digit number, unique
 *   title: REST request lifecycle
 *   area: visionclaw               # visionclaw | agentbox | estate
 *   governing:                     # governing docs (repo-relative paths, optional #anchor)
 *     - docs/IDENTITY-authority-chain.md
 *   adrs: [ADR-2009, ADR-2011]     # ledger records this file evidences
 *   sources:                       # repo-relative code paths the diagrams were verified against
 *     - src/main.rs
 *     - src/middleware/rbac_gate.rs
 *   verified_commit: b00c28a0d
 *   ---
 *   ## VC-03.1 GET /api/graph/data
 *   ```mermaid
 *   sequenceDiagram
 *   ...
 *   ```
 *
 * Every mermaid block must sit under an H2 whose first token is `<file-id>.<n>`;
 * that token is the diagram id and must be unique across the tree.
 * The generated README.md diagram table and COVERAGE.md are build artefacts.
 */

const fs = require('fs');
const path = require('path');
const os = require('os');
const { spawn } = require('child_process');

const AREAS = new Set(['visionclaw', 'agentbox', 'estate']);
const SKIP_DIRS = new Set(['hero', 'archive', 'rendered', 'src', 'upgraded', 'regen-2026-06-14', 'triptych-src', '.claude-flow', 'node_modules']);
const SKIP_FILES = new Set(['README.md', 'COVERAGE.md']);
const MAX_WIDTH = 4500; // px — wider renders are illegible at any zoom
const REQUIRED = ['id', 'title', 'area', 'governing', 'adrs', 'sources', 'verified_commit'];

function usage(msg) {
  if (msg) console.error(msg);
  console.error('Usage: node scripts/diagram-index-gen.js <dir> [--check] [--render] [--cite-check] [--jobs N] [--only S]');
  process.exit(2);
}

const argv = process.argv.slice(2);
if (argv.length < 1) usage();
const root = path.resolve(argv[0]);
const flags = { check: false, render: false, cite: false, jobs: 6, only: null };
for (let i = 1; i < argv.length; i++) {
  const a = argv[i];
  if (a === '--check') flags.check = true;
  else if (a === '--render') flags.render = true;
  else if (a === '--cite-check') flags.cite = true;
  else if (a === '--jobs') flags.jobs = parseInt(argv[++i], 10) || 6;
  else if (a === '--only') flags.only = argv[++i];
  else usage(`unknown flag ${a}`);
}
if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) usage(`not a directory: ${root}`);
const repoRoot = path.resolve(root, '..', '..');

// ---------------------------------------------------------------- walk
function walk(dir, out) {
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    if (ent.isDirectory()) {
      if (SKIP_DIRS.has(ent.name) || ent.name.startsWith('.')) continue;
      walk(path.join(dir, ent.name), out);
    } else if (ent.isFile() && ent.name.endsWith('.md')) {
      if (dir === root && SKIP_FILES.has(ent.name)) continue;
      if (dir === root) continue; // topic files live in area subdirs only
      out.push(path.join(dir, ent.name));
    }
  }
  return out;
}

// ---------------------------------------------------------------- yaml (minimal)
function parseScalar(s) {
  s = s.trim();
  if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) return s.slice(1, -1);
  return s;
}
function parseInline(s) {
  s = s.trim();
  if (s.startsWith('[') && s.endsWith(']')) {
    const inner = s.slice(1, -1).trim();
    if (!inner) return [];
    return inner.split(',').map(parseScalar).filter(Boolean);
  }
  return parseScalar(s);
}
function parseFrontmatter(text, file, errors) {
  if (!text.startsWith('---\n')) { errors.push(`${file}: missing frontmatter`); return null; }
  const end = text.indexOf('\n---', 4);
  if (end < 0) { errors.push(`${file}: unterminated frontmatter`); return null; }
  const block = text.slice(4, end).split('\n');
  const fm = {};
  let key = null;
  for (const raw of block) {
    if (!raw.trim() || raw.trim().startsWith('#')) continue;
    const m = raw.match(/^([A-Za-z_][A-Za-z0-9_]*):\s*(.*)$/);
    if (m) {
      key = m[1];
      fm[key] = m[2].trim() === '' ? [] : parseInline(m[2]);
    } else if (/^\s*-\s+/.test(raw) && key) {
      if (!Array.isArray(fm[key])) fm[key] = [];
      fm[key].push(parseScalar(raw.replace(/^\s*-\s+/, '')));
    } else {
      errors.push(`${file}: unparseable frontmatter line: ${raw}`);
    }
  }
  return { fm, body: text.slice(end + 4) };
}

// ---------------------------------------------------------------- parse topic files
function parseTopic(file, errors) {
  const rel = path.relative(root, file);
  const text = fs.readFileSync(file, 'utf8');
  const parsed = parseFrontmatter(text, rel, errors);
  if (!parsed) return null;
  const { fm, body } = parsed;
  for (const k of REQUIRED) if (!(k in fm)) errors.push(`${rel}: missing frontmatter field '${k}'`);
  for (const k of ['governing', 'adrs', 'sources']) if (k in fm && !Array.isArray(fm[k])) fm[k] = [fm[k]];
  if (fm.area && !AREAS.has(fm.area)) errors.push(`${rel}: area '${fm.area}' not in ${[...AREAS].join('|')}`);
  const areaDir = rel.split(path.sep)[0];
  if (fm.area && areaDir !== fm.area) errors.push(`${rel}: area '${fm.area}' does not match directory '${areaDir}'`);
  if (fm.id && !/^[A-Z]{2,3}-\d{2,3}$/.test(fm.id)) errors.push(`${rel}: id '${fm.id}' must match /^[A-Z]{2,3}-\\d{2,3}$/`);
  for (const s of fm.sources || []) {
    const p = s.split(':')[0];
    if (!fs.existsSync(path.join(repoRoot, p))) errors.push(`${rel}: source path does not exist: ${p}`);
  }
  for (const g of fm.governing || []) {
    const p = g.split('#')[0];
    if (!fs.existsSync(path.join(repoRoot, p))) errors.push(`${rel}: governing doc does not exist: ${p}`);
  }

  // headings + mermaid blocks
  const lines = body.split('\n');
  const diagrams = [];
  let currentH2 = null;
  let inFence = false, fenceLang = null, fenceStart = 0, buf = [];
  let proseLines = 0;
  for (let i = 0; i < lines.length; i++) {
    const ln = lines[i];
    if (!inFence) {
      const f = ln.match(/^```(\w*)/);
      if (f) { inFence = true; fenceLang = f[1]; fenceStart = i; buf = []; continue; }
      const h = ln.match(/^##\s+(\S+)\s*(.*)$/);
      if (h) { currentH2 = { id: h[1], title: h[2].trim(), line: i }; continue; }
      if (ln.startsWith('#')) continue;
      if (ln.trim() && !ln.trim().startsWith('<!--')) proseLines++;
    } else {
      if (ln.startsWith('```')) {
        inFence = false;
        if (fenceLang === 'mermaid') {
          if (!currentH2) errors.push(`${rel}: mermaid block at line ${fenceStart + 1} has no H2 heading`);
          else {
            const expect = new RegExp(`^${(fm.id || '').replace('-', '\\-')}\\.\\d+$`);
            if (!expect.test(currentH2.id)) errors.push(`${rel}: H2 id '${currentH2.id}' must be '${fm.id}.<n>'`);
            const src = buf.join('\n');
            for (const m of src.matchAll(/rect\s+rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/g)) {
              const lum = 0.299 * +m[1] + 0.587 * +m[2] + 0.114 * +m[3];
              if (lum < 140) errors.push(`${rel}:${currentH2.id} — dark rect fill rgb(${m[1]},${m[2]},${m[3]}) makes message text unreadable on the light render; use a pastel (luminance >= 140)`);
            }
            if (/^\s*(mindmap|pie|quadrantChart|timeline|journey)\b/m.test(src)) errors.push(`${rel}:${currentH2.id} — forbidden diagram kind (no information density)`);
            diagrams.push({ id: currentH2.id, title: currentH2.title, src, line: fenceStart + 1, kind: (buf[0] || '').trim().split(/\s/)[0] });
          }
        }
        continue;
      }
      buf.push(ln);
    }
  }
  if (inFence) errors.push(`${rel}: unterminated code fence`);
  if (diagrams.length === 0) errors.push(`${rel}: no mermaid diagrams`);
  if (proseLines > diagrams.length * 3) errors.push(`${rel}: ${proseLines} prose lines for ${diagrams.length} diagrams — this tree is diagrams-only (max 3 lines per diagram)`);
  return { file, rel, fm, diagrams };
}

// ---------------------------------------------------------------- citation check
// Every fact in this tree is anchored as a `path:line` (or `path:a-b`) inside a
// participant, message or Note. The path is usually written short (a basename or
// a trailing fragment), so resolve it against the file's own `sources:` list.
const CITE_RE = /([A-Za-z0-9_./-]*[A-Za-z0-9_-]\.[A-Za-z0-9]{1,12}):(\d+)(?:\s*-\s*(\d+))?/g;
function citeCheck(topics) {
  const warnings = [];
  const lineCache = new Map();
  const linesOf = (p) => {
    if (!lineCache.has(p)) {
      try { lineCache.set(p, fs.readFileSync(path.join(repoRoot, p), 'utf8').split('\n')); }
      catch { lineCache.set(p, null); }
    }
    return lineCache.get(p);
  };
  for (const t of topics) {
    for (const d of t.diagrams) {
      for (const m of d.src.matchAll(CITE_RE)) {
        const [, cited, a, b] = m;
        const hits = (t.fm.sources || []).filter((s) => {
          const sp = s.split(':')[0];
          return sp === cited || sp.endsWith('/' + cited);
        });
        if (hits.length !== 1) continue; // ambiguous or not a source of this file
        const src = hits[0].split(':')[0];
        const lines = linesOf(src);
        if (!lines) continue;
        // Both endpoints must exist; only the anchor line's CONTENT is judged —
        // a range legitimately ends on a closing brace.
        for (const n of [a, b].filter(Boolean).map(Number)) {
          if (n > lines.length) warnings.push(`${t.rel}:${d.id} — ${src}:${n} past EOF (file has ${lines.length} lines)`);
        }
        const n = Number(a);
        if (n <= lines.length) {
          const txt = (lines[n - 1] || '').trim();
          if (!txt) warnings.push(`${t.rel}:${d.id} — ${src}:${n} is blank`);
          else if (/^[)\]}>;,]+$/.test(txt)) warnings.push(`${t.rel}:${d.id} — ${src}:${n} is punctuation only ('${txt}')`);
        }
      }
    }
  }
  return warnings;
}

// ---------------------------------------------------------------- render
function renderOne(topic, d, outDir) {
  return new Promise((resolve) => {
    const mmd = path.join(outDir, `${d.id}.mmd`);
    const svg = path.join(outDir, `${d.id}.svg`);
    fs.writeFileSync(mmd, d.src + '\n');
    const child = spawn('mmdc', ['-i', mmd, '-o', svg, '-q'], { stdio: ['ignore', 'pipe', 'pipe'] });
    let err = '';
    child.stderr.on('data', (c) => { err += c.toString(); });
    child.stdout.on('data', (c) => { err += c.toString(); });
    child.on('close', (code) => {
      if (code === 0) {
        try {
          const svgText = fs.readFileSync(svg, 'utf8');
          const vb = svgText.match(/viewBox="[\d.\-]+ [\d.\-]+ ([\d.]+) ([\d.]+)"/);
          const w = vb ? Math.round(+vb[1]) : 0;
          if (w > MAX_WIDTH) return resolve(`${topic.rel}:${d.id} (md line ${d.line}) — rendered ${w}px wide (max ${MAX_WIDTH}); wrap long Notes with <br/>, cap them at ~90 chars, or split the diagram`);
        } catch (e) { /* ignore census failure */ }
        return resolve(null);
      }
      const m = err.match(/Parse error on line (\d+):[\s\S]*?\n([\s\S]*?)(?:\n\s+at |$)/);
      const detail = m ? `mermaid line ${m[1]}: ${m[2].split('\n').slice(0, 3).join(' | ')}` : err.split('\n').filter((l) => l.trim() && !/^\s+at /.test(l)).slice(0, 3).join(' | ');
      resolve(`${topic.rel}:${d.id} (md line ${d.line}) — ${detail}`);
    });
  });
}
async function renderAll(topics) {
  const jobs = [];
  for (const t of topics) {
    const outDir = path.join(root, 'rendered', t.rel.replace(/\.md$/, ''));
    fs.mkdirSync(outDir, { recursive: true });
    for (const d of t.diagrams) jobs.push(() => renderOne(t, d, outDir));
  }
  const errors = [];
  let next = 0;
  async function worker() {
    while (next < jobs.length) {
      const j = jobs[next++];
      const e = await j();
      if (e) errors.push(e);
    }
  }
  await Promise.all(Array.from({ length: Math.min(flags.jobs, jobs.length) }, worker));
  return { errors, count: jobs.length };
}

// ---------------------------------------------------------------- indexes
function mdLink(rel, anchor) {
  return anchor ? `${rel}#${anchor}` : rel;
}
function slug(id, title) {
  return `${id} ${title}`.toLowerCase().replace(/[^a-z0-9\s-]/g, '').trim().replace(/\s+/g, '-');
}
function writeIndexes(topics) {
  const byArea = { visionclaw: [], agentbox: [], estate: [] };
  for (const t of topics) byArea[t.fm.area].push(t);
  for (const k of Object.keys(byArea)) byArea[k].sort((a, b) => a.fm.id.localeCompare(b.fm.id, undefined, { numeric: true }));

  // README table (regenerated block between markers)
  const readme = path.join(root, 'README.md');
  let text = fs.existsSync(readme) ? fs.readFileSync(readme, 'utf8') : '';
  const START = '<!-- BEGIN GENERATED DIAGRAM INDEX -->', END = '<!-- END GENERATED DIAGRAM INDEX -->';
  const rows = [];
  for (const area of ['visionclaw', 'agentbox', 'estate']) {
    rows.push(`\n### ${area}\n`);
    rows.push('| ID | Topic | Diagrams | Kinds | Governing | ADRs |');
    rows.push('|----|-------|----------|-------|-----------|------|');
    for (const t of byArea[area]) {
      const kinds = [...new Set(t.diagrams.map((d) => d.kind))].join(', ');
      const gov = t.fm.governing.map((g) => `[${path.basename(g.split('#')[0])}](../../${g})`).join(', ');
      rows.push(`| ${t.fm.id} | [${t.fm.title}](${t.rel}) | ${t.diagrams.length} | ${kinds} | ${gov} | ${t.fm.adrs.join(', ')} |`);
    }
  }
  const total = topics.reduce((n, t) => n + t.diagrams.length, 0);
  const gen = `${START}\n_${topics.length} topic files, ${total} diagrams. Regenerate with_ \`node scripts/diagram-index-gen.js docs/diagrams\`.\n${rows.join('\n')}\n${END}`;
  if (text.includes(START) && text.includes(END)) {
    text = text.slice(0, text.indexOf(START)) + gen + text.slice(text.indexOf(END) + END.length);
  } else {
    text = text.trimEnd() + '\n\n## Diagram index\n\n' + gen + '\n';
  }
  fs.writeFileSync(readme, text);

  // COVERAGE.md — three inverted indexes
  const adrIdx = new Map(), govIdx = new Map(), srcIdx = new Map();
  for (const t of topics) {
    for (const a of t.fm.adrs) { if (!adrIdx.has(a)) adrIdx.set(a, []); adrIdx.get(a).push(t); }
    for (const g of t.fm.governing) { const k = g.split('#')[0]; if (!govIdx.has(k)) govIdx.set(k, []); govIdx.get(k).push(t); }
    for (const s of t.fm.sources) { const k = s.split(':')[0]; if (!srcIdx.has(k)) srcIdx.set(k, []); srcIdx.get(k).push(t); }
  }
  const sortKeys = (m) => [...m.keys()].sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  const out = [];
  out.push('<!-- GENERATED BY scripts/diagram-index-gen.js — DO NOT EDIT BY HAND -->');
  out.push('# Diagram coverage index\n');
  out.push(`${topics.length} topic files · ${total} diagrams · verified against commits: ${[...new Set(topics.map((t) => t.fm.verified_commit))].join(', ')}\n`);
  out.push('## Diagrams\n');
  out.push('| Diagram | Kind | Topic file |');
  out.push('|---------|------|------------|');
  for (const area of ['visionclaw', 'agentbox', 'estate']) for (const t of byArea[area]) for (const d of t.diagrams) out.push(`| [${d.id} ${d.title}](${t.rel}#${slug(d.id, d.title)}) | ${d.kind} | ${t.fm.id} |`);
  out.push('\n## By ADR\n');
  out.push('| ADR | Topic files |');
  out.push('|-----|-------------|');
  for (const k of sortKeys(adrIdx)) out.push(`| ${k} | ${adrIdx.get(k).map((t) => `[${t.fm.id}](${t.rel})`).join(', ')} |`);
  out.push('\n## By governing document\n');
  out.push('| Governing doc | Topic files |');
  out.push('|---------------|-------------|');
  for (const k of sortKeys(govIdx)) out.push(`| [${k}](../../${k}) | ${govIdx.get(k).map((t) => `[${t.fm.id}](${t.rel})`).join(', ')} |`);
  out.push('\n## By source path\n');
  out.push('| Source | Topic files |');
  out.push('|--------|-------------|');
  for (const k of sortKeys(srcIdx)) out.push(`| \`${k}\` | ${srcIdx.get(k).map((t) => `[${t.fm.id}](${t.rel})`).join(', ')} |`);
  fs.writeFileSync(path.join(root, 'COVERAGE.md'), out.join('\n') + '\n');
}

// ---------------------------------------------------------------- main
(async () => {
  const errors = [];
  let files = walk(root, []);
  if (flags.only) files = files.filter((f) => path.relative(root, f).includes(flags.only));
  const topics = files.map((f) => parseTopic(f, errors)).filter(Boolean);
  const seenTopic = new Map(), seenDiag = new Map();
  for (const t of topics) {
    if (seenTopic.has(t.fm.id)) errors.push(`${t.rel}: duplicate topic id ${t.fm.id} (also ${seenTopic.get(t.fm.id)})`);
    seenTopic.set(t.fm.id, t.rel);
    for (const d of t.diagrams) {
      if (seenDiag.has(d.id)) errors.push(`${t.rel}: duplicate diagram id ${d.id} (also ${seenDiag.get(d.id)})`);
      seenDiag.set(d.id, t.rel);
    }
  }
  const total = topics.reduce((n, t) => n + t.diagrams.length, 0);
  console.log(`parsed ${topics.length} topic files, ${total} mermaid diagrams`);
  if (flags.cite) {
    const w = citeCheck(topics);
    console.log(`cite-check: ${w.length} warning(s)`);
    for (const x of w) console.warn(`  ! ${x}`);
  }
  if (flags.render) {
    const { errors: rerr, count } = await renderAll(topics);
    console.log(`rendered ${count - rerr.length}/${count} diagrams via mmdc`);
    errors.push(...rerr);
  }
  if (errors.length) {
    console.error(`\n${errors.length} error(s):`);
    for (const e of errors) console.error(`  - ${e}`);
    process.exit(1);
  }
  if (!flags.check && !flags.only) {
    writeIndexes(topics);
    console.log('wrote README.md index block + COVERAGE.md');
  }
})().catch((e) => { console.error(e); process.exit(2); });
