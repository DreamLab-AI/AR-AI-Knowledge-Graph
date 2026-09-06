---
title: Quest 3 & Desktop-VR Setup
description: Install and connect the VisionClaw Godot XR client — side-load the native APK onto a Meta Quest 3 or run the same project on a desktop-tethered VIVE-style headset, verify the backend, and get into an immersive graph session.
category: how-to
difficulty-level: beginner
updated-date: 2026-08-31
related:
  - docs/how-to/features/immersive-controls.md
  - docs/explanation/xr-architecture.md
  - docs/prd/PRD-008-xr-godot-replacement.md
  - docs/adr/ADR-071-godot-rust-xr-replacement.md
---

# Quest 3 & Desktop-VR Setup

The VisionClaw XR client is a **Godot 4.3 + OpenXR** project under
`xr-client/` (`xr-client/project.godot`, `config/features` pins `"4.3"` and
`"Forward Mobile"`). It ships to two surfaces from the *same* project:

- **Standalone Quest 3** — a side-loaded native APK
  (`package/unique_name="uk.xrsystems.visionclaw.xr"`, `xr-client/export_presets.cfg`).
- **Desktop-tethered VIVE-style headset** — run the project from Godot on a
  workstation with an OpenXR runtime (SteamVR / Monado); the wands drive the
  same scene.

The browser-hosted WebXR path no longer exists. Getting *in* is covered here;
once you are in, the controls live in
[Immersive Controls](features/immersive-controls.md).

---

## What you connect to

The client opens two WebSockets and issues REST calls against the same backend
(`xr-client/scripts/graph_scene.gd:67`–`69`):

| Purpose | Path | Default base |
|---|---|---|
| Graph stream | `/wss` | `ws://localhost:4000` (`DEFAULT_BACKEND_WS`) |
| Presence stream | `/ws/presence` | same base |
| REST (fold, layout, expand, query) | `/api/…` | derived HTTP base |

`4000` is the direct backend port; a deployment behind nginx typically fronts
it on `3001`. Point the client at whichever your deployment exposes.

---

## Prerequisites

| Requirement | Notes |
|---|---|
| A VisionClaw backend reachable from the client | Must serve `/wss`, `/ws/presence` and `/api/*` |
| **Quest path:** Meta Quest 3 (Horizon OS ≥ 71), developer mode enabled | Quest 2 works at reduced refresh |
| **Quest path:** `adb` on your workstation | `apt install android-tools-adb` / `brew install android-platform-tools` |
| **Desktop-VR path:** VIVE/Index-class headset + OpenXR runtime | SteamVR or Monado |
| **Build-from-source:** Godot 4.3 stable + Android export templates | Only if you are not handed a pre-built APK |

---

## Step 1 — Verify the backend

Both sockets must answer before the client will show a graph.

```bash
# Graph stream
curl -k -i -H "Connection: Upgrade" -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" \
  -H "Sec-WebSocket-Key: $(openssl rand -base64 16)" \
  http://your-host:4000/wss
# Expected: HTTP/1.1 101 Switching Protocols (or 401 if auth required)

# Presence stream
curl -k -i -H "Connection: Upgrade" -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" \
  -H "Sec-WebSocket-Key: $(openssl rand -base64 16)" \
  http://your-host:4000/ws/presence
# Expected: HTTP/1.1 101 (or 401)
```

If `/ws/presence` returns 404 the server is missing
`src/handlers/presence_handler.rs` — rebuild the backend container.

---

## Step 2 — Desktop-VR path (fastest to try)

1. Start your OpenXR runtime (SteamVR / Monado) and put the headset on.
2. Open `xr-client/` in Godot 4.3 and press **Play**, or export a desktop
   build. The boot scene (`xr-client/scripts/xr_boot.gd`) initialises the
   OpenXR session.
3. Point the client at your backend (default `ws://localhost:4000`; override
   for a remote host).

