# ontology-mcp

Standalone, portable stdio MCP server for the NarrativeGoldmine ontology.
Extracted from the proven agentbox ontology machinery (`ontology-bridge.js`,
`lib/ontology-retrieval.js`, `lib/ontology-budget.js`) into a single-file
server with exactly one dependency (`@modelcontextprotocol/sdk`).

Fail-open by design: every tool returns structured `{ "error": "...", "message": "..." }`
strings on failure; the process never crashes on a bad fetch, bad file, or bad argument.

## Install & run

```bash
npm install
node index.js            # PUBLIC mode against https://narrativegoldmine.com
```

Or after `npm install -g .` / `npm link`: `ontology-mcp`.

Requires Node >= 18 (uses global `fetch`).

## Transports (selected by env)

| Env var | Mode | Behaviour |
|---|---|---|
| *(none set)* | **PUBLIC** (default) | Reads `https://narrativegoldmine.com` — `GET /api/search-index.json` (fetched once, cached in-process, fuzzy-matched locally) and `GET /api/pages/<slug>.json` per class (cached). |
| `ONTOLOGY_SITE=<url>` | **PUBLIC** | Same, against a different site deploy. |
| `ONTOLOGY_INDEX=<path>` | **LOCAL** | Loads a `scaffold-index.json` from disk once and serves everything from memory. **Takes precedence over `ONTOLOGY_SITE`.** |
| `ONTOLOGY_TIMEOUT_MS=<ms>` | both | HTTP fetch timeout in PUBLIC mode (default 10000). |

### LOCAL index schema (`scaffold-index.json`, version 1)

```json
{
  "version": 1,
  "generated": "2026-08-11T00:00:00Z",
  "counts": { "classes": 2 },
  "classes": {
    "knowledge-graph": {
      "t": "Knowledge Graph",
      "d": "definition, truncated to 400 chars",
      "dom": "ai",
      "q": 0.91,
      "m": "mature",
      "sup": ["graph"],
      "isup": ["data-structure"],
      "rel": { "uses": ["ontology"] },
      "bl": ["semantic-web"]
    }
  }
}
```

Slugs are kebab-case; ref IRIs look like `urn:ngm:class:<slug>` (slug = last `:` segment).
Empty `rel` lists are omitted.

## Tools

| Tool | Args | Returns |
|---|---|---|
| `ontology_search` | `query`, `limit=8` | Relevance-ranked `{slug, title, score}` matches. |
| `ontology_class_get` | `slug` | Full class record: definition, domain, qualityScore, maturity, parents, inferred ancestors, typed relationships, backlinks. |
| `ontology_neighbours` | `slug`, `depth=1` | Parents, inferred ancestors, relation targets by type, backlinks; `depth>=2` adds a bounded second ring of neighbour summaries. |
| `ontology_ask` | `question`, `budget_tokens=1500` | The retrieval recipe: fuzzy seed → fetch top records → terse Turtle context block, hard-clamped to the token budget (range 100–8000). Use the returned `context` to ground reasoning. |

## Wiring

### Claude Code (`.mcp.json` in your project, or `~/.claude.json`)

```json
{
  "mcpServers": {
    "ontology": {
      "command": "node",
      "args": ["/absolute/path/to/ontology-mcp/index.js"],
      "env": { "ONTOLOGY_SITE": "https://narrativegoldmine.com" }
    }
  }
}
```

LOCAL mode variant:

```json
{
  "mcpServers": {
    "ontology": {
      "command": "node",
      "args": ["/absolute/path/to/ontology-mcp/index.js"],
      "env": { "ONTOLOGY_INDEX": "/absolute/path/to/scaffold-index.json" }
    }
  }
}
```

### Generic MCP hosts (Claude Desktop, Cursor, Zed, ...)

Any host that spawns stdio MCP servers uses the same shape: command `node`,
args `[".../ontology-mcp/index.js"]`, plus the env vars above. Example for
Claude Desktop's `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "ontology": {
      "command": "node",
      "args": ["/absolute/path/to/ontology-mcp/index.js"]
    }
  }
}
```

## Test

```bash
npm install
npm test        # spawns the server in LOCAL mode against a tiny fixture and
                # exercises initialize, tools/list, ontology_search,
                # ontology_class_get, ontology_neighbours, ontology_ask
```
