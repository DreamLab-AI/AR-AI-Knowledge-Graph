---
id: AB-16
title: Secrets custody, seccomp and runtime profiles
area: agentbox
governing:
  - agentbox/docs/SECURITY-profiles.md
  - agentbox/docs/INGRESS-identity.md
adrs: [ADR-2007, ADR-2026, ADR-2027, ADR-2033]
sources:
  - agentbox/config/seccomp-agentbox.json
  - agentbox/docker-compose.yml
  - agentbox/scripts/ci/check-seccomp.sh
  - agentbox/config/hooks/trust-seed.cjs
  - agentbox/config/hooks/nostr-live-mirror.cjs
  - agentbox/config/nostr-gateway/gateway.cjs
  - agentbox/config/nip98-proxy/proxy.mjs
  - agentbox/management-api/lib/agent-identity.js
  - agentbox/config/entrypoint-unified.sh
  - agentbox/skills/email-search/SKILL.md
  - agentbox/docs/adr/ADR-2027-secret-custody-rotation-break-glass.md
  - scripts/backup-secrets.sh
verified_commit: b00c28a0d
---

## AB-16.1 Container hardening posture — what actually confines the box

```mermaid
flowchart TB
    subgraph HOST["docker host"]
        CMP["docker-compose.yml:88-142"]
    end
    subgraph CTR["agentbox container"]
        SUP["supervisord PID 1 as ROOT<br/>required at boot for tmpfs subdirs, cert gen, chown to uid 1000"]
        PROG["every long-running program<br/>user=devuser uid 1000"]
    end
    CMP -->|"read_only: true (compose:101)"| CTR
    CMP -->|"cap_drop: ALL (compose:102)"| CTR
    CMP -->|"cap_add: CHOWN FOWNER DAC_OVERRIDE AUDIT_WRITE KILL NET_ADMIN SETUID SETGID (compose:104-112)"| CTR
    CMP -->|"security_opt: no-new-privileges:true (compose:141)"| CTR
    CMP -->|"security_opt: seccomp=./config/seccomp-agentbox.json (compose:142)"| CTR
    CMP -->|"tmpfs: /tmp 1G, /run 64M, /var/log 128M, ~/.cache 1G, /usr/local/bin 8M exec+suid …"| CTR
    SUP -->|"setgroups + setuid demotion"| PROG
    CMP -.-> N1["INVARIANT compose:91-99 R-005 / SEC-001 — no runtime sudo.<br/>Root-at-boot via supervisord PID 1 is the ONLY elevation.<br/>No agent-facing process runs as root after bootstrap."]
    CMP -.-> N2["SETUID and SETGID are cap_add'ed for privilege DROPPING, not gaining.<br/>supervisord needs CAP_SETGID/CAP_SETUID to demote children to devuser.<br/>no-new-privileges:true neuters setuid FILE BITS at execve, which is a different axis and does not replace these caps."]
    CTR -.-> N3["DIVERGENCE — ADR-2007 governs configuration separation only.<br/>Profile isolation replaced Linux pseudo-user isolation, so profiles are NOT an OS boundary.<br/>SECURITY-profiles states this explicitly. Every profile shares uid 1000."]
    PROG -.-> N4["CONSEQUENCE — a same-uid boundary. Every supervised program, MCP server, skill and agent runs as devuser,<br/>so each can read every other's 0600 key file, the AoE daemon token and the mirror key. see AB-16.7"]
```

## AB-16.2 The seccomp profile is a supplemental denylist, not a sandbox

