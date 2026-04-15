# KikuCapturePlugin (macOS)

This module hosts macOS-native capture glue for Kiku.

- `SystemAudioCaptureHelper.swift` provides ScreenCaptureKit-based system playback capture.
- The helper streams `f32` PCM samples to Rust over stdout with a binary header.
- Permission handling is built in (`CGRequestScreenCaptureAccess`) so users are prompted from Kiku.
