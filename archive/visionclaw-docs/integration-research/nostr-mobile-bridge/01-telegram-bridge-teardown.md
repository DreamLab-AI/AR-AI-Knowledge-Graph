# CTM (Claude Telegram Mirror) — Full Teardown

Revision: 2026-06-02
Source repo: `/home/devuser/workspace/claude-telegram-mirror/`
All citations are `file:line`.

---

## 1. Architecture Overview

### Daemon Model

CTM is a single Rust binary (`ctm`) that is simultaneously a library crate and a CLI. It runs as a long-lived foreground daemon, optionally supervised by systemd or launchd (`service.rs`). The binary uses `clap` for subcommand dispatch (`main.rs:54-146`).

The daemon is started via `ctm start`, which calls `daemon::Daemon::new(cfg)` and then `daemon.start().await` (`main.rs:267-269`). On SIGINT/SIGTERM the daemon calls `daemon.stop().await` to clean up (`main.rs:295`).

Process model:

```
ctm daemon process
  └─ tokio async runtime (multi-thread)
       ├─ SocketServer (Unix domain socket, AF_UNIX, NDJSON framing)
       │    └─ broadcast channel → event_loop
       ├─ event_loop (tokio::select! on 4 arms)
       │    ├─ socket_rx (BridgeMessage broadcast)
       │    ├─ Telegram long-poll (getUpdates)
       │    ├─ topic_invalidated_rx
       │    └─ cleanup_interval
       └─ SQLite (sessions.db, costs.db, identity.db via rusqlite)
```

### How It Attaches to Claude Code

CTM attaches via Claude Code's hook system — not via any API or process injection. The hooks are installed by `ctm install-hooks`, which writes JSON entries into `~/.claude/settings.json` (or project `.claude/settings.json`). Each hook runs `ctm hook` as a subprocess, passing the event JSON on stdin (`installer.rs`).

Hook events handled: `PreToolUse`, `PostToolUse`, `Stop`, `SubagentStop`, `Notification`, `UserPromptSubmit`, `PreCompact`, `SessionEnd` (`types.rs:6-15`).

Each `ctm hook` invocation is short-lived. It reads one JSON event from stdin, builds `BridgeMessage` structs, and sends them to the long-running daemon over a Unix domain socket at `~/.config/claude-telegram-mirror/bridge.sock` (`hook.rs:72-75`).

---

## 2. Outbound Path (Agent → Phone)

### Hook → Daemon Pipeline

1. Claude Code fires a hook, spawning `ctm hook` as a subprocess.
2. `hook::process_hook()` reads stdin (bounded to 1 MiB via `MAX_LINE_BYTES`, `types.rs:489`).
3. The JSON is parsed into `HookEvent` (tagged enum, `types.rs:6-15`).
4. `build_messages()` converts the event into one or more `BridgeMessage` structs (`hook.rs:256-493`).
5. Messages are sent over the Unix socket as NDJSON, one per line (`hook.rs:823-843`).
6. The daemon's `SocketServer` broadcasts each parsed message on a tokio broadcast channel (`socket.rs:344-368`).
7. The event loop receives on `socket_rx` and dispatches to `handle_socket_message()` (`event_loop.rs:63-81`).

### Hook Event → BridgeMessage Mapping

| Hook Event | MessageType(s) produced | Notes |
|---|---|---|
| Any event | `session_start` | Sent on every hook invocation (idempotent) — `hook.rs:279-284` |
| `PreToolUse` | `tool_start` | Includes tool name, input, toolUseId — `hook.rs:287-303` |
| `PostToolUse` | `tool_result` | Full output (not truncated) sent to daemon — `hook.rs:305-335` |
| `Notification` | `agent_response` or `error` | `error` level → error type; `idle_prompt` dropped — `hook.rs:336-347` |
| `UserPromptSubmit` | `user_input` | Source marked `"cli"` — `hook.rs:348-357` |
| `Stop` | `agent_response` + optional `session_rename` + `turn_complete` | Reads transcript_summary / last_assistant_message inline; falls back to JSONL file read — `hook.rs:358-420` |
| `SubagentStop` | `agent_response` | Includes agent_id, agent_type, result summary — `hook.rs:421-467` |
| `PreCompact` | `pre_compact` | `hook.rs:469-471` |
| `SessionEnd` | `session_end` | Fires once on process exit; cleans up state file — `hook.rs:472-489` |

### Telegram Message Formatting

The daemon sends messages to Telegram using Markdown (v1). Key formatters in `daemon/socket_handlers.rs`:

- `handle_agent_response`: sends assistant text via `format_agent_response()`, chunked at `config.chunk_size` (default 4000 chars).
- `handle_tool_start`: sends a one-liner like "Running tests" (from `summarize::summarize_tool_action()`) with an optional "Details" inline button referencing `toolUseId`.
- `handle_tool_result`: sends "Completed (N lines of output)" or error summary.
- `handle_approval_request`: sends the full tool description with Approve/Reject/Abort inline keyboard.

### Session → Forum Topic Mapping

Each Claude Code `session_id` maps to exactly one Telegram Forum Topic (thread). The mapping is stored in:
- SQLite `sessions.thread_id` column (`session.rs:25`).
- In-memory `DashMap<String, i64>` keyed by session_id (`daemon/mod.rs`).

Topic creation: when the daemon receives `session_start` for a new session and `config.use_threads == true`, it calls `bot.create_forum_topic()` with a name formatted as `session_id | hostname | project_dir`, colour-hashed from the session ID (`socket_handlers.rs:241-265`).

Sub-agent sessions share the parent's topic: the daemon extracts `parent_session_id` from the transcript path pattern `…/{parentSessionId}/subagents/agent-{id}.jsonl` and reuses the parent's `thread_id` (`socket_handlers.rs:85-200`).

---

## 3. Inbound Path (Phone → Agent) — The Critical Bit

### Overview

Inbound messages from Telegram enter via the long-poll loop in `event_loop.rs:84-118` (calls `bot.get_updates(offset)`). Each update is dispatched to `telegram_handlers::handle_telegram_update()`.

**The mechanism is entirely tmux-based.** There is no API endpoint in Claude Code for injecting text. The daemon maintains the tmux pane address for each session (captured from `$TMUX` env in the hook subprocess at hook fire time — `injector.rs:247-303`) and uses `tmux send-keys` to write text into that pane.

### Step-by-Step Inbound Flow

1. User sends a text message to the Telegram forum topic associated with a session.
2. `handle_telegram_update()` verifies `msg.chat.id == config.chat_id` (silent drop if wrong — `telegram_handlers.rs:11-16`).
3. `handle_telegram_text()` is called. It reads `msg.message_thread_id` and looks up the session via `get_session_by_thread_id()` in SQLite (`telegram_handlers.rs:80-86`).
4. The session's `tmux_target` and `tmux_socket` are retrieved from the in-memory cache or DB.
5. `InputInjector::inject(text)` is called. This runs two synchronous subprocess commands (`injector.rs:95-133`):
   ```
   tmux send-keys -t <target> -l <text>
   tmux send-keys -t <target> Enter
   ```
   The `-l` flag sends literal text (no key-name interpretation), preventing injection of shell metacharacters. `Command::arg()` is used throughout — no shell interpolation (`injector.rs:9`).
6. If injection succeeds, the text appears in the Claude Code CLI at the terminal cursor position, as if the user had typed it.

### Injection Prerequisites