```mermaid
classDiagram
    class SeccompProfile {
        <<agentbox/config/seccomp-agentbox.json>>
        +string defaultAction
        +List~string~ architectures
        +List~SyscallRule~ syscalls
    }
    class RuleAfAlg {
        <<syscalls[0]>>
        +names socket
        +action SCMP_ACT_ERRNO
        +errnoRet 1
        +arg index 0 value 38 op SCMP_CMP_EQ
    }
    class RuleHighRisk {
        <<syscalls[1] — 45 names>>
        +action SCMP_ACT_ERRNO
        +errnoRet 1
        +args empty
    }
    class DockerDefault {
        <<the layer underneath>>
        +the real allowlist
    }
    SeccompProfile --> RuleAfAlg
    SeccompProfile --> RuleHighRisk
    SeccompProfile ..> DockerDefault : layered ON TOP OF, never replaces
    note for SeccompProfile "defaultAction = SCMP_ACT_ALLOW. This is INTENTIONALLY allow-by-default.<br/>The file's own comment: it is NOT a replacement allowlist and is NOT a complete sandbox on its own.<br/>An allowlist was REJECTED deliberately — the workload surface (Chromium, CUDA, Godot) is too wide to enumerate safely without breakage.<br/>architectures: SCMP_ARCH_X86_64, X86, AARCH64, ARM."
    note for RuleAfAlg "CVE-2026-31431 'Copy Fail' — blocks AF_ALG (family 38) socket creation,<br/>the algif_aead local privesc via splice(). Arg-indexed, so other socket families are unaffected."
    note for RuleHighRisk "add_key bpf clock_adjtime clock_settime create_module delete_module finit_module get_kernel_syms get_mempolicy<br/>init_module ioperm iopl kcmp kexec_file_load kexec_load keyctl lookup_dcookie mbind mount move_pages nfsservctl<br/>perf_event_open pivot_root process_vm_readv process_vm_writev ptrace query_module quotactl reboot request_key<br/>set_mempolicy setns settimeofday stime swapon swapoff sysfs _sysctl umount umount2 unshare uselib userfaultfd ustat vm86 vm86old"
    note for DockerDefault "DIVERGENCE — reading this file as the confinement boundary is wrong.<br/>Confinement comes from the COMBINATION: cap_drop ALL, read_only root, uid 1000, no-new-privileges,<br/>plus Docker's own default profile. This denylist only narrows what remains. see AB-16.1"
```

## AB-16.3 check-seccomp.sh — the invariant gate

```mermaid
sequenceDiagram
    autonumber
    participant CI as CI / pre-rebuild
    participant SH as check-seccomp.sh<br/>agentbox/scripts/ci/check-seccomp.sh
    participant F as seccomp-agentbox.json<br/>agentbox/config/seccomp-agentbox.json
    participant NODE as embedded node heredoc

    CI->>SH: sh scripts/ci/check-seccomp.sh
    SH->>SH: set -eu then ROOT resolve then FILE = $ROOT/config/seccomp-agentbox.json
    alt file missing
        SH-->>CI: fail "missing $FILE" then exit 1
    end
    SH->>NODE: node - "$FILE" "ptrace bpf mount kexec_load unshare setns"
    NODE->>F: JSON.parse(readFileSync)
    alt parse throws
        NODE-->>CI: FAIL — the profile no longer parses
    end
    NODE->>NODE: assert doc.defaultAction === 'SCMP_ACT_ALLOW'
    Note over NODE: the gate asserts the profile STAYS allow-by-default — a well-meaning switch to a denylist default would FAIL this check,<br/>because that would silently turn a supplemental layer into a broken half-allowlist
    loop for each REQUIRED syscall
        NODE->>F: find an SCMP_ACT_ERRNO rule whose names include it
        alt not found
            NODE-->>CI: FAIL — a denial was dropped
        end
    end
    NODE-->>CI: exit 0
    Note over SH,NODE: INVARIANT — the six named syscalls (ptrace bpf mount kexec_load unshare setns) are the hardening-sprint floor.<br/>The other 39 names in the rule are NOT gate-protected and could be removed without failing CI. DIVERGENCE against the file's own intent.
```

## AB-16.4 Boot privilege drop and the trust-seed hook

