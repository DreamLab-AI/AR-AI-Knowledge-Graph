#!/usr/bin/env bash
# Dream-cycle evaluator: the OpenXR action map must exist and be non-empty.
# Checked-in script because the annexe ssh dispatch strips nested double
# quotes from inline entrypoints.
set -u
echo "openxr actions: $(grep -c OpenXRAction xr-client/openxr_action_map.tres)"
test -s xr-client/openxr_action_map.tres && echo MAP-OK || echo MAP-MISSING
