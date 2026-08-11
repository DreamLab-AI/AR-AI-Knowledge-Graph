#!/usr/bin/env python3
"""Unit test for confidence-aware selective injection (Loom optimisation #2).

Runs with NO external index and NO network — it stubs the ScaffoldIndex.match
score and the section builder, so it exercises purely the gate + budget-scaling
logic. Because the feature is read from the environment at import time, each case
runs in its own subprocess with the relevant env set.

    python3 tests/test_confidence_injection.py     # direct
    pytest tests/test_confidence_injection.py       # or via pytest
"""
import os
import subprocess
import sys

APP = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "app")

# A tiny probe program executed per-case: stub the index + section builder, call
# scaffold(), and print the resulting grounding meta as `key=value` lines.
_PROBE = r'''
import os, ontology_scaffold as osc
class FakeIdx:
    def match(self, prompt, max_seeds=4):
        return [("alpha", float(os.environ["FAKE_SCORE"])), ("beta", 2.5)]
osc._section_for = lambda *a, **k: "SECTION"
meta = {}
block = osc.scaffold("q", budget_tokens=1000, index=FakeIdx(), meta_out=meta)
print("conf=%s" % osc.CONFIDENCE_INJECTION)
print("injected=%s" % meta.get("injected"))
print("eff_budget=%s" % meta.get("effective_budget"))
print("empty=%s" % (block == ""))
'''


def _run(env):
    e = dict(os.environ, PYTHONPATH=APP, **env)
    out = subprocess.check_output([sys.executable, "-c", _PROBE], env=e, text=True)
    return dict(line.split("=", 1) for line in out.strip().splitlines())


def test_disabled_by_default_is_unchanged():
    r = _run({"FAKE_SCORE": "8.0"})  # LOOM_CONFIDENCE_INJECTION unset
    assert r["conf"] == "False"
    assert r["injected"] == "True"
    assert r["eff_budget"] == "1000"  # full budget, byte-identical to legacy


def test_strong_match_keeps_full_budget():
    r = _run({"LOOM_CONFIDENCE_INJECTION": "1", "FAKE_SCORE": "8.0"})
    assert r["injected"] == "True"
    assert r["eff_budget"] == "1000"


def test_weak_match_scales_budget_down():
    r = _run({"LOOM_CONFIDENCE_INJECTION": "1", "FAKE_SCORE": "3.0"})
    assert r["injected"] == "True"
    assert r["eff_budget"] == "400"  # max(0.4, 3/8) * 1000


def test_below_threshold_skips_injection():
    r = _run({"LOOM_CONFIDENCE_INJECTION": "1", "FAKE_SCORE": "1.0"})
    assert r["injected"] == "False"
    assert r["empty"] == "True"


def _main():
    cases = [
        ("disabled-by-default (unchanged)", test_disabled_by_default_is_unchanged),
        ("strong match → full budget", test_strong_match_keeps_full_budget),
        ("weak match → scaled budget", test_weak_match_scales_budget_down),
        ("below threshold → skipped", test_below_threshold_skips_injection),
    ]
    failed = 0
    for name, fn in cases:
        try:
            fn()
            print(f"  PASS  {name}")
        except AssertionError as e:
            failed += 1
            print(f"  FAIL  {name} — {e}")
    print(f"\n{len(cases) - failed}/{len(cases)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    _main()