```mermaid
sequenceDiagram
    autonumber
    participant D as docker run
    participant SUP as supervisord PID 1 (root)
    participant EP as entrypoint-unified.sh<br/>agentbox/config/entrypoint-unified.sh
    participant TS as trust-seed.cjs<br/>agentbox/config/hooks/trust-seed.cjs
    participant CFG as ~/.claude.json
    participant SET as workspace .claude/settings.json
    participant CC as unattended claude sessions

    D->>SUP: PID 1 as root — tmpfs subdir creation, cert generation, chown runtime dirs to uid 1000
    SUP->>EP: run bootstrap
    rect rgb(255,248,235)
    Note over EP,SET: trust pre-acceptance — entrypoint-unified.sh:1117-1132
    EP->>SET: read settings.json hooks.SessionStart
    alt a hook command already contains trust-seed.cjs
        SET-->>EP: log "[trust] trust-seed hook already registered"
    else not registered
        EP->>SET: push {type command, command "<cmd> || true", timeout 8000, continueOnError true}
        SET-->>EP: log "[trust] registered trust-seed SessionStart hook in settings.json"
    end
    EP->>TS: node /opt/agentbox/config/hooks/trust-seed.cjs
    end
    TS->>TS: parseArgs — depth default 5, --dry-run, extra paths
    TS->>CFG: read ~/.claude.json projects.<abs path>
    loop workspace root plus every git checkout or worktree under it, bounded by depth
        TS->>CFG: mark trusted, SKIP node_modules target .tmp .cache .venv venv .git dist build
    end
    Note over TS,CFG: INVARIANT trust-seed.cjs:16 — never removes or overwrites other per-project state. Idempotent, safe to run at any time.
    alt any error
        TS-->>EP: one line to stderr then exit 0 — FAIL-OPEN so hooks and boot never stall
    end
    SUP->>CC: start every long-running program with user=devuser
    CC-->>CC: no "Do you trust the files in this folder?" dialog
    Note over TS,CC: rationale trust-seed.cjs:7 — observed 2026-09-02, ten Opus worker panes sat dead for an hour behind the trust gate.<br/>DIVERGENCE — this pre-accepts a SECURITY prompt for unattended agents. It is a deliberate availability-over-confirmation trade, not a hardening measure.
```

## AB-16.5 Derived keys — HMAC-SHA256 domain separation

```mermaid
sequenceDiagram
    autonumber
    participant ENV as operator key<br/>AGENTBOX_PRIVKEY_HEX or AGENTBOX_BRIDGE_SK or OPERATOR_NOSTR_PRIVKEY
    participant GW as gateway.cjs<br/>agentbox/config/nostr-gateway/gateway.cjs:171-178
    participant MIR as deriveChildKey<br/>agentbox/config/hooks/nostr-live-mirror.cjs:204
    participant PHONE as operator phone (Amethyst)
    participant RELAY as relays

    rect rgb(235,245,255)
    Note over ENV,GW: gateway identity — gateway.cjs:175-178
    GW->>ENV: envFirst(AGENTBOX_PRIVKEY_HEX, AGENTBOX_BRIDGE_SK, OPERATOR_NOSTR_PRIVKEY)
    alt not 64-hex
        GW-->>GW: log "no operator key (AGENTBOX_PRIVKEY_HEX) — exiting" then process.exit(0)
        Note over GW: FAIL-CLOSED by exit — the gateway refuses to run keyless rather than degrading
    end
    GW->>GW: adminPub = getPublicKey(rawSk) — the ONLY pubkey allowed to command
    alt AGENTBOX_GATEWAY_IDENTITY === 'gateway'
        GW->>GW: sk = HMAC-SHA256(operator_sk, AGENTBOX_GATEWAY_KEY_TAG or 'agentbox-gateway-v1')
    else default 'operator'
        GW->>GW: sk = rawSk — the gateway signs AS the operator
        Note over GW: DIVERGENCE — the DEFAULT identity is 'operator', so out of the box the gateway holds and signs with the ROOT operator key.<br/>The derived-child mode exists but is opt-in via env.
    end
    GW->>RELAY: auth and receive as getPublicKey(sk), reply to AGENTBOX_GATEWAY_REPLY_TO or adminPub
    end
    rect rgb(240,255,240)
    Note over MIR,PHONE: mirror child key — nostr-live-mirror.cjs:193-214
    MIR->>MIR: cached in _childCache, computed once per process
    alt AGENTBOX_MIRROR_CHILD === '0'
        MIR-->>MIR: null — child mode off, LEGACY operator-self-DM path is used instead
    else operator key not 64-hex
        MIR-->>MIR: null — same legacy fallback
    else
        MIR->>MIR: child_sk = HMAC-SHA256(operator_sk, AGENTBOX_MIRROR_KEY_TAG or 'agentbox-mirror-v1')
        MIR->>PHONE: only the CHILD nsec is imported to the device — the root operator key stays off the phone
        Note over MIR,PHONE: rotatable by bumping the tag. The mirror is a self-DM on the child identity, and the child signs nothing of consequence. see AB-13
    end
    end
    Note over GW,MIR: DIVERGENCE ADR-2027 — neither derived key appears in the SECURITY-profiles provisional custody register.<br/>Both are bespoke HMAC-SHA256 constructions rather than a standard KDF (HKDF), and rotation is "bump the env tag" with no revocation of the previously derived child at any relay.
```

