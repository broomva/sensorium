# MIL Roadmap & Competitive Landscape — 2026-05-13

**Status**: Draft — research synthesis. Roadmap tracks defined; Linear tickets to be minted on adoption.
**Owner**: MIL substrate (`broomva/sensorium` + `broomva/pneuma`).
**Scope**: Maps the May 2026 voice / pointer / multimodal-input agent landscape and turns the gaps into a 9-month MIL roadmap with concrete crates, backends, and product surfaces.
**Method**: Six parallel research waves dispatched 2026-05-13. Source bibliography embedded inline per axis. ~16,000 words of raw research distilled.

## TL;DR

MIL has shipped its streaming substrate end-to-end (B1–B4, 5 PRs, all green). The competitive landscape has converged on **two dominant architectures that share one weakness**: cloud-blob voice agents (OpenAI Realtime-2, Gemini Live, Hume EVI) and composed open-source pipelines (LiveKit Agents, Pipecat). Neither side ships a **typed agent contract** that downstream programs reason over. Every shipping product treats voice output as opaque text or pixel coordinates. Apple App Intents is the closest typed-deictic-substrate in production, but Apple owns the runtime+OS+index.

**MIL's wedge is the cross-vendor portable typed substrate.** The strategy is the same one Spec E makes at the silicon layer: own the contract, let backends compete underneath. Concretely this means seven roadmap tracks:

| Track | Scope | Window |
|---|---|---|
| **C** | Streaming substrate completion (B5 dialog crate + barge-in + provenance event log) | 2026 Q3 |
| **B** | Voice backend optionality (Kyutai STT, Apple SpeechAnalyzer, Whisper-turbo, Moonshine, OpenAI Realtime, Gemini Live) | 2026 Q3-Q4 |
| **T** | TTS substrate (`sensorium-speech` + Sesame CSM 1B via `csm.rs` + Kokoro edge) | 2026 Q3-Q4 |
| **V** | Vision substrate (`sensorium-vision` + ScreenCaptureKit + Moondream 3 + Florence-2 + VLM-as-backend) | 2026 Q4-2027 Q1 |
| **N** | Non-voice modalities (`sensorium-hands` Mediapipe MVP → `sensorium-gaze` Tobii → `sensorium-arkit` Vision Pro) | 2026 Q4-2027 Q2 |
| **P** | Provenance / cryptographic trust (Ed25519 signing, audit-trail UX, replayable sessions) | 2026 Q4 |
| **G** | Product surfaces on top (live-realtime demo polish, Pointer SDK, voice keyboard firmware) | 2027 |

The decisive insight: the first system to ship **typed-on-device substrate with cryptographic provenance** is competing in a different category than any of them — closer to a **Yubico USB security key** than to a lapel mic, closer to **CUDA** than to a cloud LLM. That's the bet.

---

## 1. Where MIL Is Today (post-B4)

