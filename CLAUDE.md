# Claude Code Configuration — Claude Flow V3

## Memory first

Search RuVector memory before starting non-trivial work and store what worked afterwards — that is how patterns propagate between agents and sessions:

```javascript
mcp__claude-flow__memory_search({query: "[task keywords]", namespace: "patterns", limit: 10})   // before
mcp__claude-flow__memory_store({namespace: "patterns", key: "[name]", value: "[what worked]"})  // after success
```

Memory access is MCP-only (`mcp__claude-flow__memory_*`); the `claude-flow memory *` CLI bypasses the embedding pipeline. Connection details, schema, and operational gotchas: `agentbox/CLAUDE.md`.

## Structural queries: codebase-memory MCP

The project is indexed as `home-devuser-workspace-project` (48k nodes / 96k edges). For structural questions use it before Grep/Glob/Read — `trace_path` (callers/callees), `get_architecture`, `search_graph`, `detect_changes` (diff impact), `get_code_snippet`. At session start run `index_status`; re-run `index_repository` if `needs_reindex: true`. It answers "who calls X / what depends on Y" in ~1% of the tokens grep needs.

## Skill routing

Skills self-trigger from their descriptions; the full directory is `agentbox/skills/SKILL-DIRECTORY.md`, and `/route [task]` dispatches when unsure. Routes that aren't obvious from descriptions alone:

- Web search priority: `/ceramic-search` first, `/perplexity-research` for authoritative/academic needs, built-in WebSearch as fallback; run all three in parallel for important queries. `/deep-research` orchestrates multi-agent cited research on top.
- Ontology/KG grounding ("what does our KG say about X", SPARQL, governed writeback): `/ontology-augment`.
- Owner's personal email: `/email-search` (local privacy-filtered gateway — not work mail/calendar).
- Multi-agent work: swarm for 3+ files / cross-module features (`/swarm-advanced`, hive-mind); skip orchestration for single-file fixes, docs, and questions. For massive parallel work (migrations, bulk refactors) use `/batch` — worktree isolation per agent.

## Session lifecycle

```bash
claude-flow session restore --latest                                        # start
claude-flow hooks session-end --generate-summary true --persist-state true  # end
```