## AB-16.6 Break-glass — the least governed credential

```mermaid
sequenceDiagram
    autonumber
    participant ATK as any LAN client
    participant PX as nip98-proxy verifyIdentity<br/>agentbox/config/nip98-proxy/proxy.mjs:527
    participant CT as constantTimeEqual<br/>agentbox/config/nip98-proxy/proxy.mjs:441
    participant UP as upstream (AoE or mgmt-api)
    participant ADR as ADR-2027 requirement<br/>agentbox/docs/adr/ADR-2027-secret-custody-rotation-break-glass.md

    ATK->>PX: Authorization Bearer <token> on :9096 over the LAN
    alt BREAK_GLASS unset (proxy.mjs:99 NIP98_PROXY_ALLOW_BEARER)
        PX-->>ATK: branch skipped entirely — break-glass disabled
    else configured
        PX->>CT: constantTimeEqual(token, BREAK_GLASS) (proxy.mjs:535)
        alt no match
            PX-->>ATK: fall through to the NIP-98 branch. see AB-10.3
        else match
            CT-->>PX: true
            PX->>PX: ok true, pubkey NIP98_PROXY_BEARER_PUBKEY default "break-glass", mode "break-glass" (proxy.mjs:538)
            PX->>UP: forward with X-Agentbox-Pubkey = the sentinel
            Note over PX,UP: the sentinel identity is NOT a real pubkey, so every downstream attribution for this request is a placeholder
        end
    end
    rect rgb(255,240,240)
    Note over ADR,PX: ADR-2027 REQUIRES and the code does NOT do
    ADR-->>PX: MUST be short-lived — no expiry is checked in this branch
    ADR-->>PX: MUST be single-scoped — no request-scope check exists in this branch
    ADR-->>PX: MUST be audit-logged on every use — no durable per-use receipt is written
    ADR-->>PX: MUST NOT function as a standing master credential — while set it IS one
    end
    Note over ATK,PX: DIVERGENCE INGRESS-identity "Break-glass bearer over the LAN" — accepted on :9096 AND via ?access_token= / ?bearer= on WS upgrades.<br/>A single shared secret bypasses NIP-98 entirely. Documented opt-in, but a full identity bypass while enabled. see AB-10.9
    Note over ADR: ADR-2027 status is decision_status proposed / implementation_status none / activation_status inactive.<br/>Nothing in this diagram's red block is built. It is recorded here so the gap is legible, not to imply a control exists.
```

## AB-16.7 Key files at 0600 — what that boundary is and is not

```mermaid
sequenceDiagram
    autonumber
    participant AI as agent-identity loadOrMint<br/>agentbox/management-api/lib/agent-identity.js:107
    participant KF as profile key file
    participant AOE as AoE daemon token<br/>~/.config/agent-of-empires/serve.url
    participant BSK as bridge key<br/>AGENTBOX_BRIDGE_SK_FILE default /run/secrets/nostr.key
    participant PEER as any co-resident devuser process

    AI->>KF: writeFileSync(privHex, mode 0o600) then chmodSync 0o600 (agent-identity.js:141-142)
    Note over AI,KF: comment at agent-identity.js:139 — "Persist with 0600 so the key survives a restart of this profile and no other uid can read it"
    AOE-->>AOE: minted by the daemon at launch into an owner-only 0700 directory, not env-settable
    BSK-->>BSK: loaded by nostr-pod-bridge, legacy environment fallback still present
    rect rgb(255,240,240)
    Note over PEER,BSK: the residual limit, stated in the governing docs
    PEER->>KF: read — SUCCEEDS, same uid
    PEER->>AOE: read — SUCCEEDS, same uid
    PEER->>BSK: read — SUCCEEDS, same uid
    end
    Note over PEER: DIVERGENCE GOVERNANCE-capabilities item 5 — "a process running as the same devuser can still read the token file.<br/>The token raises the bar but does not isolate same-uid peers. Per-process isolation is future work."<br/>The same sentence applies verbatim to the agent key and the bridge key. see AB-16.1
    Note over AI,KF: DIVERGENCE SECURITY-profiles custody row — persistence failure is NON-FATAL (agent-identity.js:145-148),<br/>so a container can run a whole session on an in-memory key that vanishes at restart. "Key persistence failure can produce a valid but unstable identity." see AB-11.2
```

