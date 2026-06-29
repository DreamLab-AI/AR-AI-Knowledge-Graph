# Android Nostr Client Landscape — June 2026

**Context**: Selecting an Android Nostr client to serve as a rich, interactive mobile surface into a
private Cloudflare Worker relay and onward into AI agent intelligence. Primary use cases are ad-hoc
encrypted DM-style chat with AI agents, receiving session summaries, and optionally group/session
channels. Key custody must be secure (ideally delegated/remote, not root admin key on phone).

---

## 1. Evaluation Rubric

| Criterion | Weight | Description |
|---|---|---|
| **Private relay** | Critical | Can add a custom `wss://` relay URL; ideally can restrict DM relay inbox (kind 10050) to that relay only |
| **NIP-17 DM** | Critical | Uses kind 14/15 sealed rumor inside NIP-59 gift wrap; hides sender identity and timestamp from relay |
| **NIP-04 DM** | Baseline | Legacy encrypted DM; publishes sender pubkey in the clear to relay — acceptable fallback, not preferred |
| **NIP-44 encryption** | Critical | ChaCha20-based versioned encryption used inside NIP-17 rumours; required for NIP-17 |
| **NIP-59 gift wrap** | Critical | Outer envelope randomising sender; required for full NIP-17 metadata privacy |
| **NIP-46 remote signer** | High | Supports `bunker://` login so the phone app never holds the root key; signs via relay-mediated RPC |
| **NIP-55 Android signer** | High | Supports Amber (or Primal) as a local Android signing provider via content-provider Intent — keeps key off the chat app |
| **NIP-29 groups** | Medium | Relay-managed closed groups; useful for modelling "sessions as group chats" |
| **NIP-28 chat** | Low-Med | Public channel-style chat; simpler than NIP-29 but no membership enforcement |
| **NIP-90 DVM** | Medium | Data Vending Machine support; lets agents advertise and receive job requests; makes client a richer agent surface |
| **NIP-89 app handler** | Low-Med | Discover and launch event-kind-specific handlers; extensibility signal |
| **Custom/unknown kinds** | Low-Med | Can display or at minimum not crash on non-standard event kinds from agents |
| **Open source** | High | Self-sovereign ethos; auditability of key handling |
| **Maintenance (2026)** | Critical | Active commits, recent releases in 2025-2026 |
| **UX for DM-centric use** | High | Is DM a first-class UI surface, or a buried tab behind a social feed? |

---

## 2. Comparison Table

Legend: Y = confirmed supported, N = not supported, P = partial/in-progress, ? = not confirmed, — = not applicable

