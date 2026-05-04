//! Filename-safety validation for note IDs.
//!
//! Notes use *nanosecond timestamps* as IDs (see `app/mod.rs::new_id`),
//! not UUIDs. The validation pattern accepts both shapes: any
//! alphanumeric-with-hyphens-and-underscores string up to 64 characters,
//! rejecting empties, leading dots, and `..`.
//!
//! Without this gate, an imported `*.json` whose `note_id` field carried
//! `"../../etc/foo"` would resolve via `notes_dir().join(format!("{id}.json"))`
//! to a path outside the notes directory — Sec MED #20 from the code
//! review.

/// True if `s` is safe to use as a filename component.
pub(crate) fn is_safe_id(s: &str) -> bool {
  if s.is_empty() || s.len() > 64 {
    return false;
  }
  if s == "." || s == ".." || s.starts_with('.') {
    return false;
  }
  s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
  use super::is_safe_id;

  #[test]
  fn accepts_nanosecond_timestamp() {
    // Notes' actual ID format — pure digits.
    assert!(is_safe_id("1714824000000000000"));
  }

  #[test]
  fn accepts_uuid() {
    assert!(is_safe_id("550e8400-e29b-41d4-a716-446655440000"));
  }

  #[test]
  fn accepts_alphanumeric_underscore_hyphen() {
    assert!(is_safe_id("note_42-test"));
    assert!(is_safe_id("ABC123"));
  }

  #[test]
  fn rejects_traversal() {
    assert!(!is_safe_id(".."));
    assert!(!is_safe_id("../etc"));
    assert!(!is_safe_id("foo/bar"));
    assert!(!is_safe_id("foo\\bar"));
  }

  #[test]
  fn rejects_leading_dot() {
    assert!(!is_safe_id("."));
    assert!(!is_safe_id(".hidden"));
  }

  #[test]
  fn rejects_empty_and_oversized() {
    assert!(!is_safe_id(""));
    assert!(!is_safe_id(&"a".repeat(65)));
  }

  #[test]
  fn rejects_special_chars() {
    assert!(!is_safe_id("foo bar"));
    assert!(!is_safe_id("foo:bar"));
    assert!(!is_safe_id("foo\nbar"));
    assert!(!is_safe_id("foo$bar"));
  }
}