## AB-16.8 A secret's lifecycle — implemented versus proposed

```mermaid
stateDiagram-v2
    [*] --> Provisioned
    Provisioned --> InUse : loaded at boot from env, key file or daemon state file
    InUse --> InUse : used per request with no per-use receipt
    InUse --> Derived : HMAC-SHA256 with a domain tag produces a child (gateway, mirror)
    Derived --> InUse
    InUse --> BackedUp : scripts/backup-secrets.sh collects selected config names into a ZIP
    BackedUp --> InUse
    InUse --> RotatedProposed : ADR-2027 rotation cadence
    InUse --> RevokedProposed : ADR-2027 revocation path
    RotatedProposed --> [*]
    RevokedProposed --> [*]
    note right of Provisioned
        IMPLEMENTED. Storage interfaces exist and are source-verified:
        0600 profile key file, /run/secrets/nostr.key, the AoE serve.url
        state file, NIP98_PROXY_ALLOW_BEARER and NIP98_PROXY_SESSION_SECRET
        captured at process start.
    end note
    note right of BackedUp
        PARTIAL and unverified. SECURITY-profiles: the backup script invokes
        ordinary zip and unzip integrity testing with NO encryption flags and
        no explicit umask or chmod. Final permissions depend on the invoking
        environment. Integrity testing is not a recovery exercise. The script
        was not run and no backup contents were inspected.
    end note
    note right of RotatedProposed
        NOT IMPLEMENTED. ADR-2027 is proposed / none / inactive.
        No cadence, no custodian, no incident window, no revocation
        procedure and no dated failure-recovery receipt exists for
        any of the seven register rows.
    end note
    note right of RevokedProposed
        NOT IMPLEMENTED, and structurally expensive. The relay allowlist
        is baked at nix build (relayAllowedPubkeysCsv), so publisher
        revocation needs a full rebuild — the compromise window is one
        build-deploy cycle. ADR-2027 Context. see AB-13.3
    end note
```

## AB-16.9 Provisional custody register — seven roles, zero confirmed custodians

```mermaid
flowchart LR
    R["SECURITY-profiles.md<br/>Provisional custody register 2026-09-04"]
    R --> C1["Bridge identity / unwrap key<br/>src: AGENTBOX_BRIDGE_SK_FILE default /run/secrets/nostr.key<br/>legacy env fallback remains"]
    R --> C2["Shared server publisher identity<br/>src: relay key list, build-projected<br/>ADR-2012 per-consumer split PENDING"]
    R --> C3["Proxy break-glass bearer<br/>src: NIP98_PROXY_ALLOW_BEARER captured at process start<br/>see AB-16.6"]
    R --> C4["Proxy browser-session signing secret<br/>src: NIP98_PROXY_SESSION_SECRET or per-boot random<br/>see AB-10.8"]
    R --> C5["AoE daemon token<br/>src: daemon state file read by the proxy with a last-good cache<br/>see AB-10.11"]
    R --> C6["Dream remote-execution identity<br/>src: ssh/scp dispatch on AMBIENT ssh config<br/>no explicit identity file in the inspected calls"]
    R --> C7["Secret backup artefact and recovery access<br/>src: scripts/backup-secrets.sh to workspace/secret-backups ZIP + manifest"]
    C1 --> U["EVERY ROW: custodian UNCONFIRMED, deployed location UNCONFIRMED,<br/>rotation cadence UNCONFIRMED, incident response window UNCONFIRMED"]
    C2 --> U
    C3 --> U
    C4 --> U
    C5 --> U
    C6 --> U
    C7 --> U
    U --> N1["DIVERGENCE — 'Suggested responsible role' in the register is a role TO ASSIGN,<br/>not an assertion that anyone has accepted custody. 'No cadence is invented here.'"]
    U --> N2["DIVERGENCE — these seven rows are a STARTING SET, not completeness certification.<br/>Provider credentials and other estate identities are not yet inventoried.<br/>The two derived HMAC keys in AB-16.5 are absent from the register entirely."]
    R -.-> N3["Refer to secret identifiers and custodian ROLES, never secret values.<br/>Do not copy secrets into this register. SECURITY-profiles preamble."]
```

## AB-16.10 email-search — the owner-authorised break-glass tier

