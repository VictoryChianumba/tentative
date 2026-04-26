pub mod categories;
pub mod item;
pub use categories::*;
pub use item::*;

/// Extract a bare arXiv ID from known URL patterns, or return `None`.
///
/// Handles `arxiv.org/abs/`, `arxiv.org/pdf/`, and `huggingface.co/papers/`.
pub fn arxiv_id_from_url(url: &str) -> Option<String> {
  for prefix in &["arxiv.org/abs/", "arxiv.org/pdf/", "huggingface.co/papers/"]
  {
    if let Some(pos) = url.find(prefix) {
      let rest = &url[pos + prefix.len()..];
      let id: String = rest
        .chars()
        .take_while(|&c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        .collect();
      if !id.is_empty() {
        return Some(id);
      }
    }
  }
  None
}
