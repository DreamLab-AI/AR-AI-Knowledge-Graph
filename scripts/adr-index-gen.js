#!/usr/bin/env node
'use strict';

/*
 * adr-index-gen.js — walk an ADR docs tree, parse YAML frontmatter, validate,
 * and (re)generate a README.md index table from the frontmatter.
 *
 * Usage:  node scripts/adr-index-gen.js <dir> [--check]
 *
 *   <dir>     directory to walk recursively for *.md ADR files
 *   --check   validate only; do not write README.md (still exits 1 on error)
 *
 * Exit codes:
 *   0  all validations passed (README written unless --check)
 *   1  one or more validation errors (README not written)
 *   2  usage / IO error
 *
 * The generated README.md is a build artefact — never hand-edit it.
 */

const fs = require('fs');
const path = require('path');

const REQUIRED_FIELDS = [
  'id', 'title', 'date', 'decision_status', 'implementation_status',
  'activation_status', 'supersedes', 'superseded_by', 'verified_commit',
  'owner', 'review_trigger', 'repo',
];

const ENUMS = {
  decision_status: ['proposed', 'accepted', 'rejected', 'superseded'],
  implementation_status: ['none', 'partial', 'complete'],
  activation_status: ['inactive', 'staged', 'live'],
  repo: ['visionclaw', 'agentbox'],
};

// Files that are templates/skeletons: validated for structure but excluded
// from the reciprocity graph and the emitted index (id === 'ADR-NNNN').
const TEMPLATE_ID = 'ADR-NNNN';

function fail(msg) { console.error(`error: ${msg}`); }
function warn(msg) { console.warn(`warning: ${msg}`); }