- Claude Code must be running inside a `tmux` session when the hook fires.
- The hook subprocess captures `$TMUX` env and `tmux display-message -p #S/#I/#P` to derive `target = session:window.pane` (`injector.rs:247-295`).
- This target is stored in the `BridgeMessage.metadata.tmuxTarget` field and persisted in SQLite on each `session_start` message.
- If no tmux target is found, injection fails and the daemon sends a warning to Telegram: "Reply failed — tmux not detected. Start Claude Code inside tmux for bidirectional chat." (`telegram_handlers.rs:101-118`).

### Approval Workflow (Synchronous, Blocking)

The PreToolUse hook implements a **synchronous blocking approval gate**:

1. Hook detects a tool requiring approval (Write, Edit, MultiEdit, non-safe Bash — `hook.rs:657-669`).
2. Hook sends an `approval_request` BridgeMessage to the daemon and then **blocks on `send_and_wait()` with a 300-second timeout** (`hook.rs:557`).
3. The daemon sends an approval message to Telegram with Approve/Reject/Abort buttons (`socket_handlers.rs:727-769`).
4. User taps a button → callback query arrives → `callback_handlers::handle_approval_callback()` is called.
5. The daemon sends an `approval_response` BridgeMessage back to the specific socket client that sent the `approval_request` (identified by `_client_id` metadata injected by the socket server — `socket.rs:349-355`).
6. The hook's `send_and_wait()` unblocks, reads the response, and writes the permission decision JSON to stdout:
   - `approve` → `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow",...}}`
   - `reject` → `deny` + reason
   - `abort` → `deny` with "user chose to stop the session"
   - timeout → `ask` (fall back to CLI)
   (`hook.rs:558-596`)

This is **fully synchronous and blocks Claude Code's execution** during the approval window.

### Other Inbound Capabilities

| Input | Mechanism |
|---|---|
| Free text (not a command) | `inject(text)` + Enter via tmux (`telegram_handlers.rs:203-206`) |
| `cc <cmd>` prefix | Converted to `/<cmd>` slash command, injected via `send_slash_command()` (`telegram_handlers.rs:122-128`) |
| `stop` / `interrupt` | `send_key("Escape")` via tmux (`telegram_handlers.rs:131-151`) |
| `kill` | `send_key("Ctrl-C")` via tmux (`telegram_handlers.rs:153-173`) |
| Photo sent from Telegram | File downloaded to `/tmp/ctm-images/<uuid>.jpg`, path injected as text `[Image from Telegram: /tmp/...] Caption: ...` (`telegram_handlers.rs:311-316`) |
| Document sent from Telegram | Same as photo — downloaded and path injected (`telegram_handlers.rs:333-395`) |
| AskUserQuestion inline buttons | Option selected via callback; answer injected as text or key sequence for multi-select TUI (`callback_handlers.rs`) |
| `/mute` bot command | Toggles message suppression for a thread — not injected into Claude |
| `/sessions` bot command | Lists active sessions — not injected into Claude |
| `/abort` bot command | Sends Ctrl-C to the tmux pane; marks session as `aborted` in DB |

---

## 4. Session Lifecycle

### Creation

- Created on first `session_start` message received from any hook event (`socket_handlers.rs:27-54`).
- `SessionManager::create_session()` is idempotent: if the session already exists, it updates `last_activity` and auto-heals tmux/hostname metadata (`session.rs:297-333`).
- A forum topic is created via Telegram API if `use_threads == true` and no existing `thread_id` is found (`socket_handlers.rs:240-264`).
- The `thread_id` is stored in SQLite and in-memory cache immediately after creation.

### Active State

- `last_activity` updated on every hook event.
- `turn_complete` message: signals end of one assistant turn. If a compact was pending, triggers `handle_compact_complete()`.
- `session_rename` message: renames the Telegram forum topic to `custom_title | session_id | hostname` (`socket_handlers.rs:907-966`).

### Stale Detection

The cleanup loop runs every `CLEANUP_INTERVAL_SECS` seconds. It calls `session.get_stale_session_candidates(timeout_hours)` (default 72 hours — `config.rs:354`). Stale sessions have their topic deleted (after a configurable delay) and are marked `ended` in SQLite (`daemon/cleanup.rs`).

### Ending

- `SessionEnd` hook fires exactly once at process exit, `/clear`, or logout (`types.rs:136-144`).
- `handle_session_end()` marks session `ended`, expires pending approvals atomically, sends session duration summary to Telegram, and schedules topic deletion (`socket_handlers.rs:336-406`).
- If `auto_delete_topics == true` (default), the topic is deleted after `topic_delete_delay_minutes` (default 15 min).
- Cascades to child sub-agent sessions: all active children are ended when the parent ends (`socket_handlers.rs:384-406`).

### Database Tables (sessions.db)

```
sessions: id, chat_id, thread_id, hostname, tmux_target, tmux_socket,
          started_at, last_activity, status, project_dir, metadata,
          parent_session_id, agent_id, agent_type
pending_approvals: id, session_id, prompt, created_at, expires_at, status, message_id
```
(`session.rs:139-176`)

---

## 5. Summarisation

### Two-Tier Tool Summary System

CTM does **not** generate end-of-session summaries using an LLM. The summarisation system is scoped to **tool action one-liners** for display in Telegram.

**Tier 1 — Rule-based (zero latency):** `summarize::summarize_tool_action()` (`summarize.rs:345-411`) maps tool name + input to a human-readable string:

| Tool | Pattern | Output |
|---|---|---|
| `Bash` | `cargo test` | "Running tests" |
| `Bash` | `git push` | "Pushing to remote" |
| `Bash` | `docker compose up` | "Starting containers" |
| `Read` | any `file_path` | "Reading `filename.rs`" |
| `Write` | any `file_path` | "Writing `filename.rs`" |
| `Edit`/`MultiEdit` | any `file_path` | "Editing `filename.rs`" |
| `Grep` | `pattern` | "Searching for 'pattern'" |
| `WebSearch` | `query` | "Searching: query" |
| `Task` | any | "Running task" |
| unknown | — | "Using ToolName" |

`summarize_tool_result()` (`summarize.rs:414-453`) detects error patterns (Rust `error[E...`], `Error:` prefix, `FAILED`, `panic!`, `npm ERR!`) and returns a summary line.

**Tier 2 — LLM fallback (optional):** `LlmSummarizer::summarize()` (`summarizer.rs:48-84`) is called when the rule-based result starts with "Using " or "Running `" (generic patterns). It calls a configured LLM endpoint with the prompt: "Summarize this Claude Code tool action in under 10 words. Tool: {tool}. Input: {input}" (`summarizer.rs:89`). The result is cached in-memory (LRU-capped at 200 entries, `summarizer.rs:26`).

Supported backends: Ollama (`/api/chat` or `/api/generate`), Anthropic (`/v1/messages` using `claude-haiku-4-5-20251001`, `summarizer.rs:144`), or generic `{"prompt":...}→{"response":...}` format.

**When the LLM summarizer fires:** only for `tool_start` and `tool_result` messages in `verbose` mode (`socket_handlers.rs:554-558`, `socket_handlers.rs:618-626`).

### What Summaries Are NOT in CTM

