#!/usr/bin/env python3
"""Generic OpenAI-compatible A/B runner — reuses the diffusiongemma runner's
prompts, ontology grounding, and parser so every API model is measured identically.

Used for zai (glm-4.6), gemini-3.5-flash (frontier flash), gemini-2.5-flash-lite
(cheapest flash). Claude models run via the Workflow subagent path instead (no
scriptable Anthropic key here).

Env:
  API_ENDPOINT  e.g. https://api.z.ai/api/paas/v4
  API_MODEL     e.g. glm-4.6
  API_KEY       bearer token
  API_LABEL     output model label (e.g. zai, gemini-flash, gemini-flash-lite)
  API_MAX_TOKENS default 2048
Usage: run_api_model.py <gold.json> <out.jsonl> [reps]
"""
import json
import os
import re
import sys
import urllib.request

import run_diffusiongemma as base  # shared PROMPT / ontology_grounding / parse_predictions

ENDPOINT = os.environ["API_ENDPOINT"].rstrip("/")
MODEL = os.environ["API_MODEL"]
KEY = os.environ["API_KEY"]
LABEL = os.environ.get("API_LABEL", MODEL)
MAX_TOKENS = int(os.environ.get("API_MAX_TOKENS", "2048"))


def chat(prompt):
    body = json.dumps({
        "model": MODEL,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": MAX_TOKENS,
        "temperature": 0,
    }).encode()
    req = urllib.request.Request(ENDPOINT + "/chat/completions", data=body, headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {KEY}",
    })
    with urllib.request.urlopen(req, timeout=120) as r:
        j = json.load(r)
    m = j["choices"][0]["message"]
    # zai/gemini keep thinking in reasoning_content; content is the clean answer.
    return m.get("content") or ""


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
                p = base.PROMPT[qtype].format(c=concept)
                if arm == "aug":
                    if grounding is None:
                        grounding = base.ontology_grounding(concept)
                    p = f"Knowledge-graph grounding for this query:\n{grounding}\n\nUsing the grounding above, {p}"
                try:
                    preds = base.parse_predictions(chat(p))
                    err = None
                except Exception as e:
                    preds, err = [], str(e)
                rec = {"question_id": qid, "model": LABEL, "arm": arm, "rep": rep,
                       "predictions": preds, "n_pred": len(preds)}
                if err:
                    rec["error"] = err
                fh.write(json.dumps(rec) + "\n")
                fh.flush()
                n += 1
                sys.stderr.write(f"\r  [{LABEL}] {n}/{total} {qid[:20]:20} {arm} r{rep} -> {len(preds)}   ")
    sys.stderr.write("\n")
    fh.close()


if __name__ == "__main__":
    main()