```mermaid
sequenceDiagram
    autonumber
    participant AG as agent turn
    participant SK as email-search skill<br/>agentbox/skills/email-search/SKILL.md
    participant GW as email-mcp-gateway:8765<br/>visionclaw_network
    participant IDX as privacy-filtered index
    participant MAIL as raw Proton mail (2 accounts via one Bridge)

    rect rgb(235,245,255)
    Note over AG,GW: TIER 1 — transport bearer, gates ANY call
    AG->>GW: Authorization Bearer <AGENTBOX_EMAIL_GATEWAY_TOKEN>
    alt token absent or wrong
        GW-->>AG: rejected — no tool reachable
    end
    end
    AG->>SK: ask_email(query)
    SK->>GW: ask_email
    GW->>IDX: semantic search
    IDX-->>GW: sanitised evidence[] with ref_id, no real headers
    GW-->>AG: schema-abstracted answer — ALWAYS sanitised regardless of any pubkey passed
    rect rgb(255,240,240)
    Note over AG,MAIL: TIER 2 — capability pubkey, gates the RAW tools on top of tier 1
    AG->>AG: read the operator pubkey from AGENTBOX_X_ONLY_PUBKEY_HEX at call time, never hardcoded in skill source
    AG->>GW: fetch_email_by_ref(ref_id, nostr_pubkey) or fetch_email_raw(query, nostr_pubkey)
    GW->>GW: is nostr_pubkey on PRIVILEGED_NOSTR_PUBKEYS
    Note over GW: accepts bare 64-hex, 0x-prefixed or nostr:-prefixed, case-insensitive. npub1 bech32 is NOT accepted.
    alt not on the allow-list
        GW-->>AG: {"authorized": false, "error": "Nostr pubkey not authorized for raw (unfiltered) access. Use ask_email …"}
        Note over GW: every attempt is logged with an 8-char pubkey fingerprint
    else authorised
        GW->>MAIL: bypass the privacy filter
        MAIL-->>GW: real headers, sender, date, folder, full text
        GW-->>AG: {"authorized":true,"mode":"raw", …} — verbatim mail
    end
    end
    Note over AG,GW: INVARIANT — a Nostr PUBLIC key is the publishable half, an identity/capability token, not a secret.<br/>Passing it as a tool argument is the intended design. The bearer token and the Nostr PRIVATE key are what never leave the box.
    Note over AG,MAIL: DIVERGENCE — this is a real, working break-glass tier with an allow-list, a fingerprinted audit line and a default-sanitised posture,<br/>which is MORE governed than the proxy break-glass bearer in AB-16.6. Neither appears in the ADR-2027 register as an implemented control.
```

## AB-16.11 Egress profiles — mirror versus digest have different contracts

```mermaid
flowchart TB
    S["session content"]
    S --> M["nostr-live-mirror.cjs per-turn hook<br/>agentbox/config/hooks/nostr-live-mirror.cjs"]
    S --> D["kind-30840 SessionEnd digest<br/>[sovereign_mesh.mobile_bridge]"]
    M --> M1["composes UNREDACTED selected text"]
    M1 --> M2["NIP-59 gift-wrap to the derived child identity<br/>see AB-16.5 and AB-13.9"]
    M2 --> M3["cloud worker relay<br/>wss://dreamlab-nostr-relay.solitary-paper-764d.workers.dev"]
    D --> D1["flattens input and sends it to the configured summarisation provider"]
    D1 --> D2["publishes a SEPARATELY SIGNED digest"]
    M3 --> OFF["off switch AGENTBOX_LIVE_MIRROR=0 — fail-open on any publish error"]
    D2 --> OFF2["gated by its own manifest block, different switch"]
    OFF --> X["DIVERGENCE ADR-2026 — proposed / partial / INACTIVE for the complete egress policy.<br/>SECURITY-profiles: 'Their configuration gates and encryption differ.<br/>A shared off/redaction/recipient/retention contract remains open.'"]
    OFF2 --> X
    X --> Y["Consequence: there is no single answer to 'is session content leaving this box, and redacted how'.<br/>Two paths, two gates, two encryption postures, one of them unredacted before wrapping."]
    M3 -.-> Z["TRUST BOUNDARY — this is the only agentbox egress to a NON-LAN destination in this domain.<br/>The email gateway, the Loom and the relay are all LAN or loopback. see AB-13.9"]
```