This path needs no APK and no `adb`, and is the quickest way to exercise the
controls. Jump to [Step 5 — First session](#step-5--first-session).

---

## Step 3 — Build the Quest APK (skip if handed a release APK)

If you are pulling a release APK from CI, skip to Step 4.

```bash
# Install Godot 4.3 stable and the Android export templates
godot --headless --download-export-templates

# Headless export using the committed preset
mkdir -p xr-client/export
godot --headless --path xr-client \
  --export-release "Quest 3 arm64" export/visionclaw-xr.apk

ls -lh xr-client/export/visionclaw-xr.apk
```

The export preset (`xr-client/export_presets.cfg`) already pins the package name
`uk.xrsystems.visionclaw.xr`. Building the native code and export templates is
detailed in the XR architecture reference below; the command above is the
end-to-end shape.

---

## Step 4 — Side-load onto the Quest

1. Enable **Developer Mode** on the Quest via the Meta Quest mobile app, then
   connect over USB-C and approve **Allow USB debugging?** in the headset.
2. Install:

```bash
adb devices                                   # confirm the headset shows as 'device'
adb install -r xr-client/export/visionclaw-xr.apk
```

`-r` reinstalls over an existing copy, preserving user data. If you hit
`INSTALL_FAILED_UPDATE_INCOMPATIBLE`, the existing install has a different
signing key — `adb uninstall uk.xrsystems.visionclaw.xr` then install fresh.

Launch from the **Apps** drawer, filtering by **Unknown Sources**.

---

## Step 5 — First session

On boot (`xr_boot.gd`) the client probes OpenXR, connects `/wss` and
`/ws/presence`, and renders the first graph frame. When you see the graph and
the HUD panel, you are in.

- The **HUD panel** is the wand-facing control surface with tabs
  **Graph · Query · Pins · Swarm · Session · Help** (`hud.gd:155`).
- The **Help** tab shows the wand cheat-sheet — the same bindings documented in
  [Immersive Controls](features/immersive-controls.md).

Confirm the basics: point-and-pull the trigger to grab a node, release to pin
it, and squeeze both grips to seize and scale the whole graph.

### Multi-user presence

A second headset joining the same session appears as an avatar via the
`/ws/presence` stream. If presence never populates, re-check Step 1 — a red
presence indicator almost always means `/ws/presence` is 401/404.

### Voice

Voice is provided by the deployment's voice plane rather than configured in the
XR client itself; see the [voice documentation](../../agentbox/voice/README.md) for the
operator console and room wiring.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `adb install` → `INSTALL_FAILED_VERIFICATION_FAILURE` | Verify-apps-over-USB blocking a dev-signed APK | `adb shell settings put global verifier_verify_adb_installs 0`, retry |
| `INSTALL_FAILED_UPDATE_INCOMPATIBLE` | Existing install signed with a different key | `adb uninstall uk.xrsystems.visionclaw.xr`, reinstall |
| No graph appears | `/wss` unreachable or auth failing | Re-run Step 1; check the backend base URL |
| Red presence indicator | `/ws/presence` returning 401/404 | Verify `presence_handler.rs` is mounted (Step 1) |
| Nothing happens on trigger/grip | Controllers not bound to the OpenXR profile | Restart the runtime; confirm the headset is tracking controllers |
| Desktop-VR: black view | OpenXR runtime not running before launch | Start SteamVR/Monado, then Play in Godot |

### Useful logcat filters (Quest)

```bash
adb logcat -s visionclaw-xr                       # all client events
adb logcat -s visionclaw-xr | grep -E 'ws|auth'   # network + auth
```

---

## See also

- [Immersive Controls](features/immersive-controls.md) — the full wand and
  desktop binding reference.
- [Building Graph Queries](features/natural-language-queries.md) — the visual
  query builder.
- [XR Architecture](../explanation/xr-architecture.md) — Godot + OpenXR design,
  boot sequence, frame loop.
- [PRD-008 — XR Client Replacement](../archive/prd/PRD-008-xr-godot-replacement.md).
- [ADR-071 — Godot 4 + godot-rust + OpenXR](../archive/adr/ADR-071-godot-rust-xr-replacement.md).
</content>