| Layer | Crate | Status | Last delivery |
|---|---|---|---|
| Substrate types | `sensorium-core` | Generation + StreamUpdate substrate | B1 (sensorium#7, `f2079d6`) |
| Voice STT backend | `sensorium-voice` | Parakeet TDT EOU on-device + Mock + streaming session | A1/A2/B2/B-detokenize (sensorium#5/6/8/9) |
| Workspace context | `sensorium-context-macos` | Focused app/window/file metadata | shipped |
| Typed directive | `pneuma-core` | Directive typestate + Generation field | B3 (pneuma#16, `036e1e4`) |
| Deictic resolver | `pneuma-resolver` | Anaphor → ReferentValue + bridge_generation | shipped + B3 |
| Demo runtime | `pneuma-demo` | Streaming voice path with realtime partials + speculative parse markers + WAV replay validation + MIL_VOICE_TRACE diagnostics | B4 (pneuma#17, `ebbacb0`) |

**What works end-to-end (validated via WAV replay 2026-05-13):**

```text
mic → cpal → 16kHz f32 mono → ringbuf → EnergyVad → VadGate
  → ParakeetStt (streaming Partials @ 160ms) → SentencePiece detokenize
  → StreamUpdate<TranscriptDelta> {generation, value}
  → demo render with speculative parse marker (→ file.rename)
  → on Final: parse_utterance → Directive<Composing> → resolve_directive → run_rename_flow
```

**What's missing (the immediate B5 work + this roadmap):**
- Orchestration crate (`pneuma-dialog`) extracting the inline VAD loop into a reusable engine driver
- Barge-in via `VoiceSession::cancel()` (substrate ready, no caller yet)
- Speculative directive composition on Partial (currently only *parsing* on partials; not yet composing+resolving)
- Streaming agent kickoff (Arcan begins composing while user is still speaking)
- All non-voice modalities (gaze, gesture, vision, BCI)
- All vendor optionality (only Mock + Parakeet today; no Whisper, no Apple, no cloud)
- All TTS / output (no `sensorium-speech` yet; no full-duplex)
- All cryptographic provenance (provenance struct exists, no signing)

---

## 2. Methodology

Six parallel research agents dispatched 2026-05-13, each scoped to one axis of the competitive landscape:

1. **HW voice peripherals + AI wearables** — Rabbit R1, Humane (status), Friend, Plaud, Bee, Limitless, Omi, Polycast5, Sandbar, Pebble, Subtle, MindClip, Soundcore Work, OpenAI/Ive io
2. **Desktop voice-typing AI** — Wispr Flow, Superwhisper, MacWhisper, Aqua, Willow, Talon, Apple Voice Control, OpenWhispr, Dictato, Wspr, voice-AI-agent category
3. **Pointer / screen-context / deictic AI** — DeepMind AI Pointer, Microsoft Recall, Apple App Intents, Anthropic Computer Use, OpenAI CUA, Cursor/VS Code, Rewind/Limitless, Granola, Recall.ai, Cleft, Screenpipe, GUI-agent research
4. **Realtime voice agent platforms** — OpenAI Realtime-2 family, Gemini Live, Sesame CSM, Hume EVI, Retell, Vapi, LiveKit Agents, Pipecat, Daily Bots, Bland, Cartesia, Deepgram, Speechmatics Flow, Kyutai Moshi, Ultravox, ElevenLabs, Twilio ConversationRelay, Parakeet v3
5. **BCI / gaze / gesture / multimodal** — Karyal/Emotiv, Neurable, Cognixion, Naqi, Synchron, AlterEgo, OpenBCI/Galea/Muse, Tobii 5L, Vision Pro, iPad eye tracking, WebGazer, Mediapipe, Ultraleap, Quest, Neural Band, Pison, Wisear, HoloLens, Neuralink
6. **On-device speech / vision / TTS models** — Parakeet variants, Whisper local/MLX/cpp, Moonshine, Kyutai STT/Moshi/TTS, Sherpa-ONNX, Granite Speech, Apple SpeechAnalyzer + Foundation Models, Android Gemini Nano, Moondream/Florence-2/FastVLM/Qwen2.5-VL/MiniCPM-V, Sesame CSM 1B, Kokoro 82M, StyleTTS2, Mimi, Piper, AVSpeechSynthesizer

Each agent returned a summary table + per-product detail + synthesis (~1.5-2k words). This document distills.

---

## 3. The Landscape — Six Axes

### Axis 1: HW voice peripherals & wearables

| Product | Form | Compute | Status | Price |
|---|---|---|---|---|
| Humane AI Pin | Lapel projector | Cloud | **DEAD** Feb 2025 ($116M HP acquihire) | RIP |
| Friend.com | Pendant | Cloud (Gemini) | Flop (~$348K rev on 1K shipped) | $129 |
| Plaud Note Pro / NotePin | Puck / pin | Cloud (GPT-5.2/Claude/Gemini) | Market leader (~1.5M users) | $99-$189 + $99/yr |
| Rabbit R1 | Standalone box | Cloud (LAM) | Shipping, pivoting to DLAM | $199 |
| Bee | Wristband | Cloud | Acquired by Amazon Jul 2025 | $49.99 + $19/mo |
| Limitless Pendant | Lapel clip | Cloud + AES at rest | Acquired by Meta Dec 2025, sales paused | $199-$299 |
| Omi (Based HW) | Necklace orb | **Hybrid, OSS, opt-local** | Shipping; EEG module 2026-27 | $89 / $70 dev |
| Polycast5 (RoboticWorx) | ESP32-C5 handheld | ESP32 + cloud STT, **BLE HID out** | Kickstarter pre-launch | TBD |
| Sandbar Stream Ring | Smart ring | Cloud + "Inner Voice" TTS | Preorder, summer 2026 | $249-$299 + $10/mo |
| **Pebble Index 01** | Smart ring | **On-device LLM on phone** | Preorder, Mar 2026 | **$75 (no sub)** |
| Subtle Voicebuds | Earbuds | On-device + cloud sub | CES 2026 | $199 + $17/mo |
| Soundcore Work (Anker) | Coin clip | Cloud + AES local | Shipping | $99-$159 + $99/yr |
| OpenAI/Jony Ive "io" | Behind-ear / pen | Cloud (40-50M unit target) | Slipping H2 2026 → Feb 2027 | TBD |

**Pattern across nearly every shipping device**:

```text
mic → BLE → phone → cloud STT+LLM+summary → SaaS subscription
```

Form factor is the only real differentiator. Compute placement is identical. Every "second brain" is one acquisition or shutdown from bricking (Bee → Amazon, Limitless → Meta, Humane → HP/dead).

**Exceptions of note**:
- **Pebble Index 01** — only no-subscription, on-device-LLM-on-phone device. Won Best of CES 2026 (Android Authority).
- **Omi** — only major OSS hardware, can run cloud-free.
- **Polycast5** — only device whose primary output mode is *BLE HID injection into a host computer*. Closest spiritual ancestor to a "MIL voice keyboard."

**Three MIL opportunities at this layer**:
1. **Provenance-First Recorder** — Sign every utterance at the edge with Ed25519 over audio+transcript frames. Users keep keys. Unblocks healthcare/legal/journalism segments structurally banned from Plaud/Bee. No competitor can prove a transcript is real. Limitless "Consent Mode" gestures at this, doesn't sign.
2. **Voice-to-Typed-Substrate BLE HID Layer** — Polycast5's BLE injection but typed. On-device STT → typed intent classifier (Rust, deterministic schema) → BLE HID injection of *structured data* into focused app, not raw dictation. The desktop voice typing tools (Wispr/Willow/Superwhisper) operate at the *string* level; they don't understand "this is a calendar entry" vs "this is `cargo run`."
3. **Composable Open-Substrate Pendant** — Rust-native pendant + published typed intent schema + signed events + BLE stream to any host + MCP-style action bus. Platform play >> hardware margin. Device is just the cheapest provenanced input the agent OS can have.

### Axis 2: Desktop voice-typing AI

| Product | Platform | STT | Compute | Acts on text | Price |
|---|---|---|---|---|---|
| **Wispr Flow** | Mac/Win/iOS/Android | Proprietary cloud | Cloud | Yes (Command Mode + style match) | $15/mo, $81M raised |
| **Superwhisper** | Mac/Win/iOS | Whisper local + cloud LLM BYOK | Hybrid | Yes (Custom Modes) | $9.99/mo or $249 lifetime |
| **MacWhisper** | Mac/iOS | Whisper local | Local | Light (summary) | €59-79 lifetime |
| **Whisper Memos** | iOS / Apple Watch | Whisper API + ElevenLabs Scribe | Cloud | Yes (Agents → Notion/Todoist/Things 3) | $5/mo |
| **Aqua Voice** | Mac/Win/iOS | Proprietary Avalon | Cloud | Light | $8/mo, YC, unfunded per Tracxn |
| **Willow Voice** | Mac/Win/iOS/Android | Proprietary + Llama | Cloud (opt-local on Mac/iOS) | Yes ("Hey Willow") | Sub, $4.2-4.5M raised |
| **Talon Voice** | Mac/Win/Linux | Bundled Conformer (+ Dragon-compat) | **Local** | **Python scripting + Tobii eye-tracking** | Free / Patreon ~$25/mo |
| **Apple Voice Control + Dictation** (macOS 26 Tahoe) | macOS | **SpeechAnalyzer + Apple Intelligence** | **On-device** | Yes (AI rewrite) | Free OS-bundled |
| **OpenWhispr** | Mac/Win/Linux | whisper.cpp + Parakeet + BYOK cloud | Hybrid | Yes (AI cleanup) | Free, MIT |
| **Dictato** | Mac | Parakeet (+ Whisper, + Apple) | **Local** | Light | Paid one-time |
| **Wspr / Weesper Neon Flow** | Linux + Mac | Whisper local | Local | Light | $14.99 one-time |

**Dominant patterns**:
- **Pattern A — Whisper-class STT + cloud LLM rewrite** (Wispr, Willow, Aqua, Superwhisper-cloud, Whisper Memos): audio → cloud STT → cloud LLM cleans filler/punctuation/style → insert at cursor via OS accessibility API.
- **Pattern B — Local Whisper / Parakeet** (Superwhisper-local, MacWhisper, OpenWhispr, Dictato, Wspr, Apple): same shape, cloud replaced by 1-4GB local model.
- **Pattern C — Talon outlier**: local Conformer + scripted DSL, treating voice as *input* (replaces keyboard+mouse) rather than text. The only spiritual ancestor to MIL's approach.

**Shared weakness across all of them**:
Every product treats output as **opaque text inserted at a cursor**. No shared typed substrate. No provenance ("this token came from your voice at t=842ms, with 0.91 confidence, in app=Slack"). No deictic resolution ("delete *that*" works only inside Wispr Command Mode on selected text — never against on-screen entities). No layered separation of *transcription* from *intent* from *action*. Privacy modes are bolt-ons. The voice-agent category proves intent+action is possible but discards the desktop dictation surface entirely.

**Three MIL opportunities at this layer**:
1. **Provenance-first dictation** for legal/medical/government where Wispr is structurally banned and Apple's opacity is unacceptable.
2. **Deictic / multimodal resolution layer** — "move *this* paragraph after *that* heading" resolved against actual screen AX-tree + selection + gaze (Tobii hook from Talon's playbook), with typed `Intent { verb, theme, locus }` IR.
3. **Layered runtime with hard boundaries** — local Whisper/Parakeet at L0, typed IR at L1, agent/tool dispatch at L2. Each layer independently swappable, replayable, policy-gated. Sell the substrate to OSS skill authors (Talon-style scripting), to enterprises (replayable compliance), to vertical apps.

### Axis 3: Pointer / screen-context / deictic AI

| Product | Deictic mechanism | Output | Compute | Typed? |
|---|---|---|---|---|
| **DeepMind AI Pointer / Magic Pointer** | Cursor + screen-capture VLM (Gemini) | Contextual actions | Cloud | No |
| **Microsoft Recall** | Periodic snapshots + on-device OCR/vector index | Search results | **100% on-device (NPU)** | No |
| **Apple App Intents + Personal Context** | Semantic index + App Intents typed donations | Structured App Intents → app actions | On-device + Private Cloud Compute | **YES — closest production typed-deictic contract** |
| **Anthropic Computer Use** | Pixel coords on screenshot, claude counts pixels | `click(x,y)`, `type`, `scroll`, `zoom` | Cloud | Partial (pixel-keyed tool schema) |
| **OpenAI Operator / ChatGPT Agent** | CUA: raw screenshot + RL-trained GUI perception | Mouse + keyboard in cloud VM | Cloud (VM) | Partial |
| **Cursor `@-refs` + Cmd+K** | Selection + `@codebase`/`@file`/`@symbol` | Inline diff | Cloud | **Partial — `@`-grammar is typed** (best deictic substrate currently shipping in production) |
| **VS Code Copilot Inline Chat** | Selection + `#selection` variable | Inline diff | Cloud | Partial |
| **Rewind / Limitless** | Local screen+audio capture + AI search | Search/summaries | Local→cloud hybrid | No |
| **Granola** | User-typed anchor + transcript | Markdown notes | Cloud | No |
| **Recall.ai** | Bot or Desktop Recording SDK + participant events | Recordings + structured metadata | Cloud | **Yes (event/timestamp schema)** |
| **Screenpipe** | OS-native OCR (Apple Vision / Windows OCR / Tesseract) + Whisper audio | Vector search + "Pipes" actions | **100% local** | Partial |
| **Google Workspace Gemini side panel** | Selection + Workspace Intelligence index | Inline edits | Cloud | No |
| **GUI-agent research** (ScreenSpot/OSWorld/UGround/GUI-Actor/MEGA-GUI/Fara-7B) | Pixel-grounding VLMs, SoM, region zoom, coord-free attention | Click coords / element IDs | Mixed | No |

**The market splits into two camps that don't talk to each other**:
- **Camp A — Pixel-grounded VLMs** (DeepMind AI Pointer, Anthropic Computer Use, OpenAI CUA, all academic GUI-agent work): treats "this" as a *vision problem*. Snapshot → multimodal model → coords or freeform action.
- **Camp B — Typed entity substrates** (Apple App Intents, Cursor `@-refs`, VS Code `#selection`, Granola/Recall.ai event schemas): treats "this" as a *symbol problem*. Host donates named entities; LLM picks one.

Neither camp covers what users actually mean by "this": **the union of "what eyes are on" + "what the OS says is focused"**. DeepMind's Magic Pointer is the first product to attempt fusion, but only inside Google-owned surfaces (Chrome, Googlebook).

**The cautionary tale**: Rewind / Limitless acquired by Meta Dec 19 2025, folded into Reality Labs, EU/UK service cut. Standalone local screen capture as a business doesn't sustain. **Lesson for MIL: vertical integration with OS or editor is the durable path; standalone capture isn't.**

**The market validation**: Granola hit $1.5B valuation March 2026 ($125M Series C). User-typed anchor + ambient context wins.

**The research bottleneck**: best GUI-grounding on ScreenSpot-Pro is ~74% (MVP+Qwen3VL-32B), ~25 points behind human. End-to-end OSWorld at ~27%. **Hybrid (typed substrate for the unambiguous 70%, vision for hard 30%) is the right architecture.**

**Four MIL opportunities at this layer**:
1. **MIL Vision Adapter** — typed `intent.act_on(target = pixel_region(bbox, screenshot_id))` wrapping any VLM (Claude Computer Use, CUA, Fara-7B, UGround). Composes with Spec E inference cluster.
2. **MIL Screen-Memory Resolver** — typed-event log replacing dead Rewind pattern. OS-native OCR per Screenpipe stack.
3. **MIL App Intents Bridge** — auto-generate Apple App Intents donations from MIL typed schema; symmetric Windows AppActions. Ships into Siri/Spotlight/Visual Intelligence + Windows Copilot for free.
4. **MIL Pointer SDK** — the substrate Magic Pointer should have been. Typed targets (`MilPointable<T>`); thin VLM maps pointer+speech to typed target. Hybrid resolution: typed for 70%, vision for 30%.

### Axis 4: Realtime voice agent platforms

| Platform | Type | Latency m2m | Pricing | Notes |
|---|---|---|---|---|
| **OpenAI Realtime-2** | Fused S2S | ~300-500ms low effort | $32/M in, $64/M out (~$0.18-0.46/min) | Parallel tools, Preambles, adjustable reasoning |
| **OpenAI Realtime-Translate** | Translation | <1s | $0.034/min | 70 input / 13 output langs |
| **OpenAI Realtime-Whisper** | STT | <1s | $0.017/min | Eats Deepgram price point |
| **Gemini Live 3.1 Flash Native Audio** | Fused S2S | ~600ms | Per-token | Native multimodal video; "proactive audio" smart barge-in |
| **Sesame CSM-1B** | TTS only | Sub-s | Free | **Apache 2.0**, Llama backbone + Mimi RVQ |
| **Hume EVI** | Fused + prosody | ms-class TTFB | $3-$500/mo | Only platform that natively types affect signals |
| **Retell AI** | Phone agents | 600-1500ms real | $0.07/min engine + LLM | WS + SIP |
| **Vapi.ai** | Phone orchestrator | 500-800ms | $0.05/min orchestration | Pluggable cascade, Squads multi-agent |
| **LiveKit Agents** | Open framework | 250-700ms cascade | Self-host free | **Apache 2.0**, WebRTC native, semantic turn detection, MCP-native |
| **Pipecat** | Open framework | ~1s open models | Free self-host | **BSD-2-Clause**, frame-based async, 12k stars |
| **Daily Bots** | WebRTC infra | 13ms first-hop | Per-min | Best network floor on OSS side |
| **Bland.ai** | Phone (vertical) | 400-1500ms | $0.09/min flat | Pathways graph DSL, on-prem |
| **Cartesia Sonic-3** | TTS only | 40ms TTFB | $0.03/min | **State-Space-Model**, fastest streaming TTS |
| **Deepgram Nova-3 + Flux** | STT only | <300ms | $0.0077/min PAYG | Flux = separate EOU model |
| **Speechmatics Flow** | Fused | <300ms | Per-min | **Closest analog to MIL typed Directive lifecycle** (`StartConversation/ResponseStarted/Interrupted/Completed/ToolInvoke`) |
| **Kyutai Moshi** | Open S2S | 200ms practical | Free | MIT/Apache code, **CC-BY-4.0 weights (gray area)** |
| **Ultravox (Fixie)** | Audio-in LLM | 150ms TTFT | Apache 2.0 weights | Cuts ASR latency entirely in input direction |
| **ElevenLabs Conv. AI** | Fused | 75ms TTFA | $0.08-$0.12/min | Best voice quality + brand |
| **Twilio ConversationRelay** | Carrier glue | Network-bound | Per-min + voice | PSTN/SIP + WS, model-neutral |
| **Parakeet TDT 0.6B v3** | STT only | 250ms partials | **Free, on-device** | RTFx ~3380, 25 European langs |

**Two architectures have hardened**:
- **(a) Fused cloud blob** (OpenAI / Gemini / Hume / ElevenLabs): collapses STT+LLM+TTS into one model behind one WebSocket. Advantages: latency, prosody. Cost: total lock-in.
- **(b) Composed open-source pipeline** (LiveKit / Pipecat / Vapi / Twilio): mix-and-match models per layer. Advantages: composability, redaction. Cost: cascade latency, glue complexity.

**Unfilled gaps**:
1. **Latency floor is physics + network**, not architecture. Nobody serious about on-device LLM step yet.
2. **Privacy/edge**. Open weights exist for every stage (Parakeet STT, Llama/Qwen LLM, CSM/Moshi/Pocket TTS) but no credible turnkey on-device voice agent stack ships.
3. **Typed agent contracts**. Every platform has tool calling, but tool calling = JSON-schema args, not a *lifecycle contract*. **No platform exposes a typed directive lifecycle the agent program reasons over.** Speechmatics Flow is closest at protocol level; Bland Pathways is closest at DSL level; neither is composable substrate.
4. **Generation tagging / partial reasoning**. OpenAI Preambles are a UX hack. Pipecat has internal frame types. Nobody exposes generation provenance — what was streamed vs final, tool vs synthesis, draft vs committed — as a first-class typed signal.

**Why MIL is differently positioned**: MIL — Rust-native, on-device, typed-substrate intent layer with streaming voice partials, generation tagging, and a directive contract — sits exactly in the gaps above. The market converged on either "give us audio, we give back audio" (cloud blob, no substrate) or "here are processors, wire them up" (Pipecat/LiveKit composability, no contract). **Nobody has the substrate.**

### Axis 5: Non-voice modalities (BCI / gaze / gesture / sEMG)

| Modality | Bandwidth | `PrimitiveToken` fit | 12-month viability |
|---|---|---|---|
| **Mediapipe hand-tracking** (RGB cam) | Continuous + 7 gesture classes | `Modulation` + `Approval` | **Ready now, free, cross-platform** |
| **Tobii 5L** ($330) | 120 Hz gaze + pupil | `Attention` + `Reference` (high-fi) | **Ready now**, Streams/Pro SDK |
| **Apple Vision Pro ARKit hand** | 90 Hz skeletal | `Modulation` | **Ready** (raw gaze gated by Apple) |
| **Ultraleap Leap 2 + Hyperion** ($140) | Sub-ms IR microgestures | `Modulation` + `Approval` | Ready, precision-grade |
| **Meta Quest hand v83** | 60-90 Hz + microgestures | `Modulation` + `Approval` | Quest-only |
| **Meta Neural Band sEMG** ($799 w/ Ray-Ban) | sub-ms pre-motor | `Modulation` + `Approval` | **Watershed** — first general-pop sEMG w/ no per-user training, OSS pretrained code. Quest SDK only today. |
| **WebGazer.js** | Webcam, coarse | `Attention` (low-fi) | Free, maintained-as-is |
| **EEG** (Karyal/Muse/OpenBCI/Neurable/Cognixion) | Per-user noisy, low | `State` + slow `Approval` | Not addressable for general pop in 12mo |
| **AlterEgo subvocal sEMG** | Mid-bandwidth | `Predication` (only viable non-voice text channel) | 2027-2028, pre-commercial |
| **Synchron Stentrode / Neuralink** | Implant | n/a | Accessibility-only |
| **HoloLens 2 / MRTK3** | n/a | n/a | **DEAD** — production ended Q4 2024 |

**Cleanest MVP for visible non-voice substrate growth**: **Mediapipe hand-tracking via webcam → `sensorium-hands` Rust crate** emitting `Modulation` (continuous landmarks) + `Approval` (7 gesture classes: closed_fist=stop, thumb_up=commit, open_palm=cancel, ...). One developer-week. Zero hardware purchase. Cross-platform (Mac/Win/Linux/iOS/Android). Same A1→A2→A3 shape we did for voice. **Free, ubiquitous, ships immediately.**

**Three scoping options for MIL non-voice tracks**:
1. **2026 Q4 ship: `sensorium-hands`** — Mediapipe via TFLite FFI. Cost: $0. Value: cross-platform `Modulation` + `Approval` source for "the camera saw you nod" / "you pinched."
2. **2027 Q1 ship: `sensorium-gaze` + `sensorium-arkit`** — Tobii 5L desktop + Apple Vision Pro hand tracking. Cost: $150-$3500 HW per dev. Value: high-fidelity `Attention` + `Reference`.
3. **2027 Q3+ ship: `sensorium-emg` + `sensorium-subvocal`** — Meta Neural Band (when/if Meta opens non-Quest SDK) + AlterEgo for `Predication`. Pre-motor `Modulation` + subvocal language. Dependent on third-party SDK opening.

The Neurable / Cognixion / Naqi / Wisear OEM-licensing path means a `sensorium-bci` crate is only viable once one of these vendors opens a public SDK — currently none have. Plan for 2027+ on EEG/BCI.

### Axis 6: On-device speech / vision / TTS models

**STT — Ranking alternatives to current Parakeet TDT EOU**:

| Model | License | Rust integration | Strategic role |
|---|---|---|---|
| **Parakeet TDT 0.6B v3** | CC-BY-4.0 weights | `parakeet-rs` (ort) ✅ | **Drop-in multilingual upgrade** from our current v2 (25 EU langs) |
| **Parakeet RealTime EOU 120M** | CC-BY-4.0 | parakeet-rs ✅ | Sub-500MB streaming variant; edge-tier |
| **Kyutai STT 1B / 2.6B** | weights CC-BY-4.0; **Rust code Apache-2.0, first-class server in upstream repo** | `stt-rs` ✅✅ | **Best secondary backend**. DSM architecture (delayed streams modeling) — flip delay → swap STT for TTS, single backend family. Semantic VAD predicts user-done-talking. 400 concurrent streams on H100. |
| **Apple SpeechAnalyzer** (macOS/iOS 26) | Proprietary | Swift only; `SpeechAnalyzerDylib` C-FFI exists ⚠️ | **Strategically most important** — reportedly 2× faster than Whisper-large-v3-turbo at parity quality, fully on-device, model assets in system catalog (zero app-bundle cost). Should become the **default on macOS 26+**. |
| **Whisper-large-v3-turbo** | MIT | `whisper-rs` ✅ | Reference baseline, third backend, multilingual ceiling |
| **MLX Whisper / Lightning** | MIT | `voice-stt` (MLX FFI) ⚠️ | Mac fastpath flag, 36× RT on M1 |
| **Moonshine v2** | MIT | `voice-stt`, ONNX, CT2 ⚠️ | Edge tier (26-200MB), native streaming, input-length-scaled compute |
| **Kyutai Moshi (full s2s)** | CC-BY-4.0 weights / Apache Rust | Candle Q8/BF16 ✅ | Research backend; only viable open full-duplex |
| **Sherpa-ONNX** | Apache-2.0 | First-class Rust API since 2025 ✅ | Embedded/edge umbrella, also covers TTS + diarization + VAD |
| **IBM Granite Speech 3.3** | Apache-2.0 | No first-class Rust ❌ | Server-class accuracy only |
| **Android ML Kit GenAI Speech / Gemini Nano** | Proprietary | JNI only ❌ | Out of scope for Rust-native MIL |

**VLMs for `sensorium-vision`** (future):

| Model | License | Strategic role |
|---|---|---|
| **Moondream 3 Preview** | Apache-2.0 / open | **Primary candidate** — 9B MoE / 2B active. **Dedicated grounding tokens** (one token per coord) → near-instantaneous bbox/point outputs. ScreenSpot F1@0.5 = 80.4. MLX-native. |
| **Florence-2 (0.77B)** | MIT | **Complementary** — best phrase grounding (FLD-5B training). Fixed-prompt tasks (precise OCR, document layout) where Moondream's free-form chat is overkill. CPU-runnable. |
| **Apple FastVLM (0.5B–7B)** | Apple Sample Code License (permissive) | **Future macOS-native pick** once Candle/ort port lands. 85× faster TTFT than LLaVA-OneVision-0.5B. iPhone-runnable. |
| **Qwen2.5-VL 3B/7B** | Apache-2.0 | General-purpose VLM, no native Rust |
| **MiniCPM-V 4.5** | Apache (code) / model commercial-registered | Mobile-first |

**TTS for `sensorium-speech`** (future):

| Model | License | Rust | Strategic role |
|---|---|---|---|
| **Sesame CSM 1B** | Apache-2.0 (code + weights) | **`cartesia-one/csm.rs` production Candle impl, GGUF q8/q4_k, Metal/CUDA/Accelerate, OpenAI-compat API** ✅✅ | **Primary candidate**. Drop-in. RVQ on Llama backbone. Approaches human MOS. |
| **Kokoro-82M** | Apache-2.0 | Candle/ort port viable ⚠️ | **Secondary edge tier**. 82M params, MOS 4.5+, 54 voices, 8 langs, MOS #1 on TTS Arena. 550× RT on quantized CPU. |
| **Kyutai TTS 1.6B** | Apache code / CC-BY-4.0 weights | First-class Rust server ✅ | DSM-family alternative if MIL unifies STT+TTS on Kyutai |
| **Piper TTS** | MIT | `piper-rs` ✅ | Ultra-low-resource fallback (RPi etc.) |
| **AVSpeechSynthesizer** | Proprietary | Swift FFI ⚠️ | macOS default fallback |

**Decisive insight**: **`cartesia-one/csm.rs` is a production-quality Rust Candle implementation of Sesame CSM with Metal/CUDA backends.** That's drop-in for `sensorium-speech`. We don't write from scratch.

---

## 4. What the Landscape Tells Us

Six decisive insights distilled from the six waves.

### 4.1 The category is real and converging
DeepMind AI Pointer (May 2026), OpenAI Realtime-2 (May 2026), Apple Intelligence + App Intents (macOS 26 Tahoe), Granola's $1.5B valuation, Wispr Flow's $81M Series A, the polycast5 reel hitting 7K likes in a day: voice/multimodal input layer is **the** competitive surface in 2026. MIL's thesis is validated externally.

### 4.2 Nobody ships a typed substrate
The closest production typed substrates are **Apple App Intents** (typed entities + queries, but vertically integrated with Apple's runtime), **Cursor's `@-refs`** (typed in one app), and **Speechmatics Flow** (typed lifecycle at the WS protocol level only). No cross-vendor, cross-modality, OS-portable typed substrate exists. **That slot is open.**

### 4.3 The cloud-blob vs composed-pipeline duopoly leaves on-device unattended
Both architectures assume cloud LLM. Open weights for every layer exist (Parakeet STT, Llama/Qwen LLM, CSM/Moshi/Kokoro TTS, Moondream/Florence VLMs). **No credible turnkey on-device voice agent stack ships.** MIL with Parakeet + on-device Llama 3.x + CSM 1B + Moondream 3 *is* that stack.

### 4.4 Apple's on-device move is a competitive forcing function
macOS 26 Tahoe ships SpeechAnalyzer (2× faster than Whisper-large-v3-turbo, fully on-device) + Apple Intelligence + Foundation Models framework (3B on-device LLM, `@Generable` typed Swift API) + App Intents (typed deictic substrate). For Mac users this is *better* than anything cloud-blob vendors can ship, and Apple owns the deepest integration into the OS. **MIL on macOS must plumb to Apple's native primitives via Swift FFI**, not compete with them.

### 4.5 Vertical integration is the only durable hardware play
Humane DEAD. Friend $348K total revenue on cloud-tethered companion pitch. Bee acquired by Amazon. Limitless acquired by Meta. Rewind acquired by Meta (Reality Labs). HoloLens 2 production ended. Standalone consumer voice hardware as a business **does not sustain**. The successful pattern: deeply integrate with an OS (Apple+macOS), an editor (Cursor+code), or a domain (Granola+meetings). MIL's hardware play (if/when) is OSS firmware on commodity microcontrollers (ESP32-S3) + BLE HID into any MIL-equipped host — i.e., **don't ship hardware; ship firmware**.

### 4.6 Provenance is structurally absent from the entire market
Every wearable's "second brain" is one acquisition away from bricking. Every dictation tool's output is opaque text indistinguishable from hallucination. Every voice agent platform's tool calls are JSON args with no chain of custody. **Nobody signs anything.** Limitless gestured at "Consent Mode" (voice ID gates unknown speakers), but didn't sign transcripts. This is a structural moat MIL can build that no incumbent can copy without re-architecting from scratch — closer to **Yubico-style cryptographic identity** than to any voice product on the market.

---

## 5. MIL Positioning Thesis

```
MIL is the typed dual of every camp-A pixel-grounded product on the market,
the cross-vendor portable of every camp-B vertically-integrated typed substrate,
and the on-device sovereign for everything cloud-blob vendors lock to their GPUs.
```

**The pitch in one sentence**: *MIL is the open, typed, on-device, cryptographically-provenanced substrate that turns any input modality (voice, gaze, gesture, BCI, screen pointer) into a typed directive any executor can act on — composable, replayable, vendor-neutral.*

**The strategic frame**: same play as Spec E at the silicon layer, one level up. Own the contract; let backends compete underneath. Just as Spec E declares "the agent-loop compute contract" and lets Tenstorrent / SambaNova / Apple / NVIDIA compete on hardware, MIL declares **"the multimodal intent contract"** and lets Apple SpeechAnalyzer / Kyutai STT / OpenAI Realtime / Gemini Live / Sesame CSM / Moondream 3 compete as backends. Same shape, one substrate layer up the stack.

**Three things to publish openly**:
1. **The `Directive<S>` typestate** (already public via `pneuma-core` on crates.io)
2. **The `StreamUpdate<T> + Generation` substrate** (already public via `sensorium-core`)
3. **A Spec H — "Multimodal Intent Contract"** — the canonical document declaring the contract, modeled after Spec E's "Agent-Loop Compute Contract" framing. Published Apache-2.0. Targets: SDK authors (voice/gesture/BCI), OS vendors (Apple/MS/Linux desktop), framework vendors (LiveKit/Pipecat could ship MIL as their typed-output mode).

---

## 6. Roadmap — Seven Tracks

Each track is a Linear umbrella ticket with sub-tickets. Sequencing follows dependency order; parallel tracks marked.

### Track C — Streaming substrate completion ("C" for Composition)

**Goal**: complete the streaming substrate from voice → directive → execute end-to-end with barge-in and provenance event log. Closes the B-series.

| Ticket | Scope | Acceptance |
|---|---|---|
| **C1** | `pneuma-dialog` crate (was B5): extract inline VAD loop from pneuma-demo into reusable engine driver. Owns the stream-of-streams orchestration. Tests against Mock + Parakeet. | Loop is reusable across mic + WAV + future BLE-HID input. ≥15 unit tests. fmt + clippy clean. |
| **C2** | Barge-in wiring: `VoiceSession::cancel()` fires when user starts speaking during AI synthesis or when a keyboard interrupt arrives. Generation-tagged `Cancelled` propagates through the directive lifecycle. | Live mic test: user speaks → AI starts responding → user interrupts → AI synthesis cancels, mic re-engages, generation increments. |
| **C3** | Provenance event log: every `PrimitiveToken` + every `Directive` state transition emits a Lago event. Replayable. Sets stage for Track P signing. | Append-only NDJSON journal of all session events. Round-trip replay produces identical Final transcripts. |
| **C4** | Streaming agent kickoff: Arcan begins composing response on first parseable Partial; cancels cleanly on STT revise or barge-in. | Time-to-first-AI-token reduces by ≥200ms in measured live runs. |

**Critical path**: C1 → C2 → C3 → C4. ~4 weeks for one engineer.

### Track B — Voice backend optionality

**Goal**: prove the `SpeechToText` trait is a real contract by shipping multiple backends. Provides macOS-native, edge-tier, multilingual, full-duplex, and cloud-fallback options.

| Ticket | Scope | Acceptance |
|---|---|---|
| **B-Apple** | `sensorium-voice/apple-native` feature. Swift FFI via `SpeechAnalyzerDylib` (already published) + `swift-bridge`. macOS 26+ only. Becomes default on macOS 26 if available. | Parity quality vs Parakeet on test WAVs, 2× lower latency. fmt+clippy clean under `--features apple-native`. |
| **B-Kyutai** | `sensorium-voice/kyutai` feature. Wraps `kyutai-labs/delayed-streams-modeling` Rust `stt-rs` server in-process. Semantic VAD replaces EnergyVad when enabled. | Semantic-VAD utterance boundaries fire ≥150ms earlier than EnergyVad on test WAVs. CC-BY-4.0 weight license documented. |
| **B-Whisper** | `sensorium-voice/whisper-cpp` feature via `whisper-rs`. Multilingual reference baseline. | All 3-feature matrix green (default / parakeet / whisper-cpp), no conflicts. |
| **B-Moonshine** | `sensorium-voice/moonshine` feature for edge-tier (26-200MB). | Builds + runs on Raspberry Pi class device. Latency ≤Parakeet on x86. |
| **B-Parakeet-v3** | Drop-in upgrade to multilingual Parakeet TDT 0.6B v3. 25 EU langs. | English regression test passes; at least one non-English WAV (e.g. Spanish) produces correct transcript. |
| **B-Openai-Realtime** | `sensorium-voice/openai-realtime` feature. Cloud fallback. | Same `StreamUpdate<TranscriptDelta>` substrate; opt-in by env var. |
| **B-Gemini-Live** | `sensorium-voice/gemini-live` feature. Cloud fallback. | Same shape. |

**Parallel-safe**: all 7 backends can be developed independently after the trait is reaffirmed. Suggest **B-Apple first** (highest leverage on macOS for the user's own usage), **B-Kyutai second** (semantic VAD upgrade), rest as needed.

### Track T — TTS substrate

**Goal**: introduce `sensorium-speech` crate + `TextToSpeech` trait + Sesame CSM 1B backend. Closes the voice loop (input → directive → response → speech).

| Ticket | Scope | Acceptance |
|---|---|---|
| **T1** | `sensorium-speech` crate scaffold + `TextToSpeech` trait + `MockTts` (canned WAV) | Mirror of `sensorium-voice` shape. Tests + mock work. |
| **T2** | Sesame CSM 1B backend via `cartesia-one/csm.rs`. `feature = "csm"`. First-run weight bootstrap from HuggingFace into `~/.cache/sensorium-speech/csm-1b/`. | Synthesizes "rename it to alpha" with audible Sesame-quality voice. ≤300ms first-audio latency on M3. |
| **T3** | Kokoro-82M edge backend. `feature = "kokoro"`. 82M params, sub-second on CPU. | Works on Linux without GPU. ≤500MB RAM. |
| **T4** | Audio output substrate: `AudioPlayback` (cpal output stream, twin of `AudioCapture`). Feeds from a ringbuf the synthesis thread writes into. | Generated TTS audio plays through default output device. |

**Critical path**: T1 → T2 → T4 (T3 parallel-safe with T2). ~3 weeks.

### Track V — Vision substrate (the AI Pointer response)

**Goal**: extend MIL substrate from workspace-metadata-only (focused app/window/file) to *pixel-level* deictic resolution. Closes the screen-vision modality gap the GUI-agent research line is racing in.

| Ticket | Scope | Acceptance |
|---|---|---|
| **V1** | `sensorium-vision` crate scaffold + `ScreenCapture` trait + macOS `ScreenCaptureKit` backend. Captures focused-window pixels at user-paced cadence. | A screenshot of the focused window is captured and returned as a typed value with timestamp + window ID. |
| **V2** | Moondream 3 backend for screen grounding. `feature = "moondream"`. Wraps via MLX or candle. Emits typed bounding-box answers to "where is the X" queries. | Answer "where is the file menu" → `BoundingBox { x, y, w, h }` ≤500ms on M3 with screenshot input. |
| **V3** | Florence-2 backend for structured grounding. Fixed-prompt phrase grounding + OCR + caption. | OCR test: caption every text box on a screenshot of a Notion doc. |
| **V4** | **MIL Vision Adapter** — generic VLM-as-backend trait. Wraps Claude Computer Use / OpenAI CUA / Fara-7B / UGround behind one interface. | Same `BoundingBox` answer from any backend on the same screenshot+query input. |
| **V5** | Bridge into `pneuma-resolver`: deictic queries like "the highlighted row" resolve via `sensorium-vision` when workspace-metadata can't disambiguate. | "Delete that paragraph" with no AX selection resolves correctly against screen pixels in test WAV. |

**Parallel-safe**: V2/V3/V4 can be developed independently after V1. V5 depends on at least one of V2/V3/V4. ~6 weeks total.

### Track N — Non-voice modalities

**Goal**: extend MIL's substrate beyond voice. `Modulation`, `Attention`, `Reference`, `Approval` `PrimitiveToken`s get real producers.

| Ticket | Scope | Acceptance |
|---|---|---|
| **N1** | `sensorium-hands` crate. Mediapipe Hand Landmarker via TFLite FFI from Rust. Emits 21 3D landmarks (`Modulation`) + 7-class gestures (`Approval`). Webcam-only, free, cross-platform. | "Thumb up" gesture observed on webcam → `PrimitiveToken::Approval(Commit)` emitted. |
| **N2** | `sensorium-gaze` crate (desktop). Tobii Eye Tracker 5L via Tobii Pro SDK (research, free) or Streams SDK (commercial). Emits 120 Hz gaze events as `Attention` + `Reference`. | Looking at file `/tmp/old.txt` icon for 200ms emits `Reference(File("/tmp/old.txt"))` if hit-test resolves. |
| **N3** | `sensorium-arkit` (macOS/Vision Pro). ARKit `HandTrackingProvider` via Swift FFI. Hand skeletal joints as `Modulation`. | Pinch gesture in Vision Pro emits `Approval(Commit)`. |
| **N4** | `sensorium-emg` (deferred to 2027+ once Meta opens Neural Band SDK outside Quest, or community port matures). | n/a — placeholder ticket. |

**Critical path**: N1 (free, no HW) is the visible MVP. N2 + N3 require HW investment. ~4 weeks per ticket.

### Track P — Provenance / cryptographic trust

**Goal**: make MIL the only system on the market where every `PrimitiveToken` is cryptographically signed and replayable. Unblock healthcare/legal/journalism segments structurally banned from cloud wearables.

| Ticket | Scope | Acceptance |
|---|---|---|
| **P1** | Ed25519 signing of every `PrimitiveToken` at the producer. Keys held by the user (rotatable, exportable). Signature attached as `Tagged<T>` provenance field. | A `Predication("rename it to alpha")` carries a verifiable signature over (audio_hash + transcript + ts + sensor_id). |
| **P2** | Audit-trail viewer UX. Renders the session as a signed event log; lets user prove "I said X at time T" to a third party. | Sample session exported as signed JSON-LD, verifiable with a standalone CLI. |
| **P3** | Replayable session bundle (audio + transcript + signature). Single file, single signature, future-proofed. | Bundle from session A can be replayed on machine B and produces identical transcripts + verified signatures. |

**Critical path**: P1 → P2 → P3. Sets up the "Yubico for AI input" positioning. ~3 weeks.

### Track G — Product surfaces on top of MIL

**Goal**: ship visible products that consume the substrate, so the substrate has a clear adoption story.

| Ticket | Scope | Acceptance |
|---|---|---|
| **G1** | Live-realtime demo polish. Record a 2-minute screencast of `pneuma-demo` doing real voice → directive → execute (rename, switch-app, navigate, agent.explain) with visible streaming partials + speculative parse markers. Publish on broomva.tech and YouTube. | Video exists, embedded in MIL README. |
| **G2** | **MIL Pointer SDK** (the AI Pointer answer). Apps register `MilPointable<T>` types; a thin agent maps pointer+speech to a typed target. Hybrid: typed substrate for unambiguous 70%, vision (V2/V3/V4) for hard 30%. | A demo app (text editor) registers `MilPointable<Paragraph>`; user points + says "move *this* to top" → directive resolved + executed. |
| **G3** | **MIL Voice Keyboard Firmware** (ESP32-S3 BLE HID). On-device STT (quantized speech model, e.g. Moonshine tiny) → typed intent classifier → BLE HID injection of structured directive into focused app on host. Open-source. | One ESP32-S3 dev board flashed; speak "rename it to bar" through built-in mic; on a Mac with MIL-aware demo, BLE event arrives + directive composes. |
| **G4** | **MIL App Intents Bridge** (macOS). Auto-generate Apple App Intents donations from MIL's typed entity schema. Ships into Siri/Spotlight/Visual Intelligence for free. | Spotlight surfaces MIL-typed entities; "Hey Siri, rename Old.txt" works through MIL's typed contract. |

**Sequencing**: G1 first (low cost, high signal). G2 after V1+V2. G3 after C1 (needs orchestration crate). G4 after Track P (typed schema needs to be stable + signed).

---

## 7. Linear Project Structure

**New project**: "MIL — Multimodal Intent Language" under Broomva team. (Distinct from existing "Sensorium — Perception Substrate" which is the Life-OS `life-sensorium` Pneuma fabric.)

**Initiative**: standalone for now; possibly tie to Life Agent OS later if MIL becomes Pneuma's `ExternalToL0` realization at the desktop tier.

**Umbrella ticket**: "Spec H — Multimodal Intent Contract & 2026-Q3 Roadmap". References this spec doc. Spawns the per-track sub-tickets.

**Sub-ticket count**: ~22 actionable sub-tickets across 7 tracks:

| Track | Sub-tickets |
|---|---|
| C | 4 (C1 dialog crate, C2 barge-in, C3 provenance event log, C4 streaming agent kickoff) |
| B | 7 (Apple, Kyutai, Whisper, Moonshine, Parakeet-v3, OpenAI-Realtime, Gemini-Live) |
| T | 4 (T1 scaffold, T2 CSM, T3 Kokoro, T4 audio output) |
| V | 5 (V1 capture, V2 Moondream, V3 Florence, V4 adapter, V5 resolver bridge) |
| N | 3 active (N1 Mediapipe, N2 Tobii, N3 ARKit; N4 placeholder for 2027+) |
| P | 3 (Ed25519 signing, audit-trail viewer, replay bundle) |
| G | 4 (G1 video demo, G2 Pointer SDK, G3 keyboard firmware, G4 App Intents bridge) |

**Suggested priority for next 90 days** (Q3 2026):
- **Urgent**: C1 (dialog crate), C2 (barge-in), B-Apple (macOS-native default), T1+T2 (CSM TTS)
- **High**: C3 (provenance event log), G1 (video demo), V1 (screen capture), N1 (Mediapipe MVP)
- **Medium**: B-Kyutai, T4 (audio out), V2 (Moondream)
- **Lower / next quarter**: V3, V4, V5, N2, N3, P1-3, G2-G4, remaining backends

---

## 8. Risk Register

| Risk | Mitigation |
|---|---|
| **Apple deprecates `SpeechAnalyzerDylib` C-FFI shim** | Maintain Parakeet/Whisper fallback as default; Apple-native is opt-in. |
| **Moondream 3 free-form quality regresses** | Florence-2 keeps fixed-task grounding; both backends supported. |
| **Meta Neural Band SDK stays Quest-locked through 2027** | `sensorium-emg` is explicitly 2027+, deferred. |
| **DeepMind AI Pointer eats the consumer market before MIL has a comparable demo** | Ship G1 video demo Q3; positioning is substrate not product, different category. |
| **OpenAI Realtime price drops eat on-device latency advantage** | On-device privacy + provenance is the structural moat, not latency. |
| **License surface area grows (CC-BY-4.0, Apache, MIT, Apple Sample Code)** | Track P (provenance) doubles as license-attribution chain. Document per-backend. |
| **Spec H is too ambitious; we lose focus** | Sub-tickets sequenced; only 4 urgent in Q3. Each track ships independently. |

---

## 9. Sources (consolidated)

Compiled from six research waves; full per-axis bibliography lives in the agent transcripts. Top-level URLs:

**HW peripherals**: [Plaud.ai](https://www.plaud.ai/), [Humane shutdown](https://techcrunch.com/2025/02/18/humanes-ai-pin-is-dead-as-hp-buys-startups-assets-for-116m/), [Pebble Index 01](https://repebble.com/index), [Omi](https://www.omi.me/), [RoboticWorx polycast5 reel](https://www.instagram.com/reel/DYPgBFOx7Go/), [Sandbar](https://techcrunch.com/2026/03/10/sandbar-secures-23m-series-a-for-its-ai-note-taking-ring/), [Bee→Amazon](https://www.geekwire.com/2025/amazon-is-acquiring-bee-maker-of-a-wearable-ai-assistant-that-listens-to-conversations/)

**Desktop voice typing**: [Wispr Flow](https://wisprflow.ai/pricing), [Superwhisper](https://superwhisper.com/), [MacWhisper](https://goodsnooze.gumroad.com/l/macwhisper), [Aqua Voice](https://aquavoice.com/), [Willow](https://willowvoice.com/), [Talon](https://talonvoice.com/), [Apple Voice Control](https://support.apple.com/guide/mac-help/use-voice-control-commands-mh40719/mac), [OpenWhispr](https://openwhispr.com/), [Dictato](https://dicta.to/blog/whisper-vs-parakeet-vs-apple-speech-engine/)

**Pointer / screen-context**: [DeepMind AI Pointer](https://deepmind.google/blog/ai-pointer/), [Microsoft Recall](https://support.microsoft.com/en-us/windows/retrace-your-steps-with-recall-aa03f8a0-a78b-4b3e-b0a1-2eb8ac48701c), [App Intents WWDC25](https://developer.apple.com/videos/play/wwdc2025/244/), [Claude Computer Use](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool), [OpenAI Operator](https://openai.com/index/introducing-operator/), [Cursor IDE 2.0](https://github.com/slava-kudzinau/cursor-guide), [Granola $1.5B](https://techcrunch.com/2026/03/25/granola-raises-125m-hits-1-5b-valuation-as-it-expands-from-meeting-notetaker-to-enterprise-ai-app/), [Limitless→Meta](https://www.hedy.ai/post/meta-acquires-limitless-ai-privacy/), [Screenpipe](https://screenpi.pe/), [ScreenSpot-Pro](https://arxiv.org/abs/2504.07981), [Fara-7B](https://www.microsoft.com/en-us/research/wp-content/uploads/2025/11/Fara-7B-An-Efficient-Agentic-Model-for-Computer-Use.pdf)

**Realtime voice platforms**: [OpenAI Realtime-2](https://openai.com/index/advancing-voice-intelligence-with-new-models-in-the-api/), [Gemini Live](https://ai.google.dev/gemini-api/docs/live-api), [Sesame CSM](https://www.sesame.com/research/crossing_the_uncanny_valley_of_voice), [Hume EVI](https://www.hume.ai/empathic-voice-interface), [Retell](https://www.retellai.com/), [Vapi](https://vapi.ai/), [LiveKit Agents](https://github.com/livekit/agents), [Pipecat](https://www.pipecat.ai/), [Cartesia](https://cartesia.ai/), [Deepgram](https://deepgram.com/), [Speechmatics Flow](https://docs.speechmatics.com/voice-agents/flow), [Kyutai Moshi](https://github.com/kyutai-labs/moshi), [Ultravox](https://www.ultravox.ai/), [ElevenLabs Agents](https://elevenlabs.io/agents), [Twilio ConversationRelay](https://www.twilio.com/en-us/products/conversational-ai/conversationrelay), [Parakeet v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3)

**Non-voice modalities**: [Tobii Pro SDK](https://developer.tobiipro.com/index.html), [Apple VisionOS](https://developer.apple.com/visionos/), [Mediapipe Hand Landmarker](https://ai.google.dev/edge/mediapipe/solutions/vision/hand_landmarker), [Mediapipe Gesture Recognizer](https://ai.google.dev/edge/mediapipe/solutions/vision/gesture_recognizer), [Ultraleap Hyperion](https://docs.ultraleap.com/hand-tracking/Hyperion/index.html), [Meta Neural Band](https://www.uploadvr.com/meta-semg-wristband-gestures-nature-paper/), [AlterEgo MIT](https://www.media.mit.edu/projects/alterego/overview/), [Cognixion Axon-R](https://axon-r.cognixion.com/), [Neurable licensing pivot](https://techcrunch.com/2026/04/28/bci-startup-neurable-looks-to-license-its-mind-reading-tech-for-consumer-wearables/), [Naqi](https://www.podfeet.com/blog/2026/02/ces-2026-naqi-logix/)

**On-device speech/vision/TTS**: [parakeet-rs](https://github.com/altunenes/parakeet-rs), [whisper-rs](https://github.com/tazz4843/whisper-rs), [Moonshine](https://github.com/moonshine-ai/moonshine), [Kyutai DSM repo](https://github.com/kyutai-labs/delayed-streams-modeling), [Apple SpeechAnalyzer](https://developer.apple.com/documentation/speech/speechanalyzer), [WWDC25 Session 277](https://developer.apple.com/videos/play/wwdc2025/277/), [SpeechAnalyzerDylib (HN)](https://news.ycombinator.com/item?id=44431186), [Apple Foundation Models](https://developer.apple.com/documentation/FoundationModels), [Moondream 3](https://moondream.ai/blog/moondream-station-m3-preview), [Apple FastVLM](https://github.com/apple/ml-fastvlm), [Florence-2 on Roboflow](https://blog.roboflow.com/florence-2/), [Sesame csm.rs Candle impl](https://github.com/cartesia-one/csm.rs), [Kokoro-82M HF](https://huggingface.co/hexgrad/Kokoro-82M), [piper-rs](https://github.com/thewh1teagle/piper-rs), [Sherpa-ONNX](https://github.com/k2-fsa/sherpa-onnx), [swift-bridge](https://github.com/chinedufn/swift-bridge), [candle](https://github.com/huggingface/candle)
