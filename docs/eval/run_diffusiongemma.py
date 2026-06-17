#!/usr/bin/env python3
"""diffusiongemma ± ontology pre-sampling runner (PRD-020 eval, local-vs-frontier).

The headline test: does a LOCAL model (DiffusionGemma 26B-A4B) inherit the same
KG-grounding advantage the frontier models showed (AUG F1 0.812 vs CTL 0.369)?

For each gold question × {aug, ctl} × reps:
  - ctl: ask diffusiongemma the question with parametric knowledge only.
  - aug: pre-sample the ontology (ontology_ask CLI → KG subgraph triples),
         inject as grounding, then ask the same question.
Predictions (kebab-case localnames) are written as JSONL for grade.py, which
matches them against the KG-oracle gold exactly as for the frontier models.

DiffusionGemma serialises requests, so this MUST run when the re-index is done
(the server is single-context). Resumable: skips (question,arm,rep) cells already
in the output JSONL.

Usage: run_diffusiongemma.py <gold.json> <out_predictions.jsonl> [reps]
Env: DG_ENDPOINT (default http://192.168.2.48:8084/v1), DG_MODEL, DG_N_BLOCKS,
     ONTOLOGY_ASK (path to ontology-ask.cjs).
"""
import json
import os
import re
import sys
import subprocess
import urllib.request

DG_ENDPOINT = os.environ.get("DG_ENDPOINT", "http://192.168.2.48:8084/v1").rstrip("/")
DG_MODEL = os.environ.get("DG_MODEL", "diffusiongemma-26B-A4B-it-Q8_0")
DG_N_BLOCKS = int(os.environ.get("DG_N_BLOCKS", "4"))
ONTOLOGY_ASK = os.environ.get(
    "ONTOLOGY_ASK",
    os.path.expanduser("~/.claude/skills/ontology-augment/scripts/ontology-ask.cjs"),
)

PROMPT = {
    "neighbour": "Name the knowledge-graph concepts DIRECTLY connected to '{c}' — its neighbours via relations like enables, requires, uses, supports, dependsOn, hasPart. Reply with ONLY a comma-separated list of kebab-case localnames (e.g. consensus-mechanism, merkle-tree). No prose, no explanation.",
    "subclass": "Name the subclasses / member concepts that fall UNDER the '{c}' category in the knowledge graph. Reply with ONLY a comma-separated list of kebab-case localnames. No prose.",
    "existence": "In a formal knowledge graph, give the canonical kebab-case localname(s) for the concept '{c}'. Reply with ONLY the localname(s), comma-separated. No prose.",
}

def slugify(seg):
    s = re.sub(r"[^a-z0-9]+", "-", seg.strip().lower()).strip("-")
    return s


def ontology_grounding(concept):
    """Run ontology_ask and compress its Turtle into a compact triple grounding."""
    try:
        out = subprocess.run(
            ["node", ONTOLOGY_ASK, concept, "--tier", "sonnet", "--mode", "expand"],
            capture_output=True, text=True, timeout=60,
        ).stdout
    except Exception as e:
        return f"(grounding unavailable: {e})"
    triples, decls = [], []
    for ln in out.splitlines():
        m = re.search(r"class:([a-z0-9-]+)>\s+<[^>]*#(\w+)>\s+<urn:ngm:class:([a-z0-9-]+)>", ln)
        if m:
            triples.append(f"{m.group(1)} {m.group(2)} {m.group(3)}")
        elif re.search(r"<urn:ngm:class:([a-z0-9-]+)>\s+a\s+owl:Class", ln):
            decls.append(re.search(r"class:([a-z0-9-]+)>", ln).group(1))
    g = ""
    if decls:
        g += "Related classes: " + ", ".join(dict.fromkeys(decls)) + "\n"
    if triples:
        g += "Relations:\n" + "\n".join(triples[:40])
    return g or "(no grounding found)"


def strip_thinking(text):
    markers = list(re.finditer(r"<\|channel\|?>?\s*\w+", text, re.I))
    if markers:
        text = text[markers[-1].end():]
    return text.strip()


def dg_chat(prompt):
    body = json.dumps({
        "model": DG_MODEL,
        "messages": [{"role": "user", "content": prompt}],
        "n_blocks": DG_N_BLOCKS, "seed": 0, "stream": False,
    }).encode()
    req = urllib.request.Request(DG_ENDPOINT + "/chat/completions", data=body,
                                headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=180) as r:
        j = json.load(r)
    return j["choices"][0]["message"].get("content", "")


def parse_predictions(text):
    clean = strip_thinking(text)
    # The prompt asks for a comma-separated list. Take the answer lines, split on
    # commas/newlines/semicolons, slugify each segment to a localname. This keeps
    # single-word localnames (bulletproofs, gas, mev, plonk) AND multi-word ones
    # (trusted-setup), and normalises "Trusted Setup" -> "trusted-setup".
    lines = [l for l in clean.splitlines() if l.strip()]
    blob = "\n".join(lines[-4:]) if lines else clean
    segs = re.split(r"[,\n;]+", blob)
    seen, out = set(), []
    FILLER = re.compile(r"^\s*(the|a|an|and|or|are|is|its|their|such as|including|e\.g\.?|like)\b[\s:]*", re.I)
    for seg in segs:
        seg = FILLER.sub("", seg)
        # drop prose segments (a localname is <= 4 words; longer = a sentence)
        if len(seg.split()) > 4:
            continue
        s = slugify(seg)
        if not s or len(s) < 2 or s.replace("-", "").isdigit():
            continue
        if s not in seen:
            seen.add(s)
            out.append(s)
    return out[:20]


def main():
    gold_path, out_path = sys.argv[1], sys.argv[2]
    reps = int(sys.argv[3]) if len(sys.argv) > 3 else 3
    gold = json.load(open(gold_path))

    done = set()
    if os.path.exists(out_path):
        for ln in open(out_path):
            try:
                r = json.loads(ln)
                done.add((r["question_id"], r["arm"], r["rep"]))
            except Exception:
                pass

    fh = open(out_path, "a")
    total = len(gold) * 2 * reps
    n = len(done)
    for q in gold:
        qid, concept, qtype = q["seed"], q["concept"], q["type"]
        grounding = None
        for arm in ("ctl", "aug"):
            for rep in range(reps):
                if (qid, arm, rep) in done:
                    continue
                base = PROMPT[qtype].format(c=concept)
                if arm == "aug":
                    if grounding is None:
                        grounding = ontology_grounding(concept)
                    prompt = f"Knowledge-graph grounding for this query:\n{grounding}\n\nUsing the grounding above, {base}"
                else:
                    prompt = base
                try:
                    text = dg_chat(prompt)
                    preds = parse_predictions(text)
                    err = None
                except Exception as e:
                    preds, err = [], str(e)
                rec = {"question_id": qid, "model": "diffusiongemma", "arm": arm,
                       "rep": rep, "predictions": preds, "n_pred": len(preds)}
                if err:
                    rec["error"] = err
                fh.write(json.dumps(rec) + "\n")
                fh.flush()
                n += 1
                sys.stderr.write(f"\r  {n}/{total}  {qid[:24]:24} {arm} r{rep} -> {len(preds)} preds   ")
    sys.stderr.write("\n")
    fh.close()


if __name__ == "__main__":
    main()
