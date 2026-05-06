# sensorium-voice — voice input substrate

Step #17 of `MIL-PROJECT.md` §11.2. The final piece of MIL Tier 3.

Streams microphone audio through Silero V5 voice activity detection
and NVIDIA Parakeet TDT (EOU streaming variant) on-device on Apple
Silicon, emitting `PrimitiveToken { kind: Predication }` tokens with
full provenance.

After this crate ships and gets wired into the demo, the user types
nothing — they speak, and MIL parses the resulting transcript into
the directive contract. That's the moment "talk to your computer"
becomes literal.

## Architecture

```text
microphone (cpal, CoreAudio on macOS)
   │
   ▼  16kHz f32 mono
ringbuf (lock-free SPSC, ~3 seconds buffered)
   │
   ▼  audio chunks
VAD gate (Silero V5 via voice_activity_detector)
   │
   ▼  speech-only audio (gated when VAD says "silence")
Parakeet EOU (parakeet-rs, ort + WebGPU/CPU)
   │
   ▼  streaming partials at 160ms cadence
PrimitiveToken { kind: Predication, payload: text, ... }
   │
   ▼  mpsc::channel
Consumer (pneuma-demo, future pneuma-binder)
```

## Why Parakeet instead of Whisper

Per `research/entities/project/superwhisper-voice-ecosystem.md` and a
follow-up Parakeet-vs-Whisper-on-Apple-Silicon evaluation:

- **Whisper.cpp / whisper-rs** floors at multi-second latency for
  large-v3-turbo on M3/M4 — fails MIL's `<500ms post-utterance`
  target on day one.
- **Parakeet TDT EOU** streams at 160ms chunks, runs faster on M3 CPU
  than Whisper on M3 Metal (per `parakeet-rs` author's benchmark),
  and the TDT decoder is natively streaming-friendly (token-and-
  duration prediction in one go).
- The Rust integration path via `parakeet-rs` 0.3.x + `ort` is
  production-ready in 2026.

A future `whisper` backend feature flag will add multilingual fallback
for the languages Parakeet v2 (English) and v3 (25 European) don't
cover — Mandarin, Japanese, Arabic, Korean.

## Status

v0.2.0 — Parakeet EOU streaming via `parakeet-rs`, Silero V5 VAD,
cpal mic capture. First-run weight bootstrap via `hf-hub`.
Single-backend (no fallback) — multilingual via whisper-rs is a v0.3
feature.
