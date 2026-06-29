# Voice System Integration Guide

This guide explains how to integrate with VisionClaw's voice system for speech-to-text (STT), text-to-speech (TTS), and voice command processing.

## Overview

VisionClaw provides a comprehensive voice system that enables:

- **Text-to-Speech (TTS)**: Convert text responses to audio using Kokoro or OpenAI
- **Speech-to-Text (STT)**: Transcribe audio input using Whisper or OpenAI
- **Voice Commands**: Natural language commands for swarm orchestration
- **Conversation Context**: Multi-turn voice interactions with session persistence

## Architecture

```mermaid
flowchart TB
    subgraph Client["Client Application"]
        WS[WebSocket Client]
        AudioIn[Audio Input]
        AudioOut[Audio Output]
    end

    subgraph WebSocket["WebSocket Layer"]
        SpeechSocket[SpeechSocket Handler]
    end

    subgraph SpeechService["Speech Service"]
        TTS[TTS Processor]
        STT[STT Processor]
        VoiceCmd[Voice Command Parser]
        TagManager[Voice Tag Manager]
        ContextMgr[Context Manager]
    end

    subgraph Providers["External Providers"]
        Kokoro[Kokoro TTS]
        Whisper[Whisper STT]
        OpenAITTS[OpenAI TTS]
        OpenAISTT[OpenAI STT]
    end

    subgraph Swarm["Swarm Integration"]
        MCP[MCP Connection]
        Agents[Agent System]
    end

    WS <-->|JSON/Binary| SpeechSocket
    AudioIn --> WS
    WS --> AudioOut

    SpeechSocket --> TTS
    SpeechSocket --> STT
    SpeechSocket --> VoiceCmd

    TTS --> TagManager
    VoiceCmd --> ContextMgr
    VoiceCmd --> MCP
    MCP --> Agents

    TTS --> Kokoro
    TTS --> OpenAITTS
    STT --> Whisper
    STT --> OpenAISTT
```

## WebSocket Connection

### Endpoint

Connect to the speech WebSocket endpoint:

```
ws://<host>:<port>/ws/speech
```

### Connection Lifecycle

```mermaid
sequenceDiagram
    participant C as Client
    participant S as SpeechSocket
    participant SS as SpeechService

    C->>S: WebSocket Connect
    S->>C: {"type": "connected", "message": "Connected to speech service"}

    loop Heartbeat (every 5s)
        S->>C: ping
        C->>S: pong
    end

    Note over C,S: Client timeout: 10 seconds

    C->>S: Close
    S->>C: Close acknowledgement
```

## Message Types

### Text-to-Speech (TTS)

Convert text to audio output.

**Request:**

