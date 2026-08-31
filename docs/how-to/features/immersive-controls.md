---
title: Immersive Controls
description: How to drive the VisionClaw graph in the headset and on the desktop — two-hand grip manipulation, grab-to-pin, the node menu and radial, in-graph search, fly locomotion, the fold ladder, the visual query builder, and the swarm teleport roster.
category: how-to
tags:
  - xr
  - controls
  - godot
  - desktop
  - interaction
updated-date: 2026-08-31
difficulty-level: beginner
---

# Immersive Controls

This is the how-to for *driving* the graph once it is in front of you — in the
headset (Vive-style wands or Quest controllers) and on the desktop. For getting
the headset client installed and connected, see
[Quest 3 & Desktop-VR Setup](../xr-quest3-setup.md).

The XR client is a Godot project under `xr-client/scripts/`. The bindings below
are the shipping ones; the in-app **Help** tab carries the same cheat-sheet
verbatim (`xr-client/scripts/hud.gd:554`).

---

## The wand cheat-sheet (verbatim)

These are the exact bindings shown on the HUD **Help** tab under
"Vive Wand — Controls":

### Node

| Do this | Result |
|---|---|
| Trigger — point at a node & pull | Grab it |
| …release the trigger | Pins the node in place |
| Trigger — double-pull on a node | Open its page card |
| Menu button (or A/X) | Node menu (mark variable…) |

### Graph

| Do this | Result |
|---|---|
| BOTH grips (two hands) | Seize the whole graph |
| hands apart / together | Scale |
| twist your hands | Rotate |
| move hands together | Carry |

### Panel & move

| Do this | Result |
|---|---|
| One grip while near this panel | Pick up & move the panel |
| Trackpad / thumbstick | Fly through the graph |
| Point at the panel + trigger | Click a button |

Quest Touch controllers map onto the same scheme: **trigger** = point-and-pull,
**grip** = seize/panel-pickup, **A/X** = the node menu, **thumbstick** = fly.

---

## Grabbing and pinning nodes

Point the wand at a node and **pull the trigger** to grab it — it follows your
hand. **Release the trigger** and the node stays where you dropped it: releasing
*pins* it. Pinned nodes are held out of the physics simulation until you unpin
them. The **Pins** tab lists everything you have pinned this session and offers
**Unpin All**, which hands the nodes back to physics (`hud.gd`, pins page).

## The page card (double-pull)

**Double-pull the trigger** on a node to open its **page card** — the detail
panel for that node.

## The node menu and marking variables

Press the **Menu button (or A/X)** while pointing at a node to open its **node
menu** (`xr-client/scripts/graph_scene.gd:314`). This is where **Mark as ?vN**
lives — the entry point to the [visual query builder](#visual-query-builder).
The menu is a radial: aim at an item and pull the trigger to select it.

## The radial menu

The node menu is one instance of the general **radial menu**
(`xr-client/scripts/radial_menu.gd`), opened with **Menu / A / X** and selected
by **point + trigger**. It also carries query-builder toggles such as
**Edges: concrete / any** and **Execute**.

## In-graph search

Press the **Menu button on empty space** (not aimed at a node) to open the
**top-labels search radial** (`graph_scene.gd:365`, `graph_scene.gd:2740`) — an
in-graph way to jump to a node by its label without leaving immersion.

## Two-hand graph manipulation

Squeeze **both grips** to *seize the whole graph* with two hands
(`graph_scene.gd:1564`–`1614`, ported from Graph2VR's `SphereInteraction`).
Then:

- **hands apart / together** — scale the graph up or down;
- **twist your hands** — rotate it;
- **move both hands together** — carry it.

## Fly locomotion and moving the panel

Push the **trackpad / thumbstick** to **fly through the graph**. To reposition
the HUD panel itself, hold **one grip while near the panel** and move it.

---

## The fold ladder

The **fold ladder** collapses and expands detail so a large graph stays legible.
It lives on the **Graph** tab under "Hierarchy & View" as **Fold +** and
**Fold -** (`hud.gd`, `_build_graph_page`). There are four levels
(`graph_scene.gd:229`–`236`):

- **L0** — everything visible;
- **L1** — hide low-signal nodes;
- **L2** — fold subclass chains into representatives;
- **L3** — community fold.

Each step **GETs `/api/graph/fold`** and applies the returned plan to the render
store. Fold level is *local view state* — it is not routed to the server, so two
people in the same session can hold different fold levels
(`graph_scene.gd:229`–`233`). See
[REST API Reference §GET /api/graph/fold](../../reference/rest-api.md#get-apigraphfold).

---

## Visual query builder

Mark nodes as `?vN` variables from the node menu; the visible edges between
marked nodes become a triple pattern that runs against
`POST /api/graph/query/pattern`. The **Query** tab shows the pattern, a live
match count, and **Execute** / **Clear**. Full walkthrough:
[Building Graph Queries](natural-language-queries.md).

---

## Swarm tab and teleport

The **Swarm** tab is the roster of live agents working the graph
(`hud.gd`, `_build_swarm_page`): a status dot, a `name → target-node` label, and
the agent's current task. **Tap a roster row to teleport** to that agent — the
button emits `control_pressed("teleport:<agent_id>")` (`hud.gd:471`), and because
agent wire ids *are* node ids, the existing glide-to-node path carries you there.

---

## Desktop controls

The desktop web client (`client/src/`) exposes the same graph over mouse and
keyboard. Three interactions are the desktop analogue of the wand:

1. **Click-to-focus / fly.** Selecting a node flies the camera to it. This runs
   through `client/src/features/visualisation/cameraFocus.ts` — `focusNodeById`
   dispatches the established `visionclaw:search` focus event, which
   `useGraphSelection` reads to move the camera. *(Client-side behaviour; verify
   against your build.)*
2. **Additive expansion.** The node context menu
   (`client/src/features/graph/components/NodeContextMenu.tsx`) fetches
   `GET /api/graph/node/{id}/relations`, lists **"Expand: &lt;label&gt; (N)"** per
   predicate and direction, and on click **POSTs `/expand` and additively merges**
   the returned neighbours into the current view. If the neighbours are already
   present it shows **"Already expanded — no new nodes"**.
3. **Page-card panel.** Opening a node's detail surfaces its page card, the
   desktop counterpart of the XR double-pull card. *(Client-side; exact trigger
   varies by build — confirm in your client.)*

The expansion and relations calls are wired in `client/src/api/graphExpandApi.ts`
against the same handlers the XR client uses.

---

## Related documentation

- [Quest 3 & Desktop-VR Setup](../xr-quest3-setup.md) — install and connect.
- [Building Graph Queries](natural-language-queries.md) — the query builder.
- [REST API Reference](../../reference/rest-api.md) — `/expand`, `/relations`,
  `/fold`, `/query/pattern`, `/layout/*`.
- [REST API Usage Guide](../rest-api-usage.md) — worked curl flows for the same
  endpoints.
</content>
