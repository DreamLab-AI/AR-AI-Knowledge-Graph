# Ontology-Augment Eval — in-flight data ledger (2026-06-14)

## Objective
Measure the value of the PRD-020/ADR-112 ontology binding on LLM recall/accuracy.
A/B: ontology-**augmented** vs **control** (parametric-only), across **opus** & **sonnet**.

## KG-as-oracle ground truth
Gold derived from the live KG (the oracle), not human judgement:
- concept→IRI via `POST /api/ontology-agent/discover`
- neighbours/subclasses via SPARQL over `GRAPH <urn:ngm:graph:ontology:assert>`
  (predicates: enables 11450, relatedTo 10016, requires 9156, hasPart 9071,
  uses 7509, subClassOf 6056, supports 4350, implements, dependsOn, …)

## Design
- 16 questions: 8 neighbour-recall, 4 subclass, 4 existence. Mix of familiar tech
  (control can partly guess) + DreamLab-niche (control cannot).
- Isolation: each cell = a fresh `general-purpose` subagent given ONLY the question.
  AUG arm grounds via the `ontology-augment` CLI (`ontology_ask`); control uses
  parametric knowledge only (no tools/lookup). 3 reps/cell → 16×2×2×3 = 192 runs.
- Grader (deterministic): token-set greedy 1:1 match (singularised), recall@12 for
  neighbour/subclass, any-match recall for existence, precision, F1, hallucination.
  Smoke-validated: simulated AUG F1 0.83 vs ctl 0.50; non-saturated.

## Qualitative pattern (observed across waves 1–3)
AUG recovers the KG's *exact* localnames; control gives plausible-but-generic terms
that miss the KG vocabulary (→ high hallucination):
- ZKP: AUG {zk-snarks, zk-starks, bulletproofs, trusted-setup, zk-rollup} vs
  ctl {fiat-shamir-heuristic, prover, verifier, soundness, completeness}.
- Gaussian splatting: AUG {gpu-rasterisation, adaptive-density-control,
  ssim-loss, photorealistic-telepresence} vs ctl {neural-radiance-field,
  multi-view-stereo, volumetric-rendering}.
- multi-agent-system: AUG {sandboxed-code-execution, model-context-protocol,
  orchestration-protocol} vs ctl {agent-spawning, swarm-topology, message-passing}.

## Cost (in-flight timing sample, n=41 → timing_sample.csv)
AUG mean ≈ 36.8k tokens / 41.0s (5–10 tool calls);
ctl mean ≈ 29.8k tokens / 14.8s (3–4 tool calls).
Grounding costs ~24% more tokens and ~2.8× wall-clock — the cost side of the delta.

## Known nuance (a Phase A target)
`ontology_ask` expand returns OUTGOING triples only. Subclass-**children** are
incoming (`?child subClassOf seed`), so the CLI may under-serve subclass questions
unless the agent surfaces children via the discover seed list. Expect a smaller/
weaker AUG delta on the 4 subclass items → candidate skill improvement (document
graph_query SPARQL for incoming edges).

## Artefacts
- gold.json (16 Q + gold), evals/evals.json
- workspace/iteration-1/<name>/<model>/{with_skill,without_skill}/r<rep>/output.json
- grade.py, batch_grade.py, timing_sample.csv
