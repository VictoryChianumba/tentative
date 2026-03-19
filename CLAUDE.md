# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
# Build and run (primary development workflow)
cargo run --release -- test-data/pdf/pdfreference1.7old.pdf

# Build all crates
cargo build --release

# Run tests
cargo test

# Run tests for a specific crate
cargo test -p cli-text-reader

# Run a single test
cargo test -p cli-text-reader test_name

# Check formatting
cargo fmt --check

# Lint
cargo clippy
```

Rust edition: 2024, MSRV: 1.88.

## Workspace Structure

This is a Cargo workspace. The `hygg` binary crate (entry point) orchestrates the pipeline:

```
hygg/              → main binary: arg parsing, doc conversion → cli-text-reader
cli-text-reader/   → the TUI reader (all editor logic lives here)
cli-pdf-to-text/   → PDF → plain text conversion
cli-epub-to-text/  → EPUB → plain text conversion
cli-justify/       → text justification/wrapping
hygg-shared/       → shared utilities
redirect-stderr/   → stderr redirection helper
```

## cli-text-reader Architecture

This crate is the core. Everything is implemented as `impl Editor` blocks spread across many files. The `Editor` struct is defined in `src/core_state.rs` and re-exported via `src/editor/core.rs`.

**Main loop** (`src/editor/display_loop.rs`): polls voice status, handles crossterm events, triggers redraws. Uses `needs_redraw` flag — call `self.mark_dirty()` to request a redraw.

**Event routing** (`src/editor/event_handler.rs` → `src/editor/normal_mode.rs`): `handle_event` dispatches to mode-specific handlers. Normal mode calls handlers in priority order: tmux prefix → voice keys → control keys → operator pending → search/visual → navigation.

**Modes** (`src/core_types.rs`): `EditorMode` — Normal, VisualChar, VisualLine, Search, ReverseSearch, Command, CommandExecution, Tutorial. Mode is stored per-buffer in `BufferState`; use `get_active_mode()` / `set_active_mode()`.

**Voice/TTS** (`src/voice/`, `src/editor/voice_control.rs`):
- `PlaybackController` owns a background thread (`playback_loop`) that receives `PlaybackCommand` over an mpsc channel and drives rodio audio playback.
- Text is split into ≤4500-char chunks via `chunk_paragraphs()` in `src/voice/mod.rs`.
- `VoicePlayingInfo` (shared via `Arc<Mutex>`) tracks which doc lines are playing and timing for word-highlight animation.
- `sync_voice_status()` is called each tick in the display loop — this is the hook point for detecting playback completion.
- TTS uses ElevenLabs API. Config (`ELEVENLABS_API_KEY`, `VOICE_ID`, `PLAYBACK_SPEED`) lives in `~/.config/hygg/.env`.

**Config** (`src/config.rs`): loaded from `~/.config/hygg/.env` via `dotenvy`. Call `load_config()` at startup; `save_config()` persists changes.

**Persistence**: Progress saved per-document using a hash of the document content (`src/progress.rs`). Bookmarks and highlights also keyed by document hash (`src/bookmarks.rs`, `src/highlights.rs`).

**Buffers**: The editor supports multiple `BufferState` buffers (used for split-view command output). Buffer 0 is always the document. Active buffer accessed via `self.active_buffer` index.

**Display**: `draw_content_buffered` renders to a `Vec<u8>` then flushes in one write to minimize flicker. Status line rendered separately by `draw_status_line` / `draw_status_line_buffered`.

## Key Conventions

- All editor methods are `impl Editor` — no separate structs for subsystems.
- Handler functions return `Result<Option<bool>, ...>`: `Some(true)` = quit, `Some(false)` = handled (stop propagation), `None` = not handled (continue to next handler).
- `self.offset` = first visible line index; `self.cursor_y` = cursor row on screen; `self.offset + self.cursor_y` = current doc line.
- Clippy lints `needless_return` and `unused_imports` are allowed workspace-wide.