| Client | Platform | Private relay | NIP-17 | NIP-04 | NIP-44 | NIP-59 | NIP-46 | NIP-55 | NIP-29 | NIP-28 | NIP-90 DVM | Open source | Active 2026 | DM UX | Source |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Amethyst** | Android | Y (granular relay roles: DM inbox, outbox, local, private) | Y (default since v0.87) | Y (legacy, still offered) | Y | Y | Y (bunker login) | Y (Amber integration confirmed) | Y | Y | Y (NIP-89+90, DVM tab in UI) | Y (Apache-2) | Y (v1.11.0, May 2026) | Good — DM tab + NIP-29 groups | [github.com/vitorpamplona/amethyst](https://github.com/vitorpamplona/amethyst) |
| **0xchat** | Android + iOS + Desktop (Flutter/Dart) | Y (per-session "secret chat" relay; per-user relay lists via NIP-65) | Y (default "Gift-Wrapped DM", NIP-59/44/17 stack) | Y (offered alongside NIP-17) | Y | Y | P (NIP-46 via bunker URI; no native UI confirmed) | P (fixed NIP-55 spec compliance in v1.5.3; Amber listed as supported signer) | Y (NIP-29 groups supported) | Y (NIP-28 channels restored in v1.4) | N (no NIP-90 found) | Y (MIT) | Y (v1.5.5, Mar 2025; Android Play store active) | Excellent — chat-first UI, DM is primary surface | [github.com/0xchat-app/0xchat-core](https://github.com/0xchat-app/0xchat-core) |
| **Primal** | Android + iOS + Web | P (uses Primal caching relay by default; custom relay add possible but relay control less granular than Amethyst) | P (NIP-17 inbox relay config in codebase; not confirmed as UI default on Android) | Y | Y | Y | Y (NIP-46 in v2.6.18) | Y (NIP-55 in v2.6.18; also acts as NIP-55 provider for other apps) | N (not found in releases) | N | N | Y (GitHub public) | Y (active releases 2026) | Moderate — social feed first; DM secondary | [github.com/PrimalHQ/primal-android-app](https://github.com/PrimalHQ/primal-android-app) |
| **Amber** | Android only (SIGNER, not chat) | Y (configurable signer relay) | — | — | Y (encrypts/decrypts NIP-44) | — | Y (acts as NIP-46 bunker device) | Y (IS the NIP-55 reference implementation; created the spec) | — | — | — | Y (Kotlin, 98.7%) | Y (v6.2.0, June 1 2026) | — (not a chat client) | [github.com/greenart7c3/Amber](https://github.com/greenart7c3/Amber) |
| **Coracle** | PWA (installable on Android Chrome) | Y (AUTH NIP-42 for closed relays; relay-centric architecture) | Y (confirmed in README) | Y | Y | Y | Y (NIP-46 bunker) | Y (NIP-55 listed) | Y (NIP-29) | Y (NIP-28) | Y (NIP-90) | Y (MIT) | Y (v0.6.31, Feb 2026) | Moderate — relay-browser UX; DM works | [github.com/coracle-social/coracle](https://github.com/coracle-social/coracle) |
| **Notedeck** (Damus team) | Android + Desktop (Rust) | ? (not confirmed in current releases) | P (NIP-17/giftwrap in v0.7.x; "planned for iOS and Android") | Y | Y | Y | ? | ? | N | N | N | Y (GPLv3) | Y (active 2025-2026) | Poor (multi-column browser; DM early stage) | [github.com/damus-io/notedeck](https://github.com/damus-io/notedeck) |
| **Snort** | Web/PWA | Y (fully manual relay management) | N (not in README NIP list as of Apr 2026) | Y | Y | Y | Y (NIP-46 bunker) | Y (NIP-55) | N | Y (NIP-28) | N | Y (MIT) | Y (v0.5.3, Apr 2026) | Poor for Android (web-only; no DM as primary surface) | [github.com/v0l/snort](https://github.com/v0l/snort) |
| **Nostros** | Android (React Native) | Y (relay management present) | N (not listed) | Y | N | N | N | N | N | N | N | Y (MIT) | N (archived Oct 2025) | Poor (abandoned) | [github.com/KoalaSat/nostros](https://github.com/KoalaSat/nostros) |
| **Plebstr** | Android + iOS | ? | ? | Y | ? | ? | N | N | N | N | N | Partial (limited OSS visibility) | ? (early beta, minimal public info) | Unknown | [plebstr.com](https://plebstr.com/) |
| **Freerse** | Android | Y (relay management) | ? | Y | ? | ? | ? | ? | N | N | N | Partial | ? (sparse recent activity) | Poor (social feed focused) | [freerse.com](https://freerse.com/) |
| **Yana** | Android + Desktop | Y | Y (supported per community reports) | Y | Y | Y | ? | ? | ? | Y | ? | Y | ? (unclear 2026 activity) | Moderate | [nostrapps.com/yana](https://nostrapps.com/) |
| **Damus** | iOS ONLY | — | — | — | — | — | — | — | — | — | — | Y | Y | — | [github.com/damus-io/damus](https://github.com/damus-io/damus) |
| **Nostur** | iOS + macOS ONLY | — | — | — | — | — | — | — | — | — | — | Y | Y (last iOS update Feb 2026) | — | [github.com/nostur-com/nostur-ios-public](https://github.com/nostur-com/nostur-ios-public) |

**Notes on excluded clients:**
- Damus: confirmed iOS-only. The Damus team's Android effort is Notedeck (separate app, early stage).
- Nostur: confirmed iOS/macOS only. No Android port exists or is planned as of June 2026.
- Spring/Nozzle: Spring was iOS-only; Nozzle (Android) appears to have merged into or been superseded by other efforts — no active 2026 repository found under that name.

---

## 3. Per-Client Deep Dives

### 3.1 Amethyst (Primary Android Recommendation)

**Repo**: https://github.com/vitorpamplona/amethyst
**Latest**: v1.11.0 (May 20 2026) — 14,500+ commits, actively funded by OpenSats

Amethyst is the most feature-complete Nostr client on any platform as of June 2026, and it is Android-only (by design). The developer, Vitor Pamplona, was a co-author of NIP-17 and Amethyst shipped one of the first NIP-17 implementations. The relay architecture is exceptionally granular: users can independently configure Home/Outbox relays, Public Inbox relays, DM Inbox relays (kind 10050), Private Home relays (for drafts/lists), Local relays (Citrine for on-device), and Search relays. A private Cloudflare Worker relay can be added to any of these buckets, and specifically to the DM Inbox relay list so that all NIP-17 gift-wrapped DMs are delivered exclusively to that relay. Amethyst v1.03.0 also automates relay list assembly from follows, but manual overrides are fully supported.

For the agent-bridge use case, Amethyst stands out further with full NIP-90 DVM support — there is a dedicated fourth tab for discovering and invoking Data Vending Machines — and NIP-89 app-handler dispatch. This makes the app a genuine agent interaction surface, not just a messenger. Key custody is handled cleanly: Amber (NIP-55) is natively supported, so the Amethyst install never needs to hold the nsec. NIP-46 bunker login is also supported for server-based key management. The one ergonomic limitation for a DM-centric use case is that the UI defaults to a social feed; DMs and group chats require navigating to a separate tab, though they are fully functional and polished.

### 3.2 0xchat (Strongest DM-First Alternative)

**Repo**: https://github.com/0xchat-app / https://0xchat.com
**Latest**: v1.5.5 (March 2025) — Flutter/Dart, Android + iOS + Desktop

0xchat was built from the ground up as a messaging application rather than a social network, and it shows in the UX. Gift-Wrapped NIP-17 DM is the default and recommended message type; the app explicitly states that sender identity, timestamps, and event kinds are all hidden. The "Secret Chat" feature takes this further by allowing each conversation thread to be pinned to a specific relay — a natural fit for routing all agent DMs through a single private Cloudflare Worker relay. NIP-29 group support allows modelling sessions as relay-scoped groups, and NIP-28 public channels are available for broader broadcast-style messages.

The main gap is NIP-46. 0xchat does not appear to have a native bunker URI login flow (this is confirmed absent from the core README and deepwiki analysis). Amber NIP-55 is listed as a supported signer and spec compliance was fixed in v1.5.3, so key custody via Amber is possible, but the absence of first-class NIP-46 means users cannot connect to a server-hosted nsecbunker directly from the login screen. No NIP-90 DVM support exists, limiting the "rich interactive surface" dimension. The last core release is March 2025; while the app is available on Google Play and actively used, the pace of core library updates is slower than Amethyst.

### 3.3 Amber — The NIP-55 Signer (Key Custody Component)

**Repo**: https://github.com/greenart7c3/Amber
**Latest**: v6.2.0 (June 1 2026) — Kotlin, Android only, reproducible builds

Amber is not a chat client. It is the reference Android implementation of NIP-55, the spec that greenart7c3 wrote specifically to solve the key-custody problem on Android. When installed, Amber acts as a system-level signing provider: any NIP-55-compatible client (Amethyst, 0xchat, Voyage, Fountain, Pokey, Keychat, Coracle web, etc.) issues signing intents to Amber via Android's Content Resolver, and Amber approves or denies them — optionally with a "remember my choice" setting for background auto-approval. The private nsec never leaves Amber.

Amber also operates as a NIP-46 bunker, meaning the phone itself becomes a signing device for remote sessions (web browsers, other phones, server agents) over relay-mediated RPC — no hardware wallet or additional server required. Key features relevant to an admin agent bridge: multiple account support, biometric/PIN authentication per signing request, granular per-app permission grants (which event kinds can be signed automatically), Tor support via Orbot, and GPG-signed reproducible builds. Distribution is via F-Droid, Zapstore, GitHub releases, and Obtainium (not Google Play, which creates onboarding friction for non-technical users).

### 3.4 Coracle (PWA Alternative)

**Repo**: https://github.com/coracle-social/coracle
**Latest**: v0.6.31 (February 27 2026)

Coracle is a React-based web client that can be installed as a PWA on Android Chrome and has documented build steps for Android. It has one of the broadest NIP coverage lists of any client: NIP-17, NIP-29, NIP-28, NIP-46, NIP-55, NIP-90, and NIP-42 (relay AUTH for closed relays) are all confirmed. The relay-centric architecture makes it well-suited to private relay scenarios — the entire design philosophy is around unlocking multiple relays. As a PWA the UX is less native than Amethyst or 0xchat (no push notifications by default, no Android keystore integration), but the browser security sandbox actually isolates keys reasonably well if combined with Amber via NIP-55. Coracle is particularly notable as the only non-native option with a credible NIP-90 story. Its sister project Flotilla (NIP-29 Discord-like groups PWA) was archived in February 2026, so group-chat work has consolidated back into the main Coracle codebase.

### 3.5 Primal (Runner-up, Full-Stack Nostr)

**Repo**: https://github.com/PrimalHQ/primal-android-app

Primal is the most polished onboarding experience and the best choice for users who want a single app that "just works." Version 2.6.18 added both NIP-46 remote signing and NIP-55 local signing, and Primal itself can act as a NIP-55 provider for other Android apps — so Amethyst can sign via Primal, or vice versa. The relay management story is less granular than Amethyst (Primal's caching relay is deeply integrated into the architecture and was originally the only option), and NIP-29 group support has not appeared in any release notes reviewed. NIP-90 DVM support was not found. For the private-relay agent-DM use case, Primal is viable but has the weakest relay isolation story of the three top candidates.

---

## 4. The Amber / NIP-55 Signer Pattern

### Why it matters for an admin-permissioned agent bridge

The canonical security problem with Nostr on mobile is: the chat app holds the raw nsec, and if the app is compromised, the key is lost. For an agent bridge — where the user's keypair grants admin-level access to agent sessions — this risk is amplified. The Amber pattern solves it architecturally.

**How it works:**

```
User action (e.g. "send DM to agent")
    |
    v
Chat app (Amethyst / 0xchat)
    |  "I need a signature for event kind 14"
    |  — Android Intent (Content Resolver, NIP-55) —>
    v
Amber (holds nsec in Android Keystore, protected by biometric)
    |  user approves (or auto-approves for trusted app/kind)
    |  <— returns signed event —
    v
Chat app broadcasts signed event to relay
```

The nsec is never in the chat app's process space. If Amethyst is compromised by a malicious update, the attacker gets nothing useful — they can request signatures but Amber rate-limits and prompts for approval above a configurable threshold.

**NIP-46 extension — phone as bunker:**

Amber also exposes itself as a NIP-46 bunker over a relay. This means:
- The user's desktop browser can sign into Nostr web apps using the phone as the signer
- Agent backends that use NIP-46 can request signatures from the phone over the relay, even when the phone app is in the background
- The root admin key can live exclusively in Amber and never be imported into any server

**Delegation pattern for agent bridge:**

For the specific case of an AI agent bridge, the recommended pattern is:

1. Root admin keypair lives in Amber (phone) — never leaves device
2. Agents receive a NIP-26 delegated keypair (or separate long-lived keypair) scoped to specific event kinds and time windows
3. The chat app (Amethyst or 0xchat) uses a separate account — the "conversation identity" — also backed by Amber
4. The agent's replies arrive as NIP-17 gift-wrapped kind 14 DMs from the agent's pubkey
5. Amber auto-approves outgoing DM signatures for the conversation identity (per-app/per-kind permission)

This decouples "phone holds the chat identity" from "phone holds the admin key" entirely.

---

## 5. PWA Alternative: Custom Thin Client

### The case for a custom PWA

A custom PWA (e.g. SvelteKit + nostr-tools + NDK) over the private Cloudflare Worker relay offers:

- **Full control over custom event kinds**: Agents can publish proprietary session-summary kinds (e.g. kind 30078 for parameterised replaceable application data), interactive widget kinds, or structured agent-response kinds. An off-the-shelf client will silently drop unknown kinds; a custom PWA renders whatever you define.
- **Optimised UX for the use case**: No social feed, no zap UI, no irrelevant settings — just the conversation surfaces that matter.
- **Simpler relay config**: The app can be hardcoded to connect only to `wss://your.cf-worker.relay/` — no risk of accidentally routing DMs to public relays.
- **NIP-46 integration**: nostr-tools v2 and Nostrify both have `BunkerSigner` — the PWA can connect to Amber over relay for signing, keeping key custody clean.
- **Installable on Android Chrome**: PWAs install to the Android home screen, get their own window chrome, and (with VAPID) can receive push notifications.

### The case against a custom PWA

- **Build cost**: Implementing NIP-17, NIP-44, NIP-59, relay pool management, and a decent mobile-first DM UX from scratch takes meaningful engineering time. nostr-tools and NDK lower this, but the gap to Amethyst's maturity is large.
- **No native Android Keystore integration**: A PWA key store is browser localStorage or IndexedDB — less secure than Amber's use of the Android Keystore with biometric gating. However, NIP-46 via Amber mitigates this: the PWA never needs to hold the key if it delegates signing to Amber.
- **Push notifications are harder**: Web Push requires a service worker and VAPID keys. Native Android apps (Amethyst, 0xchat with UnifiedPush) have better notification reliability.
- **Maintenance burden**: You own it; Amethyst and 0xchat evolve for free.

### Hybrid approach

A pragmatic hybrid: use 0xchat or Amethyst as the primary chat surface (NIP-17 DMs with agent contacts), and build a lightweight PWA companion for rich session-summary views and custom-kind event rendering. The PWA does not need to be the primary DM client — it supplements where off-the-shelf rendering falls short.

---

## 6. Recommendation

### Primary Recommendation: Amethyst + Amber

**Stack**: Amethyst (chat client) + Amber (NIP-55/NIP-46 signer) on the same Android device

**Reasoning:**

1. **Private relay**: Amethyst has the most granular relay configuration on any Android client. The DM Inbox relay list (kind 10050) can be set to the private Cloudflare Worker relay exclusively. All NIP-17 gift-wrapped DMs from agents will be directed to that relay by any sending client that respects kind 10050.

2. **NIP-17 encrypted DM**: Amethyst was a co-author reference implementation of NIP-17 and has shipped it as the default DM path since mid-2024. The full NIP-44 + NIP-59 stack (kind 14 rumour, kind 13 seal, kind 1059 gift wrap) is implemented and battle-tested.

3. **Key custody (Amber)**: Amethyst has first-class NIP-55 Amber integration. The admin keypair stays in Amber's Android Keystore, protected by biometrics. Amethyst sends signing intents; Amber auto-approves DM-kind events for the trusted app. The nsec never touches the chat app process.

4. **NIP-46**: Amethyst supports bunker login, so if the user prefers a server-based nsecbunker over Amber, that path is also open.

5. **Feature-rich agent surface**: No other Android client matches Amethyst's NIP-90 DVM tab + NIP-89 app-handler dispatch. Agents can be modelled as DVM service providers; Amethyst's UI already presents a discovery surface for them. NIP-29 relay-based groups add a session/channel primitive.

6. **Maintenance confidence**: v1.11.0 shipped May 2026; 14,500 commits; OpenSats-funded; Vitor Pamplona is a full-time Nostr core contributor. No other Android client comes close on velocity.

### Fallback Recommendation: 0xchat + Amber

**Reasoning**: If DM-first UX is the priority and the NIP-90 / DVM agent-surface is not yet needed, 0xchat offers the cleanest messaging experience with NIP-17 gift wrap as the default. The "Secret Chat" per-relay feature maps neatly to the private Cloudflare Worker relay. Amber NIP-55 is confirmed as a supported signer. The gaps are: slower 2025-2026 update cadence, no native NIP-46 bunker login UI, and no NIP-90. If 0xchat receives a NIP-46 login flow and continues active maintenance through 2026, it would become a stronger contender for DM-heavy use cases.

### Answer to the Critical Questions

**Which Android client has the best private-relay + NIP-17 + NIP-46/NIP-55 story in 2026?**
Amethyst. It is the only Android client with all three features fully confirmed, mature, and actively maintained as of June 2026.

**Is the right answer an off-the-shelf client, a client+signer combo, or a custom PWA?**
Client + signer combo: Amethyst + Amber. The combination gives production-grade NIP-17 DMs, granular private relay control, and secure key custody without any build investment. A custom PWA becomes worthwhile only if the use case requires rendering custom event kinds as interactive UI (e.g. structured agent response cards) — in that case, build a lightweight companion PWA for the display layer only, keeping Amethyst as the DM transport.

**Which client best supports the agent being a "contact" you DM and get rich replies/summaries from?**
Amethyst, because it is the only Android client with NIP-90 DVM support in the UI. An agent can register as a DVM service provider; Amethyst users see it in the discovery tab and can interact with it through structured job-request/result flows (kind 5000-7000) in addition to plain NIP-17 DMs. For summary messages specifically, NIP-30078 (parameterised replaceable app-specific data) is a natural kind for storing session summaries; Amethyst's NIP-89 handler dispatch means future versions can open custom kinds in registered handlers.

---

## Sources

- [Amethyst GitHub](https://github.com/vitorpamplona/amethyst)
- [Amethyst Releases](https://github.com/vitorpamplona/amethyst/releases)
- [Amethyst v0.87.2: DVMs and Gossip Model — nobsbitcoin](https://www.nobsbitcoin.com/amethyst-v0-87-2/)
- [Amethyst Relay Setup 101 — Vitor Pamplona](https://vitor.npub.pro/post/relay-setup/)
- [Amethyst — OpenSats project page](https://opensats.org/projects/amethyst)
- [0xchat GitHub organisation](https://github.com/0xchat-app)
- [0xchat-core README](https://github.com/0xchat-app/0xchat-core)
- [0xchat releases](https://github.com/0xchat-app/0xchat-app-main/releases)
- [0xchat User Relay Configuration — DeepWiki](https://deepwiki.com/0xchat-app/0xchat-core/3.4-user-relay-configuration)
- [0xchat Relay Management — DeepWiki](https://deepwiki.com/0xchat-app/0xchat-core/2.3-relay-management-and-configuration)
- [0xchat on Google Play](https://play.google.com/store/apps/details?id=com.oxchat.nostr)
- [Amber GitHub](https://github.com/greenart7c3/Amber)
- [Amber README](https://github.com/greenart7c3/Amber/blob/master/README.md)
- [Amber — F-Droid](https://f-droid.org/en/packages/com.greenart7c3.nostrsigner/)
- [Amber Review — nostr-reviews.com](https://www.nostr-reviews.com/post/1740708496659/)
- [Amber — nostrapps.com](https://nostrapps.com/amber)
- [Primal Android GitHub](https://github.com/PrimalHQ/primal-android-app)
- [Primal Android v2.6.18 release notes](https://newreleases.io/project/github/PrimalHQ/primal-android-app/release/2.6.18)
- [Coracle GitHub](https://github.com/coracle-social/coracle)
- [Notedeck GitHub](https://github.com/damus-io/notedeck)
- [Nostros GitHub (archived)](https://github.com/KoalaSat/nostros)
- [Snort GitHub](https://github.com/v0l/snort)
- [Plebstr website](https://plebstr.com/)
- [Damus GitHub (iOS only)](https://github.com/damus-io/damus)
- [Nostur GitHub (iOS/macOS only)](https://github.com/nostur-com/nostur-ios-public)
- [NIP-17 specification](https://nips.nostr.com/17)
- [NIP-44 specification](https://nips.nostr.com/44)
- [NIP-46 specification](https://nips.nostr.com/46)
- [NIP-55 specification](https://nips.nostr.com/55)
- [NIP-29 specification](https://nips.nostr.com/29)
- [NIP-89 specification](https://nips.nostr.com/89)
- [NIP-90 specification](https://nips.nostr.com/90)
- [NIP-17: Private Direct Messages — Nostr Compass](https://nostrcompass.org/en/topics/nip-17/)
- [Nostr Compass Newsletter #15 (March 2026)](https://nostrcompass.org/en/newsletters/2026-03-25-newsletter/)
- [Best Nostr Apps 2026 — humai.blog](https://www.humai.blog/best-nostr-apps-2026-damus-primal-amethyst-tested/)
- [Advancements in Nostr Clients — OpenSats](https://opensats.org/blog/advancements-in-nostr-clients)
- [Nostr MCP Server for AI Agents — Glama](https://glama.ai/mcp/servers/jorgenclaw/nostr-mcp-server)
- [Nosflare — Cloudflare Worker Nostr relay](https://github.com/Spl0itable/nosflare)
- [nostr-tools npm](https://www.npmjs.com/package/nostr-tools)
- [Nostrify TypeScript framework](https://soapbox.pub/tools/nostrify/)
- [awesome-nip46 client list](https://github.com/blackcoffeexbt/awesome-nip46-remote-nostr-signing-clients)
- [Nostr apps directory](https://nostrapps.com/)
- [nostr.co.uk clients directory](https://nostr.co.uk/clients/)
