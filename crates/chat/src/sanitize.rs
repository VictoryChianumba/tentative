//! Terminal-safe text sanitization for streamed chat content.
//!
//! Mirrors `trench/src/sanitize.rs` exactly. We keep a local copy here rather
//! than introducing a cross-crate dependency on trench internals — the helper
//! is a few dozen lines and changes rarely.
//!
//! Streamed assistant responses (Claude / OpenAI / Perplexity) bypass the
//! `FeedItem` ingestion path, so the chat render must sanitize independently.
//! User-typed messages are also sanitized — pasting hostile text into the
//! prompt could otherwise hijack the user's own terminal on render.

/// Strip every ANSI escape sequence, control byte, and DEL from `s`.
/// See `trench::sanitize::sanitize_terminal_text` for the full specification.
pub(crate) fn sanitize_terminal_text(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let mut chars = s.chars().peekable();
  while let Some(c) = chars.next() {
    match c {
      '\t' | '\n' | '\r' => out.push(c),
      '\x1b' => {
        let Some(&next) = chars.peek() else {
          continue;
        };
        match next {
          '[' => {
            chars.next();
            for ch in chars.by_ref() {
              let b = ch as u32;
              if (0x40..=0x7E).contains(&b) {
                break;
              }
            }
          }
          ']' => {
            chars.next();
            let mut prev = '\0';
            for ch in chars.by_ref() {
              if ch == '\x07' {
                break;
              }
              if prev == '\x1b' && ch == '\\' {
                break;
              }
              prev = ch;
            }
          }
          'P' | 'X' | '^' | '_' => {
            chars.next();
            let mut prev = '\0';
            for ch in chars.by_ref() {
              if prev == '\x1b' && ch == '\\' {
                break;
              }
              prev = ch;
            }
          }
          _ => {
            chars.next();
          }
        }
      }
      c if (c as u32) < 0x20 => out.push(' '),
      '\x7f' => {}
      _ => out.push(c),
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::sanitize_terminal_text;

  #[test]
  fn strips_csi_and_osc_in_assistant_response() {
    let s = "Sure, here's the answer:\x1b[2J\x1b]52;c;evil\x07 done";
    assert_eq!(sanitize_terminal_text(s), "Sure, here's the answer: done");
  }

  #[test]
  fn preserves_markdown_and_unicode() {
    let s = "## Heading\n- item with `code` and *emphasis*\n— em-dash 🚀";
    assert_eq!(sanitize_terminal_text(s), s);
  }

  #[test]
  fn idempotent() {
    let s = "before\x1b[31mred\x1b[0mafter";
    let once = sanitize_terminal_text(s);
    let twice = sanitize_terminal_text(&once);
    assert_eq!(once, twice);
  }
}
