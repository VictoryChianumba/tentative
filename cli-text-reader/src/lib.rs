mod bookmarks;
mod config;
mod core_state;
mod core_types;
mod debug;
pub mod demo_components;
mod demo_content;
pub mod demo_registry;
pub mod demo_script;
mod demo_tutorial_test;
mod editor;
mod help;
mod highlights;
mod highlights_core;
mod highlights_persistence;
mod interactive_tutorial;
mod interactive_tutorial_buffer;
mod interactive_tutorial_steps;
mod interactive_tutorial_tests;
mod interactive_tutorial_utils;
mod progress;
mod tutorial;
mod utils;
pub mod voice;

pub use editor::Editor;

// Embeddable ratatui API
pub use editor::{EditorAction, draw as draw_editor, run_ratatui};

/// Install a panic hook that restores the terminal (disable raw mode, leave
/// alternate screen, disable mouse capture, show cursor) before chaining to
/// the previous default hook. Idempotent — calling repeatedly just replaces
/// the existing hook with one that has the same semantics.
///
/// Both `trench` and standalone `hygg-reader` should call this near the top
/// of `main()`, **before** `enable_raw_mode()`. Without it, a panic anywhere
/// in the editor (including a panic propagating from a spawned thread) leaves
/// the terminal in raw / alt-screen mode and the user has to blindly type
/// `reset` to recover.
pub fn install_terminal_panic_hook() {
  let default_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
    // Best-effort terminal restore — every step ignores errors so a partial
    // failure (e.g. stderr already closed) doesn't prevent the rest from
    // running and doesn't trigger a double-panic.
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
      std::io::stderr(),
      crossterm::terminal::LeaveAlternateScreen,
      crossterm::event::DisableMouseCapture,
      crossterm::cursor::Show,
    );
    // Blank line before the panic message so it doesn't collide with any
    // partial frame the alt-screen leave just flushed.
    eprintln!();
    default_hook(info);
  }));
}

pub fn run_cli_text_reader(
  lines: Vec<String>,
  col: usize,
) -> Result<(), Box<dyn std::error::Error>> {
  run_cli_text_reader_with_demo(lines, col, false)
}

pub fn run_cli_text_reader_with_demo(
  lines: Vec<String>,
  col: usize,
  demo_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  run_cli_text_reader_with_content(lines, col, None, demo_mode)
}

pub fn run_cli_text_reader_with_content(
  lines: Vec<String>,
  col: usize,
  raw_content: Option<String>,
  demo_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  // Initialize debug logging
  debug::init_debug_logging()?;
  debug::debug_log("main", "Starting cli-text-reader");
  debug::debug_log_state("main", "lines_count", &lines.len().to_string());
  debug::debug_log_state("main", "col", &col.to_string());
  debug::debug_log_state("main", "demo_mode", &demo_mode.to_string());
  if raw_content.is_some() {
    debug::debug_log("main", "Raw content provided for consistent hashing");
  }

  let mut editor = if let Some(content) = raw_content {
    Editor::new_with_content(lines, col, content)
  } else {
    Editor::new(lines, col)
  };
  editor.tutorial_demo_mode = demo_mode;
  let result = editor.run();

  debug::debug_log("main", "Editor run completed");
  debug::flush_debug_log();
  result
}

pub fn run_cli_text_reader_with_demo_id(
  lines: Vec<String>,
  col: usize,
  demo_id: usize,
) -> Result<(), Box<dyn std::error::Error>> {
  // Initialize debug logging
  debug::init_debug_logging()?;
  debug::debug_log("main", "Starting cli-text-reader with demo");
  debug::debug_log_state("main", "demo_id", &demo_id.to_string());
  debug::debug_log_state("main", "col", &col.to_string());

  let mut editor = Editor::new(lines, col);
  editor.tutorial_demo_mode = true;
  editor.demo_id = Some(demo_id);
  let result = editor.run();

  debug::debug_log("main", "Editor run completed");
  debug::flush_debug_log();
  result
}