```json
{
  "type": "tts",
  "text": "Hello, how can I help you today?",
  "voice": "af_sarah",
  "speed": 1.0,
  "stream": true
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | Must be `"tts"` |
| `text` | string | Yes | Text to synthesize |
| `voice` | string | No | Voice identifier (default: `af_sarah`) |
| `speed` | float | No | Playback speed multiplier (default: `1.0`) |
| `stream` | boolean | No | Enable streaming audio (default: `true`) |

**Response:**

Audio data is sent as binary WebSocket frames.

**Error Response:**

```json
{
  "type": "error",
  "message": "Failed to process TTS request: <error details>"
}
```

### Speech-to-Text (STT)

Start or stop audio transcription.

**Start Transcription:**

```json
{
  "type": "stt",
  "action": "start",
  "language": "en",
  "model": "whisper-1"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | Must be `"stt"` |
| `action` | string | Yes | `"start"` or `"stop"` |
| `language` | string | No | Language code (e.g., `"en"`, `"es"`) |
| `model` | string | No | Model identifier |

**Response:**

```json
{
  "type": "stt_started",
  "message": "Transcription started"
}
```

**Stop Transcription:**

```json
{
  "type": "stt",
  "action": "stop"
}
```

**Transcription Result:**

```json
{
  "type": "transcription",
  "data": {
    "text": "Spawn a researcher agent",
    "isFinal": true,
    "timestamp": 1704067200000
  }
}
```

### Audio Data

Send raw audio data as binary WebSocket frames. The system auto-detects the format:

| Format | Header Bytes | MIME Type |
|--------|--------------|-----------|
| WebM | `0x1A 0x45 0xDF 0xA3` | `audio/webm` |
| WAV | `0x52 0x49 0x46 0x46` (RIFF) | `audio/wav` |

### Voice Commands

Send natural language commands to control the swarm.

**Request:**

```json
{
  "type": "voice_command",
  "text": "spawn a researcher agent",
  "sessionId": "abc123-session-id",
  "respondViaVoice": true
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | Must be `"voice_command"` |
| `text` | string | Yes | Natural language command |
| `sessionId` | string | No | Session for context continuity |
| `respondViaVoice` | boolean | No | Send response via TTS |

**Response:**

```json
{
  "type": "voice_response",
  "data": {
    "text": "Successfully spawned researcher agent in swarm default-swarm.",
    "isFinal": true,
    "timestamp": 1704067200000
  }
}
```

## Voice Command Processing

```mermaid
flowchart LR
    subgraph Input
        Speech[User Speech]
    end

    subgraph STT["Speech-to-Text"]
        Transcribe[Transcription]
    end

    subgraph Parser["Command Parser"]
        Intent[Intent Detection]
        Entity[Entity Extraction]
    end

    subgraph Execution
        MCP[MCP Connection]
        Swarm[Swarm Init]
        Agent[Agent Spawn]
        Task[Task Orchestrate]
    end

    subgraph Response
        Format[Response Format]
        TTS[Text-to-Speech]
    end

    Speech --> Transcribe
    Transcribe --> Intent
    Intent --> Entity
    Entity --> MCP
    MCP --> Swarm
    MCP --> Agent
    MCP --> Task
    Swarm --> Format
    Agent --> Format
    Task --> Format
    Format --> TTS
```

### Supported Intents

| Intent | Keywords | Example |
|--------|----------|---------|
| **Spawn Agent** | spawn, add agent | "spawn a researcher agent" |
| **Query Status** | status | "what's the status of all agents" |
| **List Agents** | list agents, show agents | "list all agents" |
| **Stop Agent** | stop agent, remove agent | "stop the researcher agent" |
| **Execute Task** | execute, run | "execute code review task" |
| **Add Node** | add node | "add node called authentication" |
| **Help** | help | "help" |

### Agent Types

Recognized agent types for spawning:

- `researcher`
- `coder`
- `analyst`
- `coordinator`
- `optimizer`

## Configuration

### Kokoro TTS Settings

```json
{
  "kokoro": {
    "apiUrl": "http://kokoro-tts-container:8880",
    "defaultVoice": "af_sarah",
    "defaultFormat": "mp3",
    "defaultSpeed": 1.0,
    "timeout": 30000,
    "stream": true,
    "returnTimestamps": false,
    "sampleRate": 24000
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `apiUrl` | string | `http://kokoro-tts-container:8880` | Kokoro API endpoint |
| `defaultVoice` | string | `af_sarah` | Default voice for synthesis |
| `defaultFormat` | string | `mp3` | Audio output format |
| `defaultSpeed` | float | `1.0` | Default playback speed |
| `timeout` | integer | `30000` | Request timeout (ms) |
| `stream` | boolean | `true` | Enable streaming mode |

### Whisper STT Settings

```json
{
  "whisper": {
    "apiUrl": "http://whisper-webui-backend:8000",
    "defaultModel": "whisper-1",
    "defaultLanguage": "en",
    "timeout": 30000,
    "temperature": 0.0,
    "vadFilter": true,
    "wordTimestamps": false,
    "initialPrompt": null
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `apiUrl` | string | `http://whisper-webui-backend:8000` | Whisper API endpoint |
| `defaultModel` | string | `whisper-1` | Transcription model |
| `defaultLanguage` | string | `en` | Default language |
| `temperature` | float | `0.0` | Sampling temperature |
| `vadFilter` | boolean | `true` | Voice activity detection |
| `wordTimestamps` | boolean | `false` | Include word timing |

### OpenAI Settings

```json
{
  "openai": {
    "apiKey": "sk-...",
    "model": "gpt-4o"
  }
}
```

OpenAI can be used for both TTS and STT as an alternative to Kokoro/Whisper.

## Integration Examples

### JavaScript/TypeScript Client

```typescript
class VoiceClient {
  private ws: WebSocket;
  private audioContext: AudioContext;

  constructor(url: string) {
    this.ws = new WebSocket(url);
    this.audioContext = new AudioContext();

    this.ws.onopen = () => {
      console.log('Connected to speech service');
    };

    this.ws.onmessage = (event) => {
      if (event.data instanceof Blob) {
        // Binary audio data
        this.playAudio(event.data);
      } else {
        // JSON message
        const msg = JSON.parse(event.data);
        this.handleMessage(msg);
      }
    };
  }

  // Send text-to-speech request
  speak(text: string, options?: { voice?: string; speed?: number }) {
    this.ws.send(JSON.stringify({
      type: 'tts',
      text,
      voice: options?.voice ?? 'af_sarah',
      speed: options?.speed ?? 1.0,
      stream: true
    }));
  }

  // Start transcription
  startListening(language?: string) {
    this.ws.send(JSON.stringify({
      type: 'stt',
      action: 'start',
      language: language ?? 'en'
    }));
  }

  // Stop transcription
  stopListening() {
    this.ws.send(JSON.stringify({
      type: 'stt',
      action: 'stop'
    }));
  }

  // Send audio chunk
  sendAudio(audioData: ArrayBuffer) {
    this.ws.send(audioData);
  }

  // Send voice command
  sendCommand(text: string, sessionId?: string) {
    this.ws.send(JSON.stringify({
      type: 'voice_command',
      text,
      sessionId,
      respondViaVoice: true
    }));
  }

  private handleMessage(msg: any) {
    switch (msg.type) {
      case 'connected':
        console.log('Speech service ready');
        break;
      case 'transcription':
        console.log('Transcribed:', msg.data.text);
        break;
      case 'voice_response':
        console.log('Response:', msg.data.text);
        break;
      case 'error':
        console.error('Error:', msg.message);
        break;
    }
  }

  private async playAudio(blob: Blob) {
    const arrayBuffer = await blob.arrayBuffer();
    const audioBuffer = await this.audioContext.decodeAudioData(arrayBuffer);
    const source = this.audioContext.createBufferSource();
    source.buffer = audioBuffer;
    source.connect(this.audioContext.destination);
    source.start();
  }
}

// Usage
const voice = new VoiceClient('ws://localhost:8080/ws/speech');

// Text-to-speech
voice.speak('Hello, I am ready to help you manage your agents.');

// Voice command
voice.sendCommand('spawn a researcher agent', 'session-123');

// Start listening for voice input
voice.startListening('en');
```

### Recording Audio in Browser

```typescript
async function startRecording(voiceClient: VoiceClient) {
  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  const mediaRecorder = new MediaRecorder(stream, {
    mimeType: 'audio/webm;codecs=opus'
  });

  mediaRecorder.ondataavailable = async (event) => {
    if (event.data.size > 0) {
      const arrayBuffer = await event.data.arrayBuffer();
      voiceClient.sendAudio(arrayBuffer);
    }
  };

  // Start transcription on server
  voiceClient.startListening();

  // Record in chunks
  mediaRecorder.start(250); // 250ms chunks

  return {
    stop: () => {
      mediaRecorder.stop();
      voiceClient.stopListening();
      stream.getTracks().forEach(track => track.stop());
    }
  };
}
```

## Data Flow

### TTS Data Flow

```mermaid
sequenceDiagram
    participant C as Client
    participant WS as WebSocket
    participant SS as SpeechService
    participant K as Kokoro/OpenAI

    C->>WS: {"type": "tts", "text": "Hello"}
    WS->>SS: TextToSpeech command
    SS->>K: POST /v1/audio/speech
    K-->>SS: Audio stream
    SS-->>WS: Binary audio chunks
    WS-->>C: Binary frames
```

### STT Data Flow

```mermaid
sequenceDiagram
    participant C as Client
    participant WS as WebSocket
    participant SS as SpeechService
    participant W as Whisper/OpenAI

    C->>WS: {"type": "stt", "action": "start"}
    WS->>C: {"type": "stt_started"}

    loop Audio streaming
        C->>WS: Binary audio data
        WS->>SS: ProcessAudioChunk
        SS->>W: POST /transcription/
        W-->>SS: {"identifier": "task-123"}
        SS->>W: GET /task/task-123 (polling)
        W-->>SS: {"status": "completed", "result": [...]}
        SS-->>WS: Transcription text
        WS-->>C: {"type": "transcription", "data": {...}}
    end

    C->>WS: {"type": "stt", "action": "stop"}
    WS->>C: {"type": "stt_stopped"}
```

### Server-Side Transcription Consumers (ADR-110)

Every transcription line is also fanned out server-side via
`SpeechService::subscribe_to_transcriptions()` (a `tokio::broadcast` channel).
Two actors tap this stream — conversation inside the immersive interface
guides both knowledge elevation and interface configuration:

```mermaid
sequenceDiagram
    participant SS as SpeechService
    participant BC as broadcast::Sender<String>
    participant EV as ElevationActor<br/>(elevation_voice.rs harvest)
    participant VI as VoiceInterfaceActor<br/>(voice_interface_actor.rs)
    participant SA as Settings Assistant<br/>(settings_assistant_task → agentbox LLM)
    participant ACSP as ACSP Producer<br/>(forum broker case)

    SS->>BC: transcription line (unattributed)
    BC-->>EV: recv()
    Note over EV: harvest_mentions(): n-gram match against<br/>elevatable concept index (frontier stubs + pages)<br/>→ VoiceDemandLedger (30-min half-life decay)<br/>→ PRIMARY ranking for elevation candidates
    Note over EV: explicit commands jump the queue:<br/>"elevate X", "formalise X", "make X a class"<br/>→ confirmed aloud via Kokoro TTS
    EV->>ACSP: kind-31402 knowledge_enrichment case<br/>(voice provenance: mention counts, excerpts, priority high)

    BC-->>VI: recv()
    Note over VI: parse_interface_intent(): conservative gate —<br/>a CONFIG_VERB and an INTERFACE_NOUN must BOTH match<br/>("hide the ontology nodes", "increase spring strength")
    VI->>SA: CreateTask (same assistant the Control Center<br/>command box drives) → PUT /api/settings/<path>
    VI->>SS: text_to_speech("Adjusting the interface.")
```

Ordinary conversation that mentions graph concepts feeds elevation demand
only; it is never hijacked into settings changes (the verb+noun gate keeps
the two consumers disjoint).

### Voice Command Flow

```mermaid
sequenceDiagram
    participant C as Client
    participant WS as WebSocket
    participant VC as VoiceCommand Parser
    participant CM as Context Manager
    participant MCP as MCP Connection
    participant S as Swarm

    C->>WS: {"type": "voice_command", "text": "spawn researcher"}
    WS->>VC: Parse command
    VC->>CM: Get/create session
    CM-->>VC: Session context
    VC->>MCP: call_swarm_init()
    MCP->>S: Initialize swarm
    S-->>MCP: Swarm ID
    VC->>MCP: call_agent_spawn("researcher")
    MCP->>S: Spawn agent
    S-->>MCP: Agent created
    VC->>CM: Store operation
    VC-->>WS: Response text
    WS-->>C: {"type": "voice_response", "data": {...}}
```

## Voice Tag System

The voice tag system enables tracking voice commands through the agent system:

```mermaid
flowchart TB
    subgraph TagLifecycle["Tag Lifecycle"]
        Create[Create Tag]
        Track[Track Command]
        Process[Process Response]
        Cleanup[Cleanup Expired]
    end

    subgraph States["Command States"]
        Pending[Pending]
        Processing[Processing]
        Completed[Completed]
        TimedOut[Timed Out]
        Failed[Failed]
    end

    Create --> Pending
    Pending --> Processing
    Processing --> Completed
    Processing --> TimedOut
    Processing --> Failed

    Completed --> Cleanup
    TimedOut --> Cleanup
    Failed --> Cleanup
```

Tags enable:
- Correlation of voice commands with agent responses
- Automatic TTS routing for responses
- Session-based context preservation
- Timeout and cleanup management

## Session Management

Voice sessions maintain conversation context:

```json
{
  "sessionId": "abc123",
  "userId": "user-456",
  "conversationHistory": [
    ["spawn a researcher agent", "Successfully spawned researcher agent."],
    ["what's the status", "System status: 1 agent active."]
  ],
  "currentAgents": ["researcher"],
  "pendingOperations": [],
  "turnCount": 2
}
```

Sessions automatically:
- Track conversation history
- Remember spawned agents
- Monitor pending operations
- Expire after 2 hours of inactivity

## Troubleshooting

### Connection Issues

| Symptom | Cause | Solution |
|---------|-------|----------|
| WebSocket disconnect | Heartbeat timeout | Ensure client responds to pings within 10s |
| "Speech service not available" | Service not initialized | Check that `speech_service` is configured in app state |
| Connection refused | Wrong endpoint | Verify WebSocket URL `/ws/speech` |

### TTS Issues

| Symptom | Cause | Solution |
|---------|-------|----------|
| "Kokoro API error" | API unreachable | Check Kokoro container is running |
| No audio output | Stream not enabled | Set `stream: true` in request |
| Invalid voice | Unknown voice ID | Use supported voice like `af_sarah` |

### STT Issues

| Symptom | Cause | Solution |
|---------|-------|----------|
| "Whisper API error" | API unreachable | Check Whisper container is running |
| No transcription | Audio format | Ensure WebM or WAV format |
| Timeout polling | Long audio | Increase timeout or send shorter chunks |

### Voice Command Issues

| Symptom | Cause | Solution |
|---------|-------|----------|
| "Could not determine agent type" | Missing keyword | Include agent type in command |
| "Unknown message type" | Invalid `type` field | Use `tts`, `stt`, or `voice_command` |
| MCP connection failed | Network issue | Check MCP_HOST and MCP_TCP_PORT env vars |

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MCP_HOST` | `multi-agent-container` | MCP server hostname |
| `MCP_TCP_PORT` | `9500` | MCP server port |

## Related Documentation

- [Agent Orchestration Guide](../agent-orchestration.md)
- [WebSocket API Reference](../../reference/websocket-protocol.md)
- [Configuration Reference](../../reference/configuration.md)
