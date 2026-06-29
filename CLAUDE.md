# Claude Code Configuration - Claude Flow V3

## MEMORY FIRST (Reinforced)

Before ANY task: search memory. After ANY success: store pattern. Non-negotiable.

```javascript
// Search before working
mcp__claude-flow__memory_search({query: "[task keywords]", namespace: "patterns", limit: 10})
// Store after success
mcp__claude-flow__memory_store({namespace: "patterns", key: "[name]", value: "[what worked]"})
```

ONLY MCP tools: `mcp__claude-flow__memory_store/search/list/retrieve/stats`
NEVER `claude-flow memory *` CLI.

## Auto-Learning Protocol

### Before Starting Any Task
```javascript
mcp__claude-flow__memory_search({query: "[task keywords]", namespace: "patterns", limit: 10})
mcp__claude-flow__memory_search({query: "[task type]", namespace: "tasks", limit: 5})
Bash("claude-flow hooks route --task '[task description]'")
```

### After Completing Any Task
```javascript
mcp__claude-flow__memory_store({namespace: "patterns", key: "[pattern-name]", value: "[what worked]"})
Bash("claude-flow hooks post-edit --file '[main-file]' --train-neural true")
Bash("claude-flow hooks post-task --task-id '[id]' --success true --store-results true")
```

### Memory-Enhanced Development
**Check memory before**: new features, debugging, refactoring, performance work
**Store after**: bug fixes, completions, performance fixes, security discoveries

### Improvement Workers

| Trigger | Worker |
|---------|--------|
| Major refactor | `optimize` |
| New features | `testgaps` |
| Security changes | `audit` |
| API changes | `document` |
| 5+ file changes | `map` |
| Complex debug | `deepdive` |

## Intelligent Skill Selection

Before starting any task, select the optimal orchestration approach. The decision
tree below routes tasks to the right skill based on scope, complexity, and domain.

### Quick Decision Tree

```
Don't know which skill? --> /route [describe your task]  (unified dispatcher)

Single file, quick fix? --> Direct Edit (no skill needed)
Game dev (Godot/Unity/Unreal)? --> /game-dev
Bug/feature/review (single agent)? --> lazy-fetch blueprints
Multi-file feature with TDD? --> /build-with-quality
Large codebase structural analysis (call graphs, arch, diff)? --> codebase-memory
Need current library docs while coding? --> context7
Swarm (3+ agents)? --> /swarm-advanced or hive-mind
GitHub ops (PR/release/CI)? --> github-* skills
Docs/reports? --> /report-builder, /docs-alignment
Image generation (infographics, headers, art)? --> /art (Nano Banana 2, 4K, text rendering, 16 workflows)
Media editing (resize/convert/composite)? --> /imagemagick, /ffmpeg, /blender, /comfyui
Browser automation? --> /playwright, /browser-automation, /qe-browser (QE-grade: typed assertions, visual-diff, injection scan)
AI/ML (PyTorch, CUDA, notebooks)? --> /pytorch-ml, /cuda, /jupyter-notebooks
Memory/AgentDB? --> /agentdb-*, /lazy-fetch
Wardley maps / strategic analysis? --> /wardley-maps, /report-builder
UI/UX design? --> /ui-ux-pro-max-skill, /bencium-*, /design-audit, /typography
Architecture review? --> /vanity-engineering-review, /renaissance-architecture, /human-architect-mindset
Ground reasoning in our formal ontology/KG ("what does our KG say about X", subgraph, SPARQL, governed writeback)? --> /ontology-augment (PRD-020 binding; ontology_ask, fail-open, budget-bounded)
Web search (ranked priority):
  1. Default / first try? --> /ceramic-search (keyword, rich 8k extracts, fast)
  2. Need authoritative sources, academic/policy filters, synthesized answer? --> /perplexity-research (secondary)
  3. Quick built-in fallback, no API key needed? --> Claude WebSearch (tertiary)
  Complex or important query? --> ALL THREE in parallel, dedupe + cross-verify
  Verifiable citations + lenses (academic/legal/clinical/SEC/patent)? --> /web-researcher (v1.33.0)
  Known URLs --> /gemini-url-context ; NotebookLM --> /notebooklm
Deep cited research (multi-agent fan-out + adversarial verify, orchestrates the above)? --> /deep-research (parallel agents, provenance, verification)
Optimize a metric iteratively? --> /autoresearch (experiment loop, keep/discard)
Add source verification? --> /provenance-tracking (.provenance.md sidecar)
Owner's personal email (invoices, threads, "did X email me", find a name)? --> /email-search (ask_email; local gateway, privacy-filtered; NOT work mail/calendar)
Security / compliance? --> /defense-security
AEC (building architecture)? --> /studio [task]
SEO / content optimisation? --> /toprank
```

Full routing with all 88 active skills: see `agentbox/skills/SKILL-DIRECTORY.md`

### Skill Capabilities Matrix

