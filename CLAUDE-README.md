# CLAUDE-README.md — Boot Guide (Claude Code v4 + Claude Flow Alpha)

## 1. CORE DIRECTIVES
1. **Prod-ready only** – ship runnable code; no TODOs or stubs.
2. **Token-efficient reasoning** – think silently; output final answer unless asked.
3. **Edge-case first** – list boundaries → write tests → implement.
4. **Selective context pull** – fetch only needed paths + line nums in large repos.
5. **Primary tools:**
   - `claude-flow` - PRIMARY devtool (alpha version)
   - `bash -c "<cmd>"`
   - `python - <<EOF … EOF` (Py 3.12 default)
   - `ruv-swarm` MCP (supporting agent orchestration)
6. **No hedging / flattery / chit-chat.**

## 2. ENVIRONMENT SNAPSHOT

| Layer | Key Items |
|-------|-----------|
| **HW** | Auto-detected CPU/RAM / NVMe / NVIDIA GPUs |
| **Python 3.12** | `torch` 2.7+ + CUDA 12.9, `tensorflow`, `keras`, `xgboost`, `wgpu` |
| **Python 3.13** | Clean sandbox (`pip`, `setuptools`, `wheel`) |
| **CUDA + cuDNN** | `/usr/local/cuda` ready |
| **Rust** | `rustup`, `clippy`, `cargo-edit`, `sccache` |
| **Node 22 LTS** | PRIMARY: `claude-flow@alpha`, supporting: `ruv-swarm`, `revit-mcp`, `vite`, `typescript`, `eslint`, `prettier`, `jest`, `storybook` |
| **Linters** | `shellcheck`, `flake8`, `pylint`, `hadolint` |
| **LaTeX** | `texlive-full`, `latexmk`, `chktex` |
| **Extras** | `tmux`, `hyperfine`, `docker`, `WasmEdge`, `OpenVINO`, `Modular MAX`, `Blender`, `uv` |
| **Web UI** | Claude-Flow UI → **http://localhost:3000** (PRIMARY INTERFACE) |

### Quick env switches:
```bash
source /opt/venv312/bin/activate   # ML env
source /opt/venv313/bin/activate   # Py 3.13 sandbox
```

## 3. PRIMARY TOOLS & SYNTAX

### 3.1 Claude-Flow Alpha (PRIMARY)
```bash
# Start orchestrator (auto-started in container)
claude-flow start --daemon --port 3000 --health-check-port 3000

# Agent management
claude-flow agent spawn researcher --name "Research Bot"
claude-flow agent spawn implementer --name "Code Bot"
claude-flow agent list --status active

# Task management
claude-flow task create implementation "Build REST API with auth"
claude-flow task create research "Research microservices patterns"
claude-flow task status <task-id>

# Claude instance spawning (integrated)
claude-flow claude spawn "implement user auth" --mode backend-only
claude-flow claude spawn "build dashboard" --research --parallel

# Memory and monitoring
claude-flow memory query --category research --limit 10
claude-flow monitor --interval 2
claude-flow status --detailed
```
**Web UI at port 3000 provides full interface for job management**

### 3.2 ruv-swarm MCP (SUPPORTING)
```javascript
mcp__ruv-swarm__swarm_init       { topology:"hierarchical", maxAgents:5, enableNeural:true }
mcp__ruv-swarm__agent_spawn      { type:"coder", model:"tcn-detector", pattern:"convergent" }
mcp__ruv-swarm__task_orchestrate { task:"Build REST API", strategy:"adaptive" }
```

### 3.3 Blender Headless Environment
- **Version**: Blender 4.5 LTS
- **Mode**: Headless (`--background`)
- **MCP Connection**: A custom MCP server runs inside Blender, allowing for bi-directional communication.
- **Startup**: The Blender instance is started automatically by the `entrypoint.sh` script when the container starts.

Example workflow:
```bash
# The entrypoint.sh script handles the startup of Blender.
# You can interact with Blender via MCP tools.
# For example, to get scene info:
mcp__blender__get_scene_info

# To execute a python script in blender:
mcp__blender__execute_blender_code 'import bpy; bpy.ops.mesh.primitive_cube_add()'
```

### 3.4 LaTeX Environment
- **Full Distribution**: `texlive-full` is installed.
- **Build Tool**: Use `latexmk` for easy compilation.
- **Linter**: Use `chktex` to check for common errors.

Example workflow in `/home/dev/latex`:
```bash
# Create a simple document
echo '\documentclass{article}\begin{document}Hello, LaTeX!\end{document}' > mydoc.tex

# Compile it
latexmk -pdf mydoc.tex

# Clean up auxiliary files
latexmk -c
```

## 4. STRUCTURED OUTPUT CONTRACT
Return one of:
- **diff** – patch for `git apply`
- **bash** – shell commands
- **text** – plain explanation

Always end with **✅ Done** so orchestrator can detect completion.

## 5. CODING & STYLE STANDARDS
- **Language picks:** Py 3.12 for ML; Rust for perf; Node 22 for CLIs.
- **Tests:** `pytest -q`, `cargo test`, edge cases mandatory.
- **Formatters:** `black`, `ruff`, `rustfmt`, `prettier`.
- **Docs:** Public APIs need docstrings + one example.
- **Forbidden:** long apologies, empty sections, unexplained "maybe".

## 6. COMMON WORKFLOWS

| Goal | One-liner |
|------|-----------|
| Start Claude-Flow | `claude-flow start --daemon --health-check-port 3000` |
| Spawn agents | `claude-flow agent spawn implementer --name "Code Bot"` |
| Create tasks | `claude-flow task create research "AI trends analysis"` |
| Spawn Claude instance | `claude-flow claude spawn "build API" --mode backend-only` |
| Monitor system | `claude-flow monitor --interval 2` |
| Query memory | `claude-flow memory query --category research` |

## 7. TMUX SESSION HINTS
- **List sessions:** `tmux ls`
- **Attach to claude-flow:** `tmux attach -t claude-flow`
- **Attach to mcp:** `tmux attach -t mcp`
- **New window:** `tmux new-window -t <name>`

## 8. CONTAINER SETUP
- **Port 3000:** Claude-Flow web UI (primary interface)
- **Data directory:** `/home/dev/data/claude-flow`
- **Workspace:** `/workspace` (mounted from host)
- **External files:** `/workspace/ext` (mounted from host)
- **LaTeX projects:** `/home/dev/latex` (mounted from host)

## 9. UPDATE RULES
- Keep file ≤ 350 lines; prune every quarter.
- Update Environment Snapshot on image changes.
- Append new claude-flow commands when alpha versions update.

---

### Next steps
1. Access web UI at http://localhost:3000
2. Use `claude-flow status` to check system health
3. Spawn agents with `claude-flow agent spawn <type>`
4. Create tasks with `claude-flow task create <type> "description"`
