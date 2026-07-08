---
title: Voice Routing
description: Multi-user voice routing with LiveKit SFU, push-to-talk, and spatial audio for agents
category: how-to
tags:
  - voice
  - audio
  - livekit
  - spatial
updated-date: 2026-02-11
difficulty-level: intermediate
---

# Voice Routing

> **Status (2026-07, PRD-023 WP-10 / V4).** This guide describes the AudioRouter
> four-plane model, shared by the Godot XR native client and the desktop React
> client. In the **React client**, Plane 1 ("Private Agent Commands") is the
> shipped **governed push-to-talk-to-selected-actor** loop: PTT-held STT is bound
> to the selected agent's `did:nostr` and dispatched as a signed kind-31402 to
> the agentbox `/v1/voice-intent` producer, acknowledged over Kokoro TTS (COM-15
> / V1), with a V3 confidence gate holding low-confidence input for a
> clarification turn. The legacy "Agent Command Parser" that drove **swarm
> orchestration** directly from a spoken command is **deprecated in the React
> client** — the swarm-command route returns a deprecation notice. The "Agent
> Command Parser" box in the diagrams below denotes that legacy / Godot-native
> design.

## Overview

VisionClaw's AudioRouter provides multi-user voice routing across four audio planes, enabling both private agent interaction and public spatial voice chat within the 3D collaborative space (Godot XR native APK or desktop browser client).

Each user gets an isolated voice session with per-user broadcast channels. Push-to-talk (PTT) controls audio routing between agent commands and spatial voice chat. In the React client, "agent commands" means the governed push-to-talk-to-selected-actor loop (see the status banner above), not direct swarm orchestration.

## Audio Planes

```mermaid
flowchart TB
    subgraph User["User Session"]
        Mic[Microphone]
        Ear[Speaker/Headphones]
        PTT[Push-to-Talk Button]
    end

    subgraph Plane1["Plane 1: Private Agent Commands"]
        STT[Turbo Whisper STT]
        CMD[Agent Command Parser]
    end

    subgraph Plane2["Plane 2: Private Agent Response"]
        TTS[Kokoro TTS]
        Private[Owner's Ears Only]
    end

    subgraph Plane3["Plane 3: Public Voice Chat"]
        LK1[LiveKit SFU]
        All1[All Users Spatial]
    end

    subgraph Plane4["Plane 4: Public Agent Voice"]
        AgTTS[Agent TTS at Position]
        LK2[LiveKit SFU]
        All2[All Users Spatial]
    end

    PTT -->|Held| Mic
    Mic -->|PTT held| STT
    STT --> CMD

    CMD --> TTS
    TTS --> Private
    Private --> Ear

    Mic -->|PTT released| LK1
    LK1 --> All1

    AgTTS --> LK2
    LK2 --> All2
```

| Plane | Direction | Scope | When Active |
|-------|-----------|-------|-------------|
| **1** | User mic → STT → Agent | Private (per-user) | PTT held |
| **2** | Agent → TTS → User ear | Private (per-user) | Agent responds |
| **3** | User mic → LiveKit → All users | Public (spatial) | PTT released |
| **4** | Agent TTS → LiveKit → All users | Public (spatial) | Agent configured as public |

## Push-to-Talk (PTT)

PTT determines where microphone audio is routed:

- **PTT held**: Audio routes to Plane 1 (Turbo Whisper STT for agent commands)
- **PTT released**: Audio routes to Plane 3 (LiveKit SFU for spatial voice chat)

## Per-User Voice Sessions

Each user gets an isolated `UserVoiceSession`:

```rust
pub struct UserVoiceSession {
    pub user_id: String,
    pub private_audio_tx: broadcast::Sender<Vec<u8>>,  // TTS audio for this user only
    pub transcription_tx: broadcast::Sender<String>,     // STT results for this user
    pub owned_agents: Vec<String>,                       // Agent IDs owned by user
    pub ptt_active: bool,                                // Current PTT state
    pub livekit_participant_id: Option<String>,          // LiveKit session
    pub spatial_position: [f32; 3],                      // 3D position in collaborative space
}
```

## Agent Voice Identities

Each agent has a distinct voice identity with spatial positioning:

```rust
pub struct AgentVoiceIdentity {
    pub agent_id: String,
    pub agent_type: String,
    pub owner_user_id: String,
    pub voice_id: String,        // Kokoro voice preset (e.g., "af_sarah")
    pub speed: f32,              // Speech speed multiplier
    pub position: [f32; 3],      // Agent's 3D position
    pub public_voice: bool,      // Whether all users hear this agent
}
```

### Default Voice Presets by Agent Type

| Agent Type | Voice ID | Speed | Character |
|------------|----------|-------|-----------|
| researcher | `af_sarah` | 1.0 | Clear, informative |
| coder | `am_adam` | 1.1 | Quick, technical |
| analyst | `bf_emma` | 1.0 | Measured, precise |
| optimizer | `am_michael` | 0.95 | Deliberate, methodical |
| coordinator | `af_heart` | 1.0 | Warm, collaborative |

## Setup

### Prerequisites

- LiveKit server (for spatial voice chat)
- Kokoro TTS container (for agent voice synthesis)
- Turbo Whisper STT (for speech recognition)

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `LIVEKIT_URL` | — | LiveKit server URL |
| `LIVEKIT_API_KEY` | — | LiveKit API key |
| `LIVEKIT_API_SECRET` | — | LiveKit API secret |
| `KOKORO_API_URL` | `http://kokoro-tts-container:8880` | Kokoro TTS endpoint |
| `WHISPER_API_URL` | `http://whisper-webui-backend:8000` | Whisper STT endpoint |

### Docker Compose

Add the LiveKit service to your Docker Compose:

```yaml
livekit:
  image: livekit/livekit-server:latest
  ports:
    - "7880:7880"
    - "7881:7881"
    - "7882:7882/udp"
  environment:
    - LIVEKIT_KEYS=devkey:secret
  command: --dev
```

## Integration with Spatial Presence

Voice routing integrates with the presence WebSocket for spatial audio:

1. User positions are synced from the presence WebSocket (opcode 0x43 avatar pose frames)
2. Agent positions are set when agents are spawned in the 3D world
3. LiveKit applies HRTF spatialization based on relative positions
4. Audio volume attenuates with distance (see `SpatialVoiceRouter` in `xr-client/rust/src/webrtc_audio.rs`)

## Related Documentation

- [Voice Integration](voice-integration.md) - TTS/STT WebSocket protocol details
- [XR Architecture](../../explanation/xr-architecture.md) - Multi-user XR setup and collaboration features
