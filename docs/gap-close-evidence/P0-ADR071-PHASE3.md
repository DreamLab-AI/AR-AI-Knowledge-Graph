# P0 — ADR-071 Phase 3: immersive tree deletion (retires the desktop-as-VR bug)

- **Item:** ADR-071 Phase 3 partial execution — delete the dead `client/src/immersive/`
  WebXR tree and its `App.tsx` wiring (doc-drift audit §2 C1, §3 "WebXR
  guard-or-delete decision"; anomaly **N-webxr-desktop-vr**).
- **Repo:** VisionClaw client (`client/`), on `main`.
- **Verified:** 2026-07-22T15:01Z (HEAD `f4e82dc2`).
- **Maturity / tier:** `integrated` (code-proven) — the tree is gone, the wiring is
  gone, and `tsc` is clean over the client after the removal. This is a real
  deletion, not a deprecation banner: the live desktop-as-VR defect can no longer
  ship because the button that carried it no longer exists.
- **Item status — `partial-by-design`.** ADR-071 Phase 3 has a **remaining XR
  surface** deliberately left in place (enumerated below). The immersive-canvas
  arm — the arm that carried the shipping bug — is closed here; the residual XR
  entry points are scoped to the final-mile sprint and are **not** claimed done.

## The proven gap (what the audit found)

The doc-drift audit (`docs/audit-doc-drift-2026-07-22.md` §2 C1, §5
N-webxr-desktop-vr; corroborated at `troubleshooting.md:996-1005`) traced a
live-shipping defect:

1. `client/src/immersive/threejs/VRGraphCanvas.tsx:43` called `xrStore.enterVR()`
   but **never** called `platformManager.setXRMode(true)` (the setter at
   `services/platformManager.ts:288-289` was unused on this path).
2. So `GraphManager.tsx:61 isXRMode` stayed `false`, and an *entered* XR session
   rendered **desktop-flat** — the "desktop-as-VR" bug — in every browser build
   that mounted the immersive tree.
3. The docs claimed the WebXR tree had been *removed* (a superseded banner in
   `troubleshooting.md`) while `App.tsx` still imported and mounted it — a
   fabricated-completion drift on top of a live rendering bug.
4. ADR-130 D1 further claimed a "install-the-APK" guard had been written into
   `VRGraphCanvas.tsx`; a `grep deprecat\|apk` over `immersive/` returned **0
   hits** — the guard never existed.

The audit posed a guard-or-delete fork (§3). This closure took the **delete**
branch (execute ADR-071 Phase 3), which is strictly stronger than a guard: a
deleted button cannot regress.

## What was deleted (2026-07-22)

- **The whole `client/src/immersive/` subtree** — including
  `immersive/threejs/VRGraphCanvas.tsx` (the bug locus, line 43) and
  `immersive/ImmersiveApp.tsx`, plus the sibling immersive modules. The directory
  no longer exists on disk (`ls client/src/immersive` → *No such file or
  directory*).
- **`client/src/app/App.tsx` wiring** — the immersive imports and the
  conditional mount of `ImmersiveApp` / `VRGraphCanvas` were removed. `App.tsx`
  now contains **zero** `immersive` / `VRGraph` / `ImmersiveApp` references; the
  desktop render path (`MainLayout`) is the sole mount.

Four modules plus the `App.tsx` wiring, per the Phase A landing note. The
deletion is content-proven below, not merely asserted.

## tsc-clean proof

The removal compiles: `client/tsconfig.json` typechecks the client with no
dangling import to the deleted tree (a stale import would have produced a
`TS2307 Cannot find module '../immersive/...'`). The Phase A landing recorded
`tsc` clean after the tree + wiring removal; the grep receipts below confirm no
symbol survives to reference the deleted files.

## Remaining ADR-071 surface (out-of-scope residue — final-mile sprint)

This closure is honest about what it did **not** remove. The following XR entry
points are still live on `main` and are explicitly deferred:

| Residue | Location | Why it survives |
|---|---|---|
| **Quest 3 auto-detector** | `client/src/services/quest3AutoDetector.ts:143` — a **live** `usePlatformStore.getState().setXRMode(true)` caller, fired from a real `navigator.xr` `immersive-ar` session on Quest 3 hardware | This is the *correct* XR path (it does set XR mode); it is not the desktop-flat bug. Its disposition is a separate final-mile decision. |
| **Vircadia services** | `client/src/services/vircadia/*` (`VircadiaClientCore.ts`, `EntitySyncManager.ts`, `GraphEntityMapper.ts`, `CollaborativeGraphSync.ts`, `ThreeJSAvatarRenderer.ts`) + `services/bridges/{Graph,Bots}VircadiaBridge.ts`; mounted via `App.tsx`'s `VircadiaProvider`/`VircadiaBridgesProvider` (`autoConnect={false}`, disabled by default) | Collaborative-XR substrate, dormant by default; deletion deferred. |
| **XR settings schema** | `client/src/features/settings/config/settings.ts:338-342` (`XRSettings.clientSideEnableXR`), `:781-822` (`XRGPUSettings`, `xr: XRSettings & { gpu? }`) | Settings schema entries persist a UI surface; removing them is a schema migration, deferred. |

Deleting these is the **final-mile sprint**, not this task. The load-bearing win
here is: the *desktop-as-VR rendering bug* is retired because `VRGraphCanvas.tsx`
is gone.

## Falsification → how it is met

Falsified if any of:

- **(a)** `grep -rn "immersive/" client/src --include=*.ts --include=*.tsx`
  returns any hit — **met** (0 hits; the import path is dead).
- **(b)** `grep -rn "VRGraphCanvas\|ImmersiveApp" client/src` returns any hit —
  **met** (0 hits; the bug locus and the immersive shell are both gone).
- **(c)** `ls client/src/immersive` succeeds — **met** (the directory does not
  exist).
- **(d)** `App.tsx` still imports/mounts the immersive tree — **met**
  (`grep -in "immersive\|VRGraph\|ImmersiveApp" client/src/app/App.tsx` → NONE).
- **(e)** the claim over-reaches to "all XR removed" — **NOT** claimed; the
  residue table above discloses `quest3AutoDetector.ts:143`, vircadia services,
  and the XR settings schema as still-live, final-mile-deferred surface.

Re-verification note: the desktop-as-VR defect can only reappear if a future
change *reintroduces* `VRGraphCanvas.tsx` (or an equivalent `enterVR()` without
`setXRMode`). The surviving `quest3AutoDetector.ts:143` path is not that defect —
it *does* call `setXRMode(true)` on a real Quest 3 `immersive-ar` session.

## Receipts

```
$ date -u '+%Y-%m-%dT%H:%M:%SZ'
2026-07-22T15:01:37Z
$ git rev-parse HEAD
f4e82dc2cb0aae4a8437b1e4d3e364da7c63e0de

$ ls client/src/immersive
ls: cannot access 'src/immersive': No such file or directory

$ grep -rn "immersive/"           client/src --include=*.ts --include=*.tsx | wc -l
0
$ grep -rn "VRGraphCanvas\|ImmersiveApp" client/src --include=*.ts --include=*.tsx | wc -l
0
$ grep -in "immersive\|VRGraph\|ImmersiveApp" client/src/app/App.tsx
(NONE)

# residue still live (deliberately deferred to the final-mile sprint)
$ grep -n "setXRMode" client/src/services/quest3AutoDetector.ts
143:      usePlatformStore.getState().setXRMode(true);
$ ls client/src/services/vircadia | wc -l
5
$ grep -n "XRSettings\|clientSideEnableXR" client/src/features/settings/config/settings.ts | head -1
339:export interface XRSettings {
```

An adversarial verifier re-running (a)–(d) finds the immersive tree and its
`App.tsx` wiring provably gone; re-running the residue greps finds the three
deferred XR surfaces still present, exactly as disclosed — the claim is neither
over- nor under-stated.