- There is no session-level summary generated at session end.
- There is no "what did Claude do this session?" digest sent to Telegram.
- The `session_end` message sends only session duration and a fixed-format status (`socket_handlers.rs:336-360`).
- The `Stop` hook extracts `transcript_summary` or `last_assistant_message` from the Claude Code event fields (provided by Claude Code itself), or falls back to reading new JSONL lines from the transcript file (`hook.rs:360-393`). This is forwarded verbatim as an `agent_response` — it is Claude's own summary, not CTM-generated.

**Implication for Nostr bridge:** CTM does not have a "manage sessions via summaries" feature today. That capability must be built from scratch. The raw material (transcript JSONL, per-turn `transcript_summary` from Claude Code's `Stop` event, tool action records) is all present and could feed a summary pipeline.

---

## 6. Identity and Auth Model

### Current Auth Mechanism

The only authorisation gate is **Telegram chat ID matching**. Every incoming update is checked against `config.chat_id` (`telegram_handlers.rs:11-15`). Messages from any other chat are silently dropped. There is no per-user allowlist enforced in message handling today — any participant in the authorised Telegram group can send messages.

### Identity Layer (Partial Implementation)

`identity.rs` contains a complete `IdentityStore` backed by SQLite (`identity.db`) that maps `did:nostr:<hex-pubkey>` identities to Telegram user IDs with roles (`admin` / `user`). Key methods:

- `is_allowed(telegram_id)` — checks if a Telegram user ID has any registered identity (`identity.rs:227`).
- `is_admin(telegram_id)` — checks admin role (`identity.rs:239`).
- `bootstrap_operator(pubkey_hex, telegram_id)` — seeds the operator from `agentbox.toml [sovereign_mesh.operator]` as admin on startup (`identity.rs:110-141`).
- `seed_from_config(users)` — seeds `allowed_users` from `agentbox.toml` (`identity.rs:143-176`).

The identity store schema maps each `pubkey_hex` (64 hex chars = 32-byte secp256k1 key) to a Telegram user ID, role, label, and audit metadata. The DID format is `did:nostr:{pubkey_hex}` (`identity.rs:54-61`).

**However:** as of this codebase, `is_allowed()` is not called anywhere in `telegram_handlers.rs` or `callback_handlers.rs`. The identity store exists and is bootstrapped but is not yet used to gate inbound Telegram messages at the per-user level. The single `chat_id` check is the only enforced guard.

### agentbox.toml Auth Configuration

`agentbox_config.rs:75-76` defines:
```toml
[[sovereign_mesh.telegram.allowed_users]]
pubkey_hex = "..."
telegram_id = 12345
role = "admin"
label = "John"
```
These are seeded into `IdentityStore` at startup but not yet enforced in message routing.

---

## 7. Config Surface

### Priority Order

`env vars > agentbox.toml [sovereign_mesh.telegram] > legacy config.json > defaults`
(`agentbox_config.rs:3-4`)

### Complete Config Knobs

| Config key | Env var | Default | Notes |
|---|---|---|---|
| `bot_token` | `TELEGRAM_BOT_TOKEN` | — | Required. Never read from TOML. |
| `chat_id` | `TELEGRAM_CHAT_ID` | 0 | Required. Supergroup IDs start with -100. |
| `enabled` | `TELEGRAM_MIRROR` | false | Master on/off. |
| `verbose` | `TELEGRAM_MIRROR_VERBOSE` | true | Enables tool_start/tool_result messages. |
| `approvals` | `TELEGRAM_MIRROR_APPROVALS` | true | Enables PreToolUse approval gate. |
| `use_threads` | `TELEGRAM_USE_THREADS` | true | Creates forum topics per session. |
| `chunk_size` | `TELEGRAM_CHUNK_SIZE` | 4000 | Max chars per Telegram message. |
| `rate_limit` | `TELEGRAM_RATE_LIMIT` | 20 | Messages/sec (token bucket). |
| `session_timeout` | `TELEGRAM_SESSION_TIMEOUT` | 30 | Minutes for approval timeout. |
| `stale_session_timeout_hours` | `TELEGRAM_STALE_SESSION_TIMEOUT_HOURS` | 72 | Hours before session declared stale. |
| `auto_delete_topics` | `TELEGRAM_AUTO_DELETE_TOPICS` | true | Delete forum topic on session end. |
| `topic_delete_delay_minutes` | `TELEGRAM_TOPIC_DELETE_DELAY_MINUTES` | 15 | Delay before topic deletion. |
| `inactivity_delete_threshold_minutes` | `TELEGRAM_INACTIVITY_DELETE_THRESHOLD_MINUTES` | 720 | 12h inactivity triggers deletion. |
| `socket_path` | `TELEGRAM_BRIDGE_SOCKET` | `~/.config/claude-telegram-mirror/bridge.sock` | Unix domain socket. |
| `llm_summarize_url` | `CTM_LLM_SUMMARIZE_URL` | — | Optional LLM endpoint for enhanced summaries. |
| `llm_api_key` | `CTM_LLM_API_KEY` | — | API key for LLM endpoint. |

TOML additional fields (agentbox.toml only):

| Field | Default | Notes |
|---|---|---|
| `default_model` | `"sonnet"` | Not yet used in routing. |
| `max_workers` | 5 | Not yet enforced. |
| `notification_mode` | `"live"` | Not yet used in routing. |
| `sovereign_mesh.operator.pubkey_hex` | — | Auto-bootstrapped to admin in IdentityStore. |

(`config.rs:69-99`, `agentbox_config.rs:42-130`)

### Config File Locations

- Primary: `~/.config/claude-telegram-mirror/config.json` (camelCase or snake_case keys)
- Secondary: `agentbox.toml` searched at:
  1. `/home/devuser/workspace/project/agentbox/agentbox.toml`
  2. `/opt/agentbox/agentbox.toml`
  3. `./agentbox.toml`
  (overrideable via `AGENTBOX_TOML_PATH` env — `agentbox_config.rs:13-16`)

---

## 8. Cost Tracking

`cost.rs` implements a `CostStore` backed by `costs.db` (SQLite). Schema:

```
cost_events: id, chat_id, thread_id, session_id, cost_usd, input_tokens,
             output_tokens, cache_read_tokens, cache_creation_tokens,
             model, from_user_id, created_at
```
(`cost.rs:70-94`)

Query capabilities:
- `get_group_total(chat_id)` — aggregate USD + tokens for entire Telegram group.
- `get_topic_total(chat_id, thread_id)` — per-session costs.
- `get_user_total(from_user_id)` — per-Telegram-user costs.
- `get_daily_costs(chat_id, days)` — daily breakdown.
- `check_budget(chat_id, max_budget_usd)` → `Ok | Warning(pct) | Exceeded` (`cost.rs:213-229`).

**However:** as of this codebase, `cost_events` are not populated anywhere in the daemon handlers. No hook event carries cost/token data to CTM. The `CostStore` exists as a complete infrastructure layer but cost events are never recorded via the normal hook flow. `CostEvent.record()` is defined but not called in `socket_handlers.rs` or `telegram_handlers.rs`. This is a planned feature, not yet wired up.

---

## 9. Capability Inventory

| # | Capability | Status | Nostr Equivalent Difficulty |
|---|---|---|---|
| 1 | Mirror tool start events to phone (human-readable one-liners) | Active | Easy — publish kind:1 note |
| 2 | Mirror tool result / errors to phone | Active | Easy |
| 3 | Mirror assistant responses (turn output) to phone | Active | Easy |
| 4 | Mirror user prompts to phone | Active | Easy |
| 5 | One session = one forum topic (thread isolation) | Active | Medium — NIP-29 group or NIP-10 thread |
| 6 | Session start/end notifications with duration | Active | Easy |
| 7 | Topic rename when Claude names the session | Active | Medium — edit Nostr event |
| 8 | Auto-delete old topics after inactivity | Active | Medium — publish close event |
| 9 | Send free-text from phone → Claude (tmux injection) | Active | Hard — requires tmux on server side |
| 10 | `stop` / `kill` commands (Escape / Ctrl-C via tmux) | Active | Hard — same tmux dependency |
| 11 | Slash command forwarding (`cc clear`, etc.) | Active | Hard — same tmux dependency |
| 12 | Tool approval gate (Approve/Reject/Abort inline keyboard) | Active | Hard — synchronous blocking RPC over Nostr |
| 13 | Approval timeout → fallback to CLI | Active | Medium — timeout logic, same |
| 14 | Tool details expand button | Active | Medium — send follow-up note on request |
| 15 | Sub-agent spawn / complete notifications | Active | Easy — publish notes |
| 16 | Sub-agent details button | Active | Medium |
| 17 | Photo/document send from phone → Claude (file injection) | Active | Hard — file transfer out-of-band |
| 18 | Image/file send from Claude → phone | Active (send_image msg type) | Medium — upload to media server, link in note |
| 19 | AskUserQuestion interactive multi-select | Active | Hard — stateful interactive UI |
| 20 | Mute a session thread | Active | Easy — client-side filter |
| 21 | `/sessions` — list active sessions | Active | Easy — query local DB |
| 22 | `/abort` — kill Claude session | Active | Hard — tmux Ctrl-C |
| 23 | Mirroring on/off toggle | Active | Easy — config flag |
| 24 | LLM-enhanced tool summaries (Ollama/Anthropic) | Active (optional) | Easy — same logic |
| 25 | Cost tracking per session/user | Infrastructure only (not wired) | Easy — record from hook data |
| 26 | DID:nostr identity RBAC | Infrastructure only (not enforced) | Easy — enforce at message handler level |
| 27 | `ctm doctor` diagnostics | Active | Not needed (different deployment) |
| 28 | systemd/launchd service integration | Active | Not needed |

---

## Critical Questions — Explicit Answers

### What is the complete set of capabilities a Nostr bridge must match?

To fully replace CTM for "ad-hoc chats + manage sessions via summaries":

**Must-have for parity:**
1. Outbound mirroring of all 8 hook event types (capabilities 1-8, 15).
2. Session-scoped message threading (capability 5).
3. Free-text injection into Claude (capability 9) — this is the hardest.
4. Tool approval gate with synchronous response (capability 12).
5. Stop/kill/slash-command injection (capabilities 10-11).

**For "manage sessions via summaries" specifically (not in CTM today):**
CTM does not generate session-level summaries. It forwards Claude Code's own `transcript_summary` field from `Stop` events. A Nostr bridge targeting "manage via summaries" needs to either: (a) use Claude Code's built-in `transcript_summary` field, or (b) implement its own LLM summarisation pass over the JSONL transcript on `SessionEnd`. Neither is present in CTM.

### Exactly How Does Inbound Message Injection Work?

The mechanism is `tmux send-keys`. There is no other path.

Steps (concrete):
1. When Claude Code runs inside tmux, the hook captures `$TMUX` env var (socket path + session ID from format `socket,pid,window`) and runs `tmux display-message -p "#S"`, `#I`, `#P"` to get `session:window.pane` (`injector.rs:247-295`).
2. This target string is stored in `BridgeMessage.metadata.tmuxTarget` and persisted in SQLite.
3. On inbound Telegram message, the daemon runs:
   ```
   tmux send-keys -t <session>:<window>.<pane> -l <text>
   tmux send-keys -t <session>:<window>.<pane> Enter
   ```
   using `std::process::Command::arg()` — no shell (`injector.rs:95-133`).
4. Claude Code receives this as keyboard input to its PTY.

**Critical constraint for Nostr:** The daemon must still be on the same machine as the tmux session (or have SSH access to it). This is a local process, not a remote API call. A Nostr bridge cannot eliminate this local component — it can only move the notification/control UI layer to Nostr while the injection side remains local.

The synchronous approval gate adds another constraint: the `ctm hook` subprocess blocks for up to 300 seconds waiting for an `approval_response` on the socket (`hook.rs:557`). For Nostr this means the local daemon must still hold the socket connection open and relay the Nostr response back to the blocking hook process within that window.

### How Are Summaries Triggered and What Do They Contain?

CTM's summarisation is tool-action summaries only, triggered per-tool at display time:

- **Trigger:** `handle_tool_start()` or `handle_tool_result()` when `config.verbose == true`.
- **Content:** A one-liner like "Running tests" or "Editing config.rs" from `summarize_tool_action()`.
- **LLM trigger:** Only when the rule-based result is generic ("Using X" / "Running `X`"), and `CTM_LLM_SUMMARIZE_URL` is set.
- **Model:** `claude-haiku-4-5-20251001` for Anthropic backend; `qwen3-coder:latest` for Ollama (`summarizer.rs:118`, `summarizer.rs:144`).
- **Session-level summaries:** Not implemented. CTM passes through `transcript_summary` / `last_assistant_message` from Claude Code's own `Stop` event hook fields (`hook.rs:362-392`), but does not generate them.

### What Is the Authorisation/Permission Model?

Current enforced model: single `chat_id` allowlist. Only messages from the configured Telegram group chat are processed. Per-user identity checking is not enforced.

Planned/infrastructure model: `IdentityStore` maps `did:nostr:<pubkey_hex>` ↔ Telegram user ID with `admin`/`user` roles. The operator's Nostr pubkey is auto-seeded as admin from `agentbox.toml`. The per-user `is_allowed()` check exists in code but is **not called** during message handling.

For the Nostr bridge, the identity model in `identity.rs` is directly reusable: it already uses Nostr public keys as the primary identifier (`pubkey_hex` = 32-byte secp256k1 key in hex, identical to Nostr NIP-01 format). Enforcing `is_allowed(telegram_id)` → `is_allowed_by_pubkey(nostr_pubkey)` is a small wiring change.

---

## 1. Architecture Overview

### Daemon model

CTM is a single Rust binary (`ctm`) that operates in two execution modes:

1. **Hook mode** (`ctm hook`): A short-lived process invoked by Claude Code's hook system. Reads a JSON hook event from stdin, constructs one or more `BridgeMessage` structs, and sends them over a Unix domain socket to the daemon. Returns immediately. For `PreToolUse` events that require approval, it blocks on `send_and_wait()` up to 300 seconds waiting for an `ApprovalResponse` to arrive back on the same socket connection before returning the `permissionDecision` JSON to Claude Code via stdout (`hook.rs:557`).

2. **Daemon mode** (`ctm start`): A long-running async Tokio process. Runs a Unix socket server (`socket.rs`), a Telegram long-polling bot (`bot/`), a SQLite-backed session manager (`session.rs`), and an event loop (`daemon/event_loop.rs`) that fans out all incoming socket messages to per-type handlers.

### How CTM attaches to Claude Code

CTM installs entries into Claude Code's `~/.claude/settings.json` (or project `.claude/settings.json`) via `ctm install-hooks` (`installer.rs`). The hooks registered are:

- `PreToolUse` — fires before any tool execution; CTM uses this for tool-approval gating
- `PostToolUse` — fires after a tool completes
- `Stop` — fires after every assistant turn
- `Notification` — fires on agent notifications
- `UserPromptSubmit` — fires when the user submits a prompt in the CLI
- `PreCompact` — fires when context compaction begins
- `SessionEnd` — fires exactly once when a session truly terminates (`types.rs:136–144`)
- `SubagentStop` — fires when a sub-agent completes

All hooks invoke `ctm hook`, which reads the event JSON from stdin (`hook.rs:18–19`).

### Process model

```
Claude Code CLI
    |
    | (hook invocation: ctm hook)
    v
ctm hook process (short-lived)
    |
    | Unix domain socket NDJSON write
    | (default: ~/.config/claude-telegram-mirror/bridge.sock)
    v
ctm daemon (long-running)
    |-- SocketServer (tokio, broadcast channel, 64 max clients)
    |-- TelegramBot (long-polling, reqwest)
    |-- SessionManager (SQLite: sessions.db, pending_approvals)
    |-- IdentityStore (SQLite: identity.db)
    |-- CostStore (SQLite: costs.db)
    |-- InputInjector (tmux send-keys)
    |-- LlmSummarizer (optional, HTTP to Ollama/Anthropic)
    |-- DaemonState (Arc<> shared across all handlers)
```

The socket file is chmod 0600, parent directory 0700 (`socket.rs:136–163`). An exclusive `flock(2)` on `bridge.pid` prevents duplicate daemon instances (`socket.rs:372–389`).

---

## 2. Outbound Path — Agent to Phone

### Hook event to BridgeMessage

For each Claude Code hook invocation, `process_hook()` (`hook.rs:13`) runs:

1. Reads up to 1 MiB from stdin (`hook.rs:18–19`).
2. Parses into a `HookEvent` enum (`types.rs:6–15`) tagged on `hook_event_name`.
3. Validates the session ID (alphanumeric + `.-_`, max 128 chars, `types.rs:510–516`).
4. Detects the current tmux session via `InputInjector::detect_tmux_session()` and captures hostname (`hook.rs:62–63`).
5. Calls `build_messages()` which produces a `Vec<BridgeMessage>`.
6. Sends all messages to the daemon socket as NDJSON (`hook.rs:72–76`).

### Message construction per hook event type

| Hook event | BridgeMessage types produced | Notes |
|---|---|---|
| Any event | `SessionStart` (always prepended, idempotent) | `hook.rs:279–284` |
| `PreToolUse` | `ToolStart` + possibly blocks for `ApprovalRequest/Response` | `hook.rs:287–303` |
| `PostToolUse` | `ToolResult` (if output len >= 10) | `hook.rs:305–334` |
| `Notification` | `AgentResponse` or `Error`; `idle_prompt` type is suppressed | `hook.rs:336–346` |
| `UserPromptSubmit` | `UserInput` | `hook.rs:348–357` |
| `Stop` | `AgentResponse` (from transcript_summary, last_assistant_message, or JSONL), optionally `SessionRename`, then `TurnComplete` | `hook.rs:358–420` |
| `SubagentStop` | `AgentResponse` with agentId metadata | `hook.rs:421–468` |
| `PreCompact` | `PreCompact` | `hook.rs:469–471` |
| `SessionEnd` | `SessionEnd` | `hook.rs:472–489` |

### Daemon-side rendering

The daemon's `handle_socket_message()` (`daemon/mod.rs:579`) dispatches to type-specific handlers in `daemon/socket_handlers.rs`. The key rendering operations:

- **session_start**: Creates a Telegram forum topic (or reuses the parent's for sub-agents). Topic name format: `{hostname} • {project_basename} • {short_session_id}`. Sends a formatted session-start message to the topic (`socket_handlers.rs:277–293`).
- **agent_response**: Sends the assistant's text, formatted via `format_agent_response()`. Sub-agent completions get a one-liner + "Details" button (`socket_handlers.rs:410–491`).
- **tool_start**: Sends a one-liner summary (rule-based via `summarize.rs`, optionally LLM-enhanced via `summarizer.rs`) with a "Details" inline button (`socket_handlers.rs:551–593`).
- **tool_result**: Sends action + result summary with a "Details" button (`socket_handlers.rs:596–686`).
- **approval_request**: Sends the formatted prompt with Approve/Reject/Abort inline buttons (`socket_handlers.rs:727–768`).
- **session_end**: Sends a duration-formatted session-end message; schedules topic deletion after `topic_delete_delay_minutes` (default 15 min) (`socket_handlers.rs:336–406`).
- **error**: Sends the error text (`socket_handlers.rs:771–787`).
- **pre_compact**: Sends a "Compacting..." notification (`socket_handlers.rs:823–850`).
- **session_rename**: Renames the Telegram forum topic (`socket_handlers.rs:905–965`).

### Session to forum-topic mapping

Each Claude Code session ID maps 1:1 to a Telegram forum topic (thread). The mapping is persisted in SQLite (`sessions.db`, column `thread_id`). An in-memory `HashMap<String, i64>` cache fronts the DB (`daemon/mod.rs:136`). Sub-agents share their parent session's forum topic (`socket_handlers.rs:216–232`).

Topic names are constructed from `{hostname} • {project_basename} • {short_session_id[0..8]}` (`daemon/mod.rs:551–573`). Topic color is deterministic from session ID hash mod 6 (`socket_handlers.rs:243`).

---

## 3. Inbound Path — Phone to Agent

### Complete injection mechanism

This is the hardest part to replicate. The full chain is:

```
User types in Telegram topic
    |
    | Telegram long-poll update
    v
handle_telegram_update() [daemon/telegram_handlers.rs:6]
    |
    | check: msg.chat.id == config.chat_id (security check, line 12–15)
    v
handle_telegram_text() [telegram_handlers.rs:60]
    |
    | 1. Look up session by message_thread_id in SQLite
    | 2. Look up tmux target for session (3-tier: cache -> DB -> live detect)
    | 3. Classify message type
    v
InputInjector::inject(text) OR send_key(key) OR send_slash_command(cmd)
    |
    | tmux send-keys -t {session:window.pane} -l {text}
    | tmux send-keys -t {session:window.pane} Enter
    v
Claude Code CLI stdin (via tmux pane)
```

### Key details of tmux injection

`InputInjector` (`injector.rs`) executes `tmux send-keys` via `std::process::Command::arg()` with no shell interpolation. The `-l` flag sends text literally, bypassing tmux key binding interpretation. Enter is sent as a separate command (`injector.rs:95–133`).

**Tmux target resolution** is a 3-tier lookup (`daemon/mod.rs:1126–1200`):
1. In-memory `HashMap<session_id, tmux_target>` (e.g., `"turbo-flow:0.0"`)
2. SQLite `sessions` table `tmux_target` column
3. Live detection: `$TMUX` env var → `tmux display-message -p "#S:#I.#P"`, fallback to scanning all panes for a process named `claude`

The tmux target is captured by the hook process at the time of each hook event from `$TMUX` and the `tmux display-message` command, then stored in the `BridgeMessage` metadata as `tmuxTarget` and `tmuxSocket` (`hook.rs:62`, `injector.rs:247–303`). On every incoming message, `check_and_update_tmux_target()` updates the cached target if it changed (`daemon/mod.rs:736–773`).

**If tmux is not detected**, the daemon sends back an error message to the Telegram topic and drops the injection. There is no queuing or retry (`telegram_handlers.rs:100–119`).

### Message type routing (inbound)

| Input text | Handling | Injector method |
|---|---|---|
| Free text | Injected as prompt | `inject(text)` + Enter |
| `cc <cmd>` prefix | Converted to `/<cmd>` slash command | `send_slash_command()` |
| `stop`, `cancel`, `escape`, etc. | Sends Escape key | `send_key("Escape")` |
| `kill`, `exit`, `ctrl-c`, `^c`, etc. | Sends Ctrl-C | `send_key("Ctrl-C")` |
| Photo | Downloaded to `~/.config/claude-telegram-mirror/downloads/`, injected as `[Image from Telegram: {path}] Caption: {caption}` | `inject()` |
| Document | Same as photo with file path | `inject()` |
| `/rename <name>` | Injects `/rename <name>` slash command into Claude Code | `send_slash_command()` |
| `/sessions`, `/status`, `/ping`, `/attach`, `/detach`, `/mute`, `/unmute`, `/abort`, `/toggle` | Handled locally by bot | No injection |
| AskUserQuestion button tap | Captures tentative answer, injects on "Submit All" | `inject()` on finalize |

### Approval gating (synchronous, blocking)

For `PreToolUse` events requiring approval (Write, Edit, MultiEdit, non-safe Bash), the hook process calls `send_and_wait()` (`hook.rs:557`) which:

1. Sends an `ApprovalRequest` BridgeMessage to the daemon socket.
2. Keeps the socket connection open and reads back NDJSON.
3. Blocks up to 300 seconds for an `ApprovalResponse` message matching the same `session_id` (`hook.rs:868–900`).
4. Maps the response content (`"approve"` / `"reject"` / `"abort"`) to Claude Code's `permissionDecision` (`"allow"` / `"deny"`) JSON output on stdout.

The daemon side routes the Telegram button tap to the specific waiting hook client by recording `approval_id → client_id` in `pending_approval_clients` (`socket_handlers.rs:739–745`). The response is sent only to that client's write half of the socket connection, not broadcast to all clients (`daemon/callback_handlers.rs`).

Safe Bash commands (whitelist: `ls`, `pwd`, `cat`, `head`, `tail`, `echo`, `grep`, `find`, `which`) auto-approve without Telegram interaction (`types.rs:354–356`).

**Critical implication for Nostr replacement**: This is a synchronous blocking call in Claude Code's hook process. The Nostr bridge must deliver the approval decision back to a waiting HTTP/socket endpoint before the 300-second timeout, or Claude Code falls back to CLI-level approval. The bridge cannot be purely fire-and-forget for approval events.

### Bidirectionality

CTM is fully bidirectional:
- Outbound: hook events → daemon → Telegram
- Inbound: Telegram → daemon → tmux inject → Claude Code stdin
- Synchronous round-trip: PreToolUse approval (hook blocks on socket, daemon relays Telegram button tap)

---

## 4. Session Lifecycle

### Creation

Sessions are created in two ways:
- **On first hook event**: Every hook invocation prepends a `SessionStart` BridgeMessage. The daemon's `ensure_session_exists()` lazily creates the session and forum topic on first occurrence (`daemon/mod.rs:785–947`).
- **Explicit session_start message**: When the daemon receives `SessionStart` and the session does not yet exist in SQLite, `handle_session_start()` runs.

The SQLite `sessions` table schema (`session.rs:139–157`):
```
id, chat_id, thread_id, hostname, tmux_target, tmux_socket,
started_at, last_activity, status, project_dir, metadata,
parent_session_id, agent_id, agent_type
```

### Forum topic creation

On new session, the daemon calls `bot.create_forum_topic()` with a name derived from hostname, project basename, and session ID prefix. The assigned `thread_id` is stored in SQLite and the in-memory cache.

Sub-agent sessions (detected via transcript path containing `/subagents/` or `agent_id` in hook metadata) reuse the parent session's `thread_id` — they appear in the same Telegram topic as the parent (`socket_handlers.rs:216–232`).

### Activity tracking

Every incoming socket message updates `last_activity` in SQLite (`daemon/mod.rs:595–604`). The cleanup loop (`daemon/cleanup.rs`) runs every 5 minutes and marks sessions as stale after `stale_session_timeout_hours` (default 72 hours). Topics of ended sessions are scheduled for deletion after `topic_delete_delay_minutes` (default 15 minutes) from session end, or `inactivity_delete_threshold_minutes` (default 720 minutes) from last activity.

### Session end

Triggered by the `SessionEnd` hook event (fires exactly once on process exit, `/clear`, or logout). The daemon:
1. Marks status as `ended` in SQLite and expires all pending approvals.
2. Sends a duration-formatted end message to the Telegram topic.
3. Schedules topic deletion (default: 15-minute delay, then `closeForumTopic`/`deleteForumTopic`).
4. Cascades end to all active child (sub-agent) sessions.
5. Removes from tmux cache and custom titles cache.

Reactivation: If hook events arrive for an ended session (race condition or daemon restart), the daemon reactivates it via `reactivate_session()` and cancels any pending deletion.

### Session database

SQLite at `~/.config/claude-telegram-mirror/sessions.db` (0600 perms). Two tables: `sessions` and `pending_approvals`. Both indexed on `chat_id`, `status`, `session_id` (`session.rs:139–176`).

---

## 5. Summarisation

### Two summarisation systems

CTM has two distinct summarisation systems, both in `summarize.rs` and `summarizer.rs`:

#### 5.1 Rule-based tool summariser (`summarize.rs`)

`summarize_tool_action(tool, input)` maps tool names and inputs to human-readable one-liners. Examples:
- `Bash` with `cargo test` → `"Running tests"` (`summarize.rs:127`)
- `Bash` with `git commit` → `"Committing changes"` (`summarize.rs:152`)
- `Read` with `/path/to/file` → `"Reading file.rs"` (`summarize.rs:356–362`)
- `Write` → `"Writing file.rs"` (`summarize.rs:363–369`)
- Generic fallback: `"Using {tool_name}"` (`summarize.rs:409`)

`summarize_tool_result(tool, output)` detects errors in output:
- Rust `error[E...]` → `"Failed: ..."` (`summarize.rs:426–430`)
- `panicked at` → `"Panicked: ..."` (`summarize.rs:441–445`)
- `FAILED` → `"Tests failed"` (`summarize.rs:436–438`)
- Default: `"Completed ({n} lines of output)"` (`summarize.rs:451`)

#### 5.2 LLM-backed tool summariser (`summarizer.rs`)

`LlmSummarizer` optionally enhances the rule-based summary when it is "generic" (starts with `"Using "` or `"Running \`"`). It calls an external LLM endpoint with the prompt:

> "Summarize this Claude Code tool action in under 10 words. Tool: {tool}. Input: {truncated_input_500_chars}"

Results are cached in-memory (LRU-style: clear all at 200 entries). Supported backends:
- **Ollama** (`/api/chat` or `/api/generate`): model `qwen3-coder:latest`, `num_predict: 30` (`summarizer.rs:115–141`)
- **Anthropic** (`/v1/messages`): model `claude-haiku-4-5-20251001`, `max_tokens: 30` (`summarizer.rs:143–168`)
- **Generic chat** (`{"prompt": ...}` → `{"response": ...}`) (`summarizer.rs:170–194`)
- 15-second HTTP timeout; falls back to rule-based on failure (`summarizer.rs:32–33`)

Enabled by setting `CTM_LLM_SUMMARIZE_URL` env var or `summarizer.url` in `agentbox.toml`.

### When summaries are generated

The rule-based summariser runs on every `tool_start` and `tool_result` event routed to Telegram. The LLM summariser augments those same events when enabled and when the rule-based result is generic. There is no "session summary" generated by CTM itself — summarisation is limited to individual tool action one-liners.

**Note**: The `transcript_summary` and `last_assistant_message` fields of the `Stop` hook event are passed through as `AgentResponse` messages. These are summaries generated by Claude Code itself (not CTM), relayed verbatim to Telegram (`hook.rs:360–393`).

---

## 6. Identity and Auth Model

### Current authorisation model

CTM performs **Telegram chat ID validation** as its primary authorisation check. All incoming Telegram updates are silently dropped if `msg.chat.id != config.chat_id` (`telegram_handlers.rs:12–15`). This is a single-chat, single-operator model by default.

### IdentityStore (`identity.rs`)

CTM includes a more sophisticated identity system backed by SQLite (`~/.config/claude-telegram-mirror/identity.db`). The schema (`identity.rs:91–107`):
```
pubkey_hex  TEXT PRIMARY KEY,  -- secp256k1 hex pubkey (64 chars)
telegram_id INTEGER NOT NULL UNIQUE,
role        TEXT NOT NULL DEFAULT 'user',  -- 'admin' | 'user'
label       TEXT NOT NULL DEFAULT '',
added_by    TEXT NOT NULL,
added_at    TEXT NOT NULL
```

Key functions:
- `is_allowed(telegram_id)`: checks if the Telegram user ID has any mapping (`identity.rs:227`)
- `is_admin(telegram_id)`: checks for admin role (`identity.rs:239`)
- `bootstrap_operator(pubkey_hex, telegram_id)`: upserts the operator as admin (`identity.rs:112–141`)
- `seed_from_config(users)`: bulk-inserts from `agentbox.toml` `allowed_users` array (`identity.rs:143–176`)

### DID:nostr linkage

The identity store links `did:nostr:<hex-pubkey>` to `telegram_id`. The `IdentityRecord.did()` method produces the DID string (`identity.rs:54–56`). This is a forward-looking design: pubkeys are stored, but the current auth enforcement relies on Telegram IDs, not Nostr signature verification.

### Operator configuration

In `agentbox.toml`:
```toml
[sovereign_mesh.operator]
pubkey_hex = "..."  # 64-char secp256k1 hex pubkey
npub = "..."        # Bech32 npub (informational only)
display_name = "..."

[[sovereign_mesh.telegram.allowed_users]]
pubkey_hex = "..."
telegram_id = 12345
role = "admin"  # or "user"
label = "Alice"
```

The `bootstrap_operator` call uses `operator.pubkey_hex` + `config.chat_id` to auto-grant admin (`agentbox_config.rs:260–263`).

**Current effective auth**: Telegram chat ID allowlist (single chat). The pubkey/DID system is present in the database layer but not enforced in the message handler path — `is_allowed()` is not called in `handle_telegram_update()`. Auth is purely at the chat level.

---

## 7. Configuration Surface

### Priority order

`env vars` > `agentbox.toml [sovereign_mesh.telegram]` > `~/.config/claude-telegram-mirror/config.json` > defaults

### Full config knobs

| Env var | agentbox.toml field | Default | Description |
|---|---|---|---|
| `TELEGRAM_BOT_TOKEN` | never in TOML | — | Bot token from @BotFather (required) |
| `TELEGRAM_CHAT_ID` | `chat_id` | 0 | Target Telegram supergroup ID |
| `TELEGRAM_MIRROR` | `telegram_mirror` (boolean in `[sovereign_mesh]`) | false | Global enable/disable |
| `TELEGRAM_MIRROR_VERBOSE` | `verbose` | true | Show tool_start + tool_result events |
| `TELEGRAM_MIRROR_APPROVALS` | `approvals` | false | Enable tool-approval gating |
| `TELEGRAM_USE_THREADS` | `use_threads` | true | Use forum topics (one per session) |
| `TELEGRAM_CHUNK_SIZE` | `chunk_size` | 4000 | Max chars per Telegram message |
| `TELEGRAM_RATE_LIMIT` | `rate_limit` | 20 | Outbound messages per minute |
| `TELEGRAM_SESSION_TIMEOUT` | `session_timeout_minutes` | 30 | Approval timeout in minutes |
| `TELEGRAM_STALE_SESSION_TIMEOUT_HOURS` | `stale_session_hours` | 72 | Hours before session marked stale |
| `TELEGRAM_AUTO_DELETE_TOPICS` | `auto_delete_topics` | true | Delete topics after session ends |
| `TELEGRAM_TOPIC_DELETE_DELAY_MINUTES` | `topic_delete_delay_minutes` | 15 | Delay before topic deletion on session end |
| `TELEGRAM_INACTIVITY_DELETE_THRESHOLD_MINUTES` | `inactivity_threshold_minutes` | 720 | Delete topic after inactivity |
| `TELEGRAM_BRIDGE_SOCKET` | `socket_path` | `~/.config/claude-telegram-mirror/bridge.sock` | Unix socket path |
| `CTM_LLM_SUMMARIZE_URL` | `summarizer.url` | — | LLM endpoint for tool summaries |
| `CTM_LLM_API_KEY` | — | — | API key for LLM endpoint |
| `AGENTBOX_TOML_PATH` | — | hardcoded search paths | Override TOML location |
| `CLAUDE_CODE_HEADLESS` | — | — | Suppresses topic creation for headless sessions |

Runtime toggle: `ctm toggle [--on|--off]` writes `~/.config/claude-telegram-mirror/status.json` and sends an `enable`/`disable` command over the socket to update the running daemon's `AtomicBool` without restart.

### Config file

`~/.config/claude-telegram-mirror/config.json` (camelCase or snake_case keys). The bot token is deliberately excluded from TOML (`agentbox_config.rs:3–4`); it must be in env or `config.json`.

---

## 8. Cost Tracking

### Schema

`~/.config/claude-telegram-mirror/costs.db` (0600 perms), table `cost_events` (`cost.rs:70–94`):
```
id                    TEXT PRIMARY KEY,
chat_id               INTEGER NOT NULL,
thread_id             INTEGER NOT NULL,
session_id            TEXT NOT NULL,
cost_usd              REAL NOT NULL DEFAULT 0.0,
input_tokens          INTEGER NOT NULL DEFAULT 0,
output_tokens         INTEGER NOT NULL DEFAULT 0,
cache_read_tokens     INTEGER NOT NULL DEFAULT 0,
cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
model                 TEXT NOT NULL DEFAULT '',
from_user_id          INTEGER NOT NULL DEFAULT 0,
created_at            TEXT NOT NULL
```

### Query surface

- `get_group_total(chat_id)` — total USD + tokens for the chat
- `get_topic_total(chat_id, thread_id)` — per-session cost
- `get_user_total(from_user_id)` — per-user cost
- `get_daily_costs(chat_id, days)` — daily breakdown
- `check_budget(chat_id, max_usd)` — returns `Ok` / `Warning(pct)` / `Exceeded`

**Not present in code**: Cost events are never actually inserted by the current hook path. The `CostStore` schema and query API exist, but no call to `cost_store.record()` appears in `hook.rs`, `socket_handlers.rs`, or `telegram_handlers.rs`. The cost tracking infrastructure is present but not yet wired to any data source that populates it.

---

## 9. Capability Inventory Table

The following table lists every user-facing capability CTM provides, for use as a Nostr bridge feature-parity checklist.

| Capability | CTM Implementation | Required for Nostr Replacement |
|---|---|---|
| Mirror all agent turns (assistant text) to phone | `Stop` hook → `AgentResponse` → Telegram | Yes |
| Mirror tool invocations (start) | `PreToolUse` hook → `ToolStart` → Telegram | Optional (verbose mode) |
| Mirror tool completions (result) | `PostToolUse` hook → `ToolResult` → Telegram | Optional (verbose mode) |
| Mirror user prompts (from CLI) | `UserPromptSubmit` hook → `UserInput` → Telegram | Optional |
| Mirror notifications and errors | `Notification` hook → Telegram | Yes |
| Mirror sub-agent spawn and completion | `SubagentStop` + transcript path detection → Telegram | Yes |
| Per-session forum topic (one thread per session) | Telegram supergroup + forum topics + SQLite mapping | Yes (Nostr equivalent: one event thread or channel per session) |
| Topic naming from hostname/project/session | `format_topic_name()` | Yes |
| Topic auto-delete after inactivity | `cleanup.rs` + scheduled tasks | Yes |
| Tool-approval gating (synchronous) | `PreToolUse` → `ApprovalRequest` + inline buttons → `ApprovalResponse` → hook stdout | Yes (hardest to replicate) |
| Safe command auto-approve whitelist | `SAFE_COMMANDS` list (`ls`, `cat`, etc.) | Yes |
| Free-text reply injection into Claude | `handle_telegram_text()` → `InputInjector::inject()` → tmux | Yes (requires tmux or equivalent IPC) |
| Special key injection (Escape, Ctrl-C) | `send_key()` via tmux | Yes |
| Claude slash command forwarding (`/clear`, `/compact`, etc.) | `cc` prefix → `send_slash_command()` | Yes |
| Photo/image forwarding to Claude | Download + path injection | Desirable |
| Document forwarding to Claude | Download + path injection | Desirable |
| AskUserQuestion interactive form | `handle_ask_user_question()` → inline buttons → tentative + Submit All | Yes |
| Multi-select option questions | `toggle:` callbacks + `MultiOption` state | Yes |
| Session list query | `/sessions` bot command | Yes |
| Session status query | `/status` bot command | Yes |
| Attach/detach to specific session | `/attach`, `/detach` bot commands | Yes |
| Mute/unmute session notifications | `/mute`, `/unmute` bot commands | Yes |
| Abort session (Escape + mark aborted) | `/abort` bot command | Yes |
| Rename session (topic title) | `/rename` + `SessionRename` hook + JSONL custom-title | Yes |
| Session resume after topic deletion | `reactivate_session()` + new topic creation | Yes |
| Sub-agent routing to parent topic | ADR-013 parent-child session detection | Yes |
| Session context compaction notification | `PreCompact` hook → Telegram + turn_complete | Optional |
| Runtime toggle (mirror on/off) | `/toggle` bot command + `status.json` + socket command | Yes |
| LLM-enhanced tool summaries | `LlmSummarizer` (Ollama/Anthropic) | Optional |
| Rule-based tool summaries | `summarize.rs` (30+ command patterns) | Yes |
| Cost tracking per session/user/topic | `CostStore` schema (not yet populated) | Optional |
| Budget enforcement | `check_budget()` (not yet wired) | Optional |
| Operator pubkey + allowed-user list | `IdentityStore` + `agentbox.toml` | Yes |
| Telegram user ID allowlist | `chat_id` check + `identity.db` | Replace with Nostr pubkey auth |
| Bot startup/shutdown notification | `send_message()` on start/stop | Yes |
| Session age display in `/sessions` | Elapsed time from `started_at` | Yes |
| Ping latency test | `/ping` + message edit | Optional |
| Bot connectivity test | `ctm config --test` | Optional |
| `ctm doctor` diagnostics | `doctor.rs` | Optional |
| Service management (systemd/launchd) | `service/` module | Environment-specific |
| Security: token scrubbing from logs | `ScrubWriter` in `main.rs` | Yes |
| Security: path traversal prevention | `validate_transcript_path()`, socket path checks | Yes |
| Security: tmux injection without shell interpolation | `Command::arg()` + `-l` flag | Yes |

---

## Critical Questions — Explicit Answers

### What is the complete set of capabilities a Nostr bridge must match?

For "ad-hoc chats + manage sessions via summaries," the minimum viable set is:

1. **Outbound**: relay `AgentResponse` (assistant turns) and `SessionEnd` events as Nostr events to the operator's Nostr relay, addressed to the operator's npub.
2. **Inbound**: accept Nostr DMs or NIP-28 channel messages from the operator's key, authenticated by Nostr signature, and inject them into the running Claude session via tmux.
3. **Approval gating**: deliver `ApprovalRequest` as a Nostr event and relay the response back to the blocking hook process within 300 seconds.
4. **Session threading**: maintain a 1:1 mapping from session ID to Nostr event thread (via `e` tags or a dedicated NIP-28 channel per session).
5. **Session summaries**: expose the `transcript_summary` / `last_assistant_message` content (from `Stop` hook) in Nostr events, enabling a summary-browsing interface.

### Exactly how does inbound message injection work?

The mechanism requires **three co-located components on the agent host**:

1. **tmux**: Claude Code must be running inside a tmux pane. There is no injection path that bypasses tmux.
2. **CTM daemon**: holds the tmux target address (session:window.pane) per Claude session in memory and SQLite.
3. **Nostr relay listener**: must be a process co-located on the same machine that can write to the Unix domain socket (or directly call `tmux send-keys`).

The Nostr bridge cannot be pure cloud: at minimum a thin agent-side process must receive the Nostr DM/event and execute `tmux send-keys`. The Telegram variant of CTM handles this because the daemon runs locally. A pure-relay Nostr approach (no local process) cannot inject text.

### How are summaries triggered and what do they contain?

CTM does not independently generate "session summaries." Two things it does relay:

1. **Claude Code's built-in summary**: The `Stop` hook event carries `transcript_summary` (a compacted summary Claude Code generates) and `last_assistant_message` (the raw final assistant turn). CTM relays whichever is present as an `AgentResponse` message to Telegram (`hook.rs:360–393`).
2. **Tool action one-liners**: CTM's `summarize.rs` generates one-liners for each tool invocation (e.g., "Building project", "Editing src/main.rs"). These are relayed as `ToolStart`/`ToolResult` messages.

There is no "end-of-session summary" that CTM itself generates. For "manage sessions via summaries," the Nostr bridge would need to either (a) relay the `transcript_summary` field from each `Stop` event, or (b) implement its own summarization step on the session's full transcript.

### What is the authorisation/permission model today?

Single-layer: Telegram chat ID must equal `config.chat_id`. All other updates are silently dropped. The `IdentityStore` with `did:nostr:<pubkey>` → `telegram_id` → `role` (admin/user) mapping exists in the database schema and is seeded from `agentbox.toml`, but the `is_allowed()` check is **not called** in the active message handler path. Effective auth today is: "anyone who can send a message in this specific Telegram supergroup."

For a Nostr replacement: the `IdentityStore` pubkey field directly maps to a Nostr npub. Auth would shift to Nostr signature verification on the event, with the `pubkey_hex` field serving as the canonical identity.
