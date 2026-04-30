use doc_model::{Block, VisualLine, VisualLineKind, build_visual_lines};

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
  Normal,
  Search,
}

pub struct Reader {
  pub blocks: Vec<Block>,
  pub visual_lines: Vec<VisualLine>,
  pub sections: Vec<(usize, u8, String)>, // (line_idx, level, title)
  pub toc_visible: bool,
  pub offset: usize,
  pub cursor_y: usize,
  pub width: usize,
  pub height: usize,
  pub search_query: String,
  pub search_matches: Vec<usize>,
  pub search_idx: usize,
  pub mode: Mode,
}

impl Reader {
  pub fn new(blocks: Vec<Block>, width: usize, height: usize) -> Self {
    let visual_lines = build_visual_lines(&blocks, width);
    let sections = build_sections(&visual_lines);
    Self {
      blocks,
      visual_lines,
      sections,
      toc_visible: false,
      offset: 0,
      cursor_y: 0,
      width,
      height,
      search_query: String::new(),
      search_matches: Vec::new(),
      search_idx: 0,
      mode: Mode::Normal,
    }
  }

  pub fn resize(&mut self, width: usize, height: usize) {
    self.width = width;
    self.height = height;
    self.visual_lines = build_visual_lines(&self.blocks, width);
    self.sections = build_sections(&self.visual_lines);
    // Clamp position to new bounds.
    let max_offset = self.visual_lines.len().saturating_sub(1);
    self.offset = self.offset.min(max_offset);
    let content_height = height.saturating_sub(2); // status + search lines
    self.cursor_y = self.cursor_y.min(content_height.saturating_sub(1));
  }

  pub fn toggle_toc(&mut self) {
    self.toc_visible = !self.toc_visible;
  }

  /// Index into `sections` of the last section header at or above the current line.
  pub fn current_section_idx(&self) -> Option<usize> {
    let cur = self.current_line();
    self.sections.iter().rposition(|s| s.0 <= cur)
  }

  pub fn current_line(&self) -> usize {
    self.offset + self.cursor_y
  }

  pub fn total_lines(&self) -> usize {
    self.visual_lines.len()
  }

  pub fn content_height(&self) -> usize {
    // Reserve 1 row for status line; 1 more when in search mode.
    match self.mode {
      Mode::Normal => self.height.saturating_sub(1),
      Mode::Search => self.height.saturating_sub(2),
    }
  }

  pub fn update_search_matches(&mut self) {
    let q = self.search_query.to_lowercase();
    self.search_matches = if q.is_empty() {
      Vec::new()
    } else {
      self.visual_lines
        .iter()
        .enumerate()
        .filter(|(_, vl)| vl.text.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
    };
    self.search_idx = 0;
  }

  pub fn jump_to_match(&mut self, idx: usize) {
    if self.search_matches.is_empty() {
      return;
    }
    let line = self.search_matches[idx];
    self.offset = line;
    self.cursor_y = 0;
  }
}

fn build_sections(visual_lines: &[VisualLine]) -> Vec<(usize, u8, String)> {
  visual_lines
    .iter()
    .enumerate()
    .filter_map(|(i, vl)| {
      if let VisualLineKind::Header(level) = &vl.kind {
        Some((i, *level, vl.text.clone()))
      } else {
        None
      }
    })
    .collect()
}