| Skill | Agents | Scope | Memory | Best For |
|-------|--------|-------|--------|----------|
| **lazy-fetch** | 1 (self) | Single session | RuVector bridge | Context discovery, plan tracking, blueprints, security scan |
| **game-dev** | 48 | Game project | Session state files | Game design, engine-specific code, team orchestration |
| **ruflo/claude-flow** | 1-15 | Multi-agent | RuVector native | Swarm coordination, complex features, cross-module work |
| **build-with-quality** | 111+ | QE pipeline | RuVector native | Testing, quality gates, coverage analysis |
| **sparc-methodology** | 5-8 | Full lifecycle | RuVector native | Specification through completion |

### Combining Skills

Skills compose. A single task may use multiple skills in sequence:

1. `lazy gather "combat system"` -- discover relevant files first
2. `/game-dev team-combat "melee attacks"` -- orchestrate the game dev team
3. `lazy check` -- validate the implementation
4. `lazy remember "combat" "melee uses hitbox detection, not raycasts"` -- persist

For ruflo swarms, each spawned agent uses worktree isolation:
- Agent spawns in worktree (`wt-add <name>` for Ruflo, `isolation: "worktree"` for Agent tool)
- Runs `lazy init` + `lazy gather` for its task scope
- Implements using lazy-fetch plan tracking
- Results merge back via ruflo coordination or git merge

For massive parallelisable work (migrations, bulk refactors), use `/batch` — it fans out to N worktree agents automatically.

### Session Start Protocol

1. Search RuVector memory for task context
2. Run `lazy read` if working in a lazy-fetch-initialised project
3. Check hooks route for skill recommendation
4. Select skill from decision tree above
5. Begin work

## Swarm Orchestration

### 3-Tier Model Routing (ADR-026)

| Tier | Handler | Latency | Cost | Use Cases |
|------|---------|---------|------|-----------|
| **1** | Agent Booster | <1ms | $0 | Simple transforms -- Skip LLM |
| **2** | Haiku | ~500ms | $0.0002 | Simple tasks, low complexity |
| **3** | Sonnet/Opus | 2-5s | $0.003-0.015 | Complex reasoning, security |

`[AGENT_BOOSTER_AVAILABLE]` -> Edit tool directly
`[TASK_MODEL_RECOMMENDATION]` -> use recommended model in Task tool

### Agent Routing

| Code | Task | Agents |
|------|------|--------|
| 1 | Bug Fix | coordinator, researcher, coder, tester |
| 3 | Feature | coordinator, architect, coder, tester, reviewer |
| 5 | Refactor | coordinator, architect, coder, reviewer |
| 7 | Performance | coordinator, perf-engineer, coder |
| 9 | Security | coordinator, security-architect, auditor |
| 11 | Docs | researcher, api-docs |

### Task Complexity

**Swarm**: 3+ files, new features, cross-module, API+tests, security, performance, DB schema
**Skip**: single file, 1-2 line fix, docs, config, questions

## Project Config

| Setting | Value |
|---------|-------|
| Topology | hierarchical-mesh |
| Max Agents | 15 |
| Strategy | specialized |
| Consensus | raft |
| Memory | hybrid |
| HNSW | Enabled |
| Neural | Enabled |

## Tool Delineation

**Task tool**: ALL execution (agents, files, code, git)
**CLI (Bash)**: `claude-flow swarm init`, `claude-flow hooks *`, `claude-flow agent spawn`
**MCP (MANDATORY)**: `mcp__claude-flow__memory_{store,search,list,retrieve}`

## RuVector PostgreSQL

See `agentbox/CLAUDE.md` for full RuVector connection details, schema,
SQL examples, and extension capabilities. Summary:

- Host: `ruvector-postgres:5432` | DB: `ruvector` | Connection: `$RUVECTOR_PG_CONNINFO`
- ALWAYS use MCP tools (`mcp__claude-flow__memory_*`), never CLI
- Tables: `memory_entries` (1.17M+), `patterns`, `reasoning_patterns`, `session_state`

## Hive-Mind

Topologies: `hierarchical`, `mesh`, `hierarchical-mesh` (recommended), `adaptive`
Strategies: `byzantine` (f<n/3), `raft` (f<n/2), `gossip`, `crdt`, `quorum`

## Session Persistence
```bash
claude-flow session restore --latest                                              # Start
claude-flow hooks session-end --generate-summary true --persist-state true        # End
```

## Codebase Memory MCP (ACTIVE — USE FIRST)

`codebase-memory-mcp` is indexed for this project (`home-devuser-workspace-project`, 48,159 nodes, 95,766 edges). Use these tools BEFORE Grep/Glob/Read for all structural queries.

| Query Type | Tool | Instead of |
|---|---|---|
| Who calls function X? | `trace_path(function_name, direction="callers")` | Grep |
| Module/route architecture | `get_architecture(project, aspects=[...])` | Manual file reading |
| Find struct/fn by name | `search_graph(project, query)` | Glob |
| Git diff impact + risk score | `detect_changes(project)` | Manual inspection |
| Fetch function source | `get_code_snippet(qualified_name)` | Read |
| Check if index is current | `index_status(project)` | — |
| Re-index after large changes | `index_repository(repo_path)` | — |

**Project ID**: `home-devuser-workspace-project`
**Session start**: run `index_status` — if `needs_reindex: true`, run `index_repository`.
**Token savings**: ~99.2% vs grep (3,400 vs 412,000 tokens for 5 structural queries).
