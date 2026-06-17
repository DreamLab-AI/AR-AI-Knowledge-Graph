#!/usr/bin/env python3
"""Deterministic KG-as-oracle grader (PRD-020 ontology-augment eval).

Rebuilt from REPORT_DATA.md spec. Grades a model's predicted localnames for each
gold question via token-set greedy 1:1 matching (singularised), then aggregates
recall@K / precision / F1 / hallucination by model, by question-type, and overall
— matching the summary-mm.json shape so the LaTeX report consumes it unchanged.

Matching rule (per question):
  - normalise each term: lowercase, split on non-alnum, drop a trailing 's'
    (crude singularisation), as a token SET.
  - a predicted term MATCHES a gold term iff their token sets overlap on the
    shorter set's length (subset-or-equal) OR share >= 2 tokens. Greedy 1:1:
    each gold can be claimed once, each prediction claims at most one gold.
  - recall@K: cap predictions to K (12 for neighbour/subclass) before matching;
    existence uses any-match recall (did the model assert the right localname).
  - precision = matched / predictions_considered ; F1 = harmonic mean ;
    hallucination = unmatched_predictions / predictions_considered.

Input JSONL (one row per cell):  {question_id, model, arm, rep, predictions:[...]}
  arm ∈ {aug, ctl}.  gold.json supplies the gold per question_id (index or seed).

Usage: grade.py <gold.json> <predictions.jsonl> [out_summary.json] [out_rows.csv]
"""
import json
import sys
import csv
import re
from collections import defaultdict

RECALL_K = 12


def toks(term):
    parts = [p for p in re.split(r"[^a-z0-9]+", str(term).lower()) if p]
    return frozenset(p[:-1] if len(p) > 3 and p.endswith("s") else p for p in parts)


def matches(pred_t, gold_t):
    if not pred_t or not gold_t:
        return False
    inter = len(pred_t & gold_t)
    if inter == 0:
        return False
    shorter = min(len(pred_t), len(gold_t))
    return inter >= shorter or inter >= 2


def grade_cell(predictions, gold, qtype):
    preds = list(predictions)
    if qtype in ("neighbour", "subclass"):
        preds = preds[:RECALL_K]
    pred_t = [toks(p) for p in preds]
    gold_t = [toks(g) for g in gold]
    claimed = set()
    matched = 0
    for pt in pred_t:
        for gi, gt in enumerate(gold_t):
            if gi in claimed:
                continue
            if matches(pt, gt):
                claimed.add(gi)
                matched += 1
                break
    n_pred = max(1, len(preds))
    n_gold = max(1, len(gold))
    if qtype == "existence":
        recall = 1.0 if matched >= 1 else 0.0
    else:
        recall = matched / n_gold
    precision = matched / n_pred
    f1 = (2 * precision * recall / (precision + recall)) if (precision + recall) else 0.0
    halluc = (len(preds) - matched) / n_pred
    return {"recall": recall, "precision": precision, "f1": f1, "halluc": halluc}


def main():
    gold_path, pred_path = sys.argv[1], sys.argv[2]
    out_summary = sys.argv[3] if len(sys.argv) > 3 else "/dev/stdout"
    out_rows = sys.argv[4] if len(sys.argv) > 4 else None

    goldj = json.load(open(gold_path))
    gold_items = goldj if isinstance(goldj, list) else goldj.get("questions") or goldj.get("items")
    gold_by_id = {}
    for i, it in enumerate(gold_items):
        for key in (it.get("seed"), it.get("label"), it.get("concept"), str(i)):
            if key:
                gold_by_id[str(key)] = it
        gold_by_id[i] = it  # numeric index too

    rows = []
    with open(pred_path) as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            r = json.loads(line)
            qid = r.get("question_id")
            it = gold_by_id.get(qid) if not isinstance(qid, int) else gold_items[qid]
            if it is None:
                it = gold_by_id.get(str(qid))
            if it is None:
                continue
            g = grade_cell(r.get("predictions", []), it.get("gold", []), it.get("type", "neighbour"))
            rows.append({"model": r.get("model", "?"), "arm": r.get("arm", "?"),
                         "type": it.get("type", "neighbour"), "qid": str(qid), **g})

    def agg(filt):
        sel = [r for r in rows if filt(r)]
        if not sel:
            return None
        n = len(sel)
        return {k: round(sum(r[k] for r in sel) / n, 4) for k in ("f1", "recall", "precision", "halluc")}

    models = sorted({r["model"] for r in rows})
    types = sorted({r["type"] for r in rows})
    summary = {"by_model": {}, "by_type": {}, "overall": {}}
    for m in models:
        a = agg(lambda r, m=m: r["model"] == m and r["arm"] == "aug")
        c = agg(lambda r, m=m: r["model"] == m and r["arm"] == "ctl")
        summary["by_model"][m] = {
            "aug": (a or {}).get("f1"), "ctl": (c or {}).get("f1"),
            "delta": round(((a or {}).get("f1", 0) - (c or {}).get("f1", 0)), 4),
            "aug_halluc": (a or {}).get("halluc"), "ctl_halluc": (c or {}).get("halluc"),
        }
    for t in types:
        a = agg(lambda r, t=t: r["type"] == t and r["arm"] == "aug")
        c = agg(lambda r, t=t: r["type"] == t and r["arm"] == "ctl")
        summary["by_type"][t] = {"aug": (a or {}).get("f1"), "ctl": (c or {}).get("f1"),
                                 "delta": round(((a or {}).get("f1", 0) - (c or {}).get("f1", 0)), 4)}
    ao = agg(lambda r: r["arm"] == "aug")
    co = agg(lambda r: r["arm"] == "ctl")
    summary["overall"] = {"aug": (ao or {}).get("f1"), "ctl": (co or {}).get("f1"),
                          "delta": round(((ao or {}).get("f1", 0) - (co or {}).get("f1", 0)), 4),
                          "aug_halluc": (ao or {}).get("halluc"), "ctl_halluc": (co or {}).get("halluc"),
                          "n_rows": len(rows)}
    json.dump(summary, open(out_summary, "w"), indent=2)
    if out_rows:
        with open(out_rows, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=["model", "arm", "type", "qid", "f1", "recall", "precision", "halluc"])
            w.writeheader()
            w.writerows(rows)
    sys.stderr.write(f"graded {len(rows)} cells; {len(models)} models; overall aug={summary['overall']['aug']} ctl={summary['overall']['ctl']} Δ={summary['overall']['delta']}\n")


if __name__ == "__main__":
    main()
