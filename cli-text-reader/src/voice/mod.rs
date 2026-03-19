pub mod elevenlabs;
pub mod playback;
pub mod stream_buffer;

pub use playback::{PlaybackCommand, PlaybackController, PlaybackStatus};

use std::time::Instant;

/// Snapshot of what the background thread is currently playing.
/// Updated each time a new chunk starts, so the editor can track word position.
pub struct VoicePlayingInfo {
  pub doc_start_line: usize,
  pub doc_end_line: usize,
  /// Wall-clock instant when the current chunk began playing.
  pub started_at: Instant,
  /// Total chars spoken in all *completed* chunks before this one.
  pub chars_before_chunk: usize,
}

/// Split `text` into chunks no larger than `max_chars`, respecting blank-line
/// paragraph boundaries.  Adjacent short paragraphs are merged.
pub fn chunk_paragraphs(text: &str) -> Vec<String> {
  const MAX: usize = 4500;
  let mut chunks: Vec<String> = Vec::new();
  let mut current = String::new();

  for para in text.split("\n\n") {
    let para = para.trim();
    if para.is_empty() {
      continue;
    }
    if current.is_empty() {
      current.push_str(para);
    } else if current.len() + 2 + para.len() <= MAX {
      current.push_str("\n\n");
      current.push_str(para);
    } else {
      chunks.push(std::mem::take(&mut current));
      current.push_str(para);
    }
  }
  if !current.is_empty() {
    chunks.push(current);
  }

  // Fallback: text with no blank lines — split at MAX chars on whitespace
  if chunks.is_empty() {
    let text = text.trim();
    let mut start = 0;
    while start < text.len() {
      let end = (start + MAX).min(text.len());
      // Try to break at a whitespace boundary
      let end = if end < text.len() {
        text[start..end]
          .rfind(char::is_whitespace)
          .map(|i| start + i + 1)
          .unwrap_or(end)
      } else {
        end
      };
      chunks.push(text[start..end].to_string());
      start = end;
    }
  }

  chunks
}
