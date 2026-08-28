#!/usr/bin/env bash
# Dream-cycle evaluator: sanity-check the cargo metadata dump produced by
# the build step (/tmp/vc-meta.json). Checked-in script because the annexe
# ssh dispatch strips nested double quotes from inline entrypoints.
set -u
python3 - <<'PY'
import json
m = json.load(open('/tmp/vc-meta.json'))
pk = [p['name'] for p in m['packages']]
print('crates:', len(pk))
print('xr-presence:', 'visionclaw-xr-presence' in pk)
PY