// --- minimal YAML frontmatter parser (flat scalars + inline [a, b] lists) ---
function parseFrontmatter(text, file) {
  const m = /^---\r?\n([\s\S]*?)\r?\n---/.exec(text);
  if (!m) return null;
  const body = m[1];
  const out = {};
  for (const rawLine of body.split(/\r?\n/)) {
    // strip trailing comments (only when not inside quotes — simple heuristic)
    let line = rawLine;
    if (!/["']/.test(line)) line = line.replace(/\s+#.*$/, '');
    line = line.replace(/\s+$/, '');
    if (line.trim() === '') continue;
    const km = /^([A-Za-z_][\w-]*):\s?(.*)$/.exec(line);
    if (!km) continue;
    const key = km[1];
    let val = km[2].trim();
    if (val === '') { out[key] = ''; continue; }
    if (val.startsWith('[') && val.endsWith(']')) {
      const inner = val.slice(1, -1).trim();
      out[key] = inner === '' ? [] : inner.split(',').map(s => s.trim().replace(/^["']|["']$/g, '')).filter(Boolean);
      continue;
    }
    val = val.replace(/^["']|["']$/g, '');
    out[key] = val;
  }
  return out;
}

function walk(dir, acc) {
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) walk(p, acc);
    else if (ent.isFile() && ent.name.endsWith('.md') && ent.name !== 'README.md') acc.push(p);
  }
  return acc;
}

function main() {
  const args = process.argv.slice(2);
  const check = args.includes('--check');
  const dir = args.find(a => !a.startsWith('--'));
  if (!dir) { fail('usage: adr-index-gen.js <dir> [--check]'); process.exit(2); }
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
    fail(`not a directory: ${dir}`); process.exit(2);
  }

  const files = walk(dir, []);
  const records = [];
  let errors = 0;

  for (const file of files) {
    const rel = path.relative(dir, file);
    const text = fs.readFileSync(file, 'utf8');
    const fm = parseFrontmatter(text, file);
    if (!fm) { fail(`${rel}: no YAML frontmatter block`); errors++; continue; }

    for (const field of REQUIRED_FIELDS) {
      if (!(field in fm)) { fail(`${rel}: missing required field '${field}'`); errors++; }
    }
    // The template skeleton (id ADR-NNNN) must carry every key but is exempt
    // from value-level checks (empty scalars, enum membership) — it is a form.
    const isTemplate = fm.id === TEMPLATE_ID;
    if (!isTemplate) {
      // required scalars must be non-empty (list fields may legitimately be empty)
      for (const field of REQUIRED_FIELDS) {
        if (field === 'supersedes' || field === 'superseded_by') continue;
        if (field in fm && String(fm[field]).trim() === '') {
          fail(`${rel}: required field '${field}' is empty`); errors++;
        }
      }
      for (const [field, allowed] of Object.entries(ENUMS)) {
        if (field in fm && fm[field] !== '' && !allowed.includes(fm[field])) {
          fail(`${rel}: '${field}' = '${fm[field]}' not in {${allowed.join(', ')}}`); errors++;
        }
      }
    }
    for (const lf of ['supersedes', 'superseded_by']) {
      if (lf in fm && !Array.isArray(fm[lf])) {
        fail(`${rel}: '${lf}' must be a list, e.g. [] or [ADR-0042]`); errors++;
      }
    }
    records.push({ file, rel, fm });
  }

  // --- supersedes reciprocity (skip templates) ---
  const real = records.filter(r => r.fm.id && r.fm.id !== TEMPLATE_ID);
  const byId = new Map();
  for (const r of real) {
    if (byId.has(r.fm.id)) { fail(`duplicate id '${r.fm.id}' (${r.rel} and ${byId.get(r.fm.id).rel})`); errors++; }
    else byId.set(r.fm.id, r);
  }
  for (const r of real) {
    for (const target of (r.fm.supersedes || [])) {
      const t = byId.get(target);
      if (!t) { warn(`${r.rel}: supersedes '${target}' which is not present in this tree`); continue; }
      if (!(t.fm.superseded_by || []).includes(r.fm.id)) {
        warn(`reciprocity: ${r.fm.id} supersedes ${target}, but ${target}.superseded_by does not list ${r.fm.id}`);
      }
    }
    for (const src of (r.fm.superseded_by || [])) {
      const s = byId.get(src);
      if (!s) { warn(`${r.rel}: superseded_by '${src}' which is not present in this tree`); continue; }
      if (!(s.fm.supersedes || []).includes(r.fm.id)) {
        warn(`reciprocity: ${r.fm.id} superseded_by ${src}, but ${src}.supersedes does not list ${r.fm.id}`);
      }
    }
  }

  if (errors > 0) {
    fail(`${errors} validation error(s); README not generated`);
    process.exit(1);
  }

  // --- emit README.md ---
  const rows = real.slice().sort((a, b) => a.fm.id.localeCompare(b.fm.id, undefined, { numeric: true }));
  const esc = s => String(s == null ? '' : s).replace(/\|/g, '\\|');
  const listCell = v => (Array.isArray(v) && v.length ? v.join(', ') : '—');

  let md = '<!-- GENERATED BY scripts/adr-index-gen.js — DO NOT EDIT BY HAND -->\n\n';
  md += '# Architecture Decision Records\n\n';
  md += `_${rows.length} record(s). Regenerate with_ \`node scripts/adr-index-gen.js ${dir}\`.\n\n`;
  md += '| ID | Title | Date | Decision | Impl | Activation | Supersedes | Superseded by | Owner | Repo |\n';
  md += '|----|-------|------|----------|------|------------|------------|---------------|-------|------|\n';
  for (const r of rows) {
    const f = r.fm;
    const link = `[${esc(f.id)}](${encodeURI(r.rel)})`;
    md += `| ${link} | ${esc(f.title)} | ${esc(f.date)} | ${esc(f.decision_status)} | ${esc(f.implementation_status)} | ${esc(f.activation_status)} | ${esc(listCell(f.supersedes))} | ${esc(listCell(f.superseded_by))} | ${esc(f.owner)} | ${esc(f.repo)} |\n`;
  }

  const outPath = path.join(dir, 'README.md');
  if (check) {
    console.log(`ok: ${rows.length} ADR(s) valid (--check, README not written)`);
  } else {
    fs.writeFileSync(outPath, md);
    console.log(`ok: ${rows.length} ADR(s) valid; wrote ${path.relative(process.cwd(), outPath)}`);
  }
  process.exit(0);
}

main();
