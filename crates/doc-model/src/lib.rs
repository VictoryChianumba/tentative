/// Inline styled run within a paragraph line.
/// `color` uses raw RGB rather than a ratatui type — doc-model has no UI dependency.
#[derive(Debug, Clone)]
pub struct InlineSpan {
  pub text: String,
  pub bold: bool,
  pub italic: bool,
  pub underline: bool,
  pub strikethrough: bool,
  pub monospace: bool,
  pub color: Option<(u8, u8, u8)>,
  pub url: Option<String>,
}

impl InlineSpan {
  pub fn plain(text: impl Into<String>) -> Self {
    Self {
      text: text.into(),
      bold: false,
      italic: false,
      underline: false,
      strikethrough: false,
      monospace: false,
      color: None,
      url: None,
    }
  }

  pub fn bold(text: impl Into<String>) -> Self {
    Self { bold: true, ..Self::plain(text) }
  }

  pub fn italic(text: impl Into<String>) -> Self {
    Self { italic: true, ..Self::plain(text) }
  }

  pub fn monospace(text: impl Into<String>) -> Self {
    Self { monospace: true, ..Self::plain(text) }
  }
}

/// Semantic block — the producer's view of the document.
#[derive(Debug, Clone)]
pub enum Block {
  /// A single line of prose, already word-wrapped by the producer.
  Line(String),
  /// A display math equation rendered as multiple Unicode lines, treated as one unit.
  /// `num` carries the equation number for numbered environments (equation, align, etc.).
  DisplayMath { lines: Vec<String>, num: Option<usize> },
  /// A section header. level: 1=section, 2=subsection, 3=subsubsection/paragraph.
  Header { level: u8, text: String },
  /// A matrix rendered as a grid of cells (row-major).
  Matrix { rows: Vec<Vec<String>> },
  /// Explicit vertical space (blank line).
  Blank,
  /// A prose line carrying inline styling (bold, italic, monospace, etc.).
  /// The producer emits this when any span has a non-default style.
  /// build_visual_lines wraps it to terminal_width.
  StyledLine(Vec<InlineSpan>),
  /// A list item. depth=0 for top-level; marker is "• " or "1. " etc.
  ListItem { depth: u8, marker: String, content: Vec<InlineSpan> },
  /// A verbatim / code-listing block. Lines are raw (no LaTeX processing).
  CodeBlock { lang: Option<String>, lines: Vec<String> },
  /// A horizontal rule: \hline, \toprule, \midrule, \bottomrule.
  Rule,
  /// A block quote: \begin{quote}, \begin{quotation}, \begin{epigraph}.
  Quote(Vec<InlineSpan>),
}

/// A single screen row, fully expanded from a Block.
/// This is the flat table the reader indexes into — offset and cursor_y
/// are indices into Vec<VisualLine>, identical to how they used Vec<String>.
#[derive(Debug, Clone)]
pub struct VisualLine {
  pub block_idx: usize,
  pub line_in_block: usize,
  pub text: String,
  pub kind: VisualLineKind,
}

#[derive(Debug, Clone)]
pub enum VisualLineKind {
  Prose,
  /// Part of a display math block. text is pre-centered with leading spaces.
  MathLine { block_width: usize, is_first: bool, is_last: bool },
  Header(u8),
  MatrixLine { is_first: bool, is_last: bool },
  Blank,
  /// Prose with inline styling. text = plain concatenation (for search).
  /// Spans carry the styled runs for the renderer.
  StyledProse(Vec<InlineSpan>),
  /// A list item row. text already contains indent+marker prefix.
  ListItem { depth: u8, marker_len: u8, is_continuation: bool },
  /// A line from a code/verbatim block.
  Code { is_first: bool, is_last: bool },
  /// A horizontal rule; text = "─".repeat(terminal_width).
  Rule,
  /// A block quote; text = plain concatenation of spans.
  Quote { is_continuation: bool },
}

/// Expand a block list into the flat visual line table.
///
/// Called once at document load and again on terminal resize (only the
/// centering offset of MathLine entries changes on resize).
pub fn build_visual_lines(blocks: &[Block], terminal_width: usize) -> Vec<VisualLine> {
  let mut out = Vec::new();
  let mut i = 0;

  while i < blocks.len() {
    // Table groups: consecutive Rule and Matrix blocks are rendered together
    // so they can share column widths and proper box-drawing borders.
    if matches!(&blocks[i], Block::Rule | Block::Matrix { .. }) {
      let group_start = i;
      while i < blocks.len() && matches!(&blocks[i], Block::Rule | Block::Matrix { .. }) {
        i += 1;
      }
      let group = &blocks[group_start..i];
      let has_matrix = group.iter().any(|b| matches!(b, Block::Matrix { .. }));
      if has_matrix {
        render_table_group(group, group_start, &mut out);
      } else {
        for (k, _) in group.iter().enumerate() {
          out.push(VisualLine {
            block_idx: group_start + k,
            line_in_block: 0,
            text: "─".repeat(terminal_width),
            kind: VisualLineKind::Rule,
          });
        }
      }
      continue;
    }

    let block_idx = i;
    match &blocks[i] {
      Block::Line(s) => {
        out.push(VisualLine {
          block_idx,
          line_in_block: 0,
          text: s.clone(),
          kind: VisualLineKind::Prose,
        });
      }

      Block::Blank => {
        out.push(VisualLine {
          block_idx,
          line_in_block: 0,
          text: String::new(),
          kind: VisualLineKind::Blank,
        });
      }

      Block::Header { level, text } => {
        out.push(VisualLine {
          block_idx,
          line_in_block: 0,
          text: text.clone(),
          kind: VisualLineKind::Header(*level),
        });
      }

      Block::DisplayMath { lines, num } => {
        let block_width = lines.iter().map(|l| visual_width(l)).max().unwrap_or(0);
        let n = lines.len();
        for (li, line) in lines.iter().enumerate() {
          let mut centered = center_line(line, block_width, terminal_width);
          if li == n - 1 {
            if let Some(eq_num) = num {
              let tag = format!("({})", eq_num);
              let used = visual_width(&centered);
              let avail = terminal_width.saturating_sub(tag.len());
              if used < avail {
                centered.push_str(&" ".repeat(avail - used));
              }
              centered.push_str(&tag);
            }
          }
          out.push(VisualLine {
            block_idx,
            line_in_block: li,
            text: centered,
            kind: VisualLineKind::MathLine {
              block_width,
              is_first: li == 0,
              is_last: li == n - 1,
            },
          });
        }
      }

      Block::StyledLine(spans) => {
        let wrapped = wrap_spans(spans, terminal_width);
        for (li, (line_spans, plain)) in wrapped.into_iter().enumerate() {
          out.push(VisualLine {
            block_idx,
            line_in_block: li,
            text: plain,
            kind: VisualLineKind::StyledProse(line_spans),
          });
        }
      }

      Block::ListItem { depth, marker, content } => {
        let wrapped = wrap_list_item(*depth, marker, content, terminal_width);
        for (li, (_line_spans, plain, is_continuation)) in wrapped.into_iter().enumerate() {
          out.push(VisualLine {
            block_idx,
            line_in_block: li,
            text: plain,
            kind: VisualLineKind::ListItem {
              depth: *depth,
              marker_len: marker.len() as u8,
              is_continuation,
            },
          });
        }
      }

      Block::CodeBlock { lines, .. } => {
        let n = lines.len();
        for (i, line) in lines.iter().enumerate() {
          out.push(VisualLine {
            block_idx,
            line_in_block: i,
            text: line.clone(),
            kind: VisualLineKind::Code {
              is_first: i == 0,
              is_last: i == n - 1,
            },
          });
        }
      }

      Block::Quote(spans) => {
        let quote_width = terminal_width.saturating_sub(4).max(1);
        let wrapped = wrap_spans(spans, quote_width);
        for (li, (_line_spans, plain)) in wrapped.into_iter().enumerate() {
          out.push(VisualLine {
            block_idx,
            line_in_block: li,
            text: plain,
            kind: VisualLineKind::Quote { is_continuation: li > 0 },
          });
        }
      }

      // Rule and Matrix are handled above via the table-group path.
      Block::Rule | Block::Matrix { .. } => {}
    }

    i += 1;
  }

  out
}

// ── Table rendering helpers ───────────────────────────────────────────────────

/// Build a horizontal border line for a table.
/// Example: make_border(&[10,6], '┌','┬','┐','─') → "┌────────────┬────────┐"
fn make_border(col_widths: &[usize], left: char, mid: char, right: char, fill: char) -> String {
  let mut s = String::new();
  s.push(left);
  for (j, &w) in col_widths.iter().enumerate() {
    for _ in 0..w + 2 { s.push(fill); }
    if j + 1 < col_widths.len() { s.push(mid); }
  }
  s.push(right);
  s
}

/// Format a single data row: "│ cell1 │ cell2 │"
fn format_data_row(padded_cells: &[String]) -> String {
  let mut s = String::from('│');
  for cell in padded_cells {
    s.push(' ');
    s.push_str(cell);
    s.push(' ');
    s.push('│');
  }
  s
}

/// Render a table group — a run of consecutive `Block::Rule` and `Block::Matrix` blocks.
/// Computes shared column widths and emits proper box-drawing borders.
fn render_table_group(group: &[Block], base_block_idx: usize, out: &mut Vec<VisualLine>) {
  // Compute unified column widths across all Matrix blocks in this group.
  let ncols = group.iter()
    .filter_map(|b| if let Block::Matrix { rows } = b { Some(rows) } else { None })
    .flat_map(|rows| rows.iter())
    .map(|r| r.len())
    .max()
    .unwrap_or(0);

  if ncols == 0 { return; }

  let mut col_widths = vec![0usize; ncols];
  for block in group {
    if let Block::Matrix { rows } = block {
      for row in rows {
        for (j, cell) in row.iter().enumerate() {
          if j < ncols {
            col_widths[j] = col_widths[j].max(visual_width(cell));
          }
        }
      }
    }
  }

  let top = make_border(&col_widths, '┌', '┬', '┐', '─');
  let mid = make_border(&col_widths, '├', '┼', '┤', '─');
  let bot = make_border(&col_widths, '└', '┴', '┘', '─');

  let n = group.len();
  let mut added_top = false;
  let mut seq = 0usize; // line-in-group counter for block_idx bookkeeping

  for (k, block) in group.iter().enumerate() {
    let block_idx = base_block_idx + k;
    match block {
      Block::Rule => {
        let matrix_before = group[..k].iter().any(|b| matches!(b, Block::Matrix { .. }));
        let matrix_after  = group[k + 1..].iter().any(|b| matches!(b, Block::Matrix { .. }));
        let text = if !matrix_before {
          added_top = true;
          top.clone()
        } else if !matrix_after {
          bot.clone()
        } else {
          mid.clone()
        };
        out.push(VisualLine { block_idx, line_in_block: seq, text, kind: VisualLineKind::Rule });
        seq += 1;
      }
      Block::Matrix { rows } => {
        if !added_top {
          out.push(VisualLine {
            block_idx,
            line_in_block: seq,
            text: top.clone(),
            kind: VisualLineKind::Rule,
          });
          seq += 1;
          added_top = true;
        }
        let n_rows = rows.len();
        for (ri, row) in rows.iter().enumerate() {
          let mut padded: Vec<String> = row.iter().enumerate()
            .map(|(j, cell)| {
              let w = col_widths.get(j).copied().unwrap_or(0);
              format!("{:<width$}", cell, width = w)
            })
            .collect();
          while padded.len() < ncols {
            let j = padded.len();
            padded.push(" ".repeat(col_widths.get(j).copied().unwrap_or(0)));
          }
          out.push(VisualLine {
            block_idx,
            line_in_block: seq,
            text: format_data_row(&padded),
            kind: VisualLineKind::MatrixLine { is_first: ri == 0, is_last: ri == n_rows - 1 },
          });
          seq += 1;
        }
        if k == n - 1 {
          out.push(VisualLine {
            block_idx,
            line_in_block: seq,
            text: bot.clone(),
            kind: VisualLineKind::Rule,
          });
        }
      }
      _ => {}
    }
  }
}

/// Center `line` (of visual width `block_width`) within `terminal_width`.
fn center_line(line: &str, block_width: usize, terminal_width: usize) -> String {
  if terminal_width <= block_width {
    return line.to_string();
  }
  let pad = (terminal_width - block_width) / 2;
  format!("{}{}", " ".repeat(pad), line)
}

/// Approximate visual column width of a string (ASCII chars = 1, others = 1 for now).
/// A full Unicode-aware implementation can replace this without API changes.
fn visual_width(s: &str) -> usize {
  s.chars().count()
}

/// Word-wrap a sequence of styled spans to `width` columns.
/// Returns a vec of (line_spans, plain_text) pairs — one entry per visual line.
/// Adjacent words with identical style are coalesced into a single span.
fn wrap_spans(spans: &[InlineSpan], width: usize) -> Vec<(Vec<InlineSpan>, String)> {
  // Collect all words with their per-span style metadata.
  struct Word {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    monospace: bool,
    color: Option<(u8, u8, u8)>,
    url: Option<String>,
  }

  let mut words: Vec<Word> = Vec::new();
  for span in spans {
    for word in span.text.split_whitespace() {
      if !word.is_empty() {
        words.push(Word {
          text: word.to_string(),
          bold: span.bold,
          italic: span.italic,
          underline: span.underline,
          strikethrough: span.strikethrough,
          monospace: span.monospace,
          color: span.color,
          url: span.url.clone(),
        });
      }
    }
  }

  if words.is_empty() {
    return vec![(vec![], String::new())];
  }

  let effective_width = width.max(1);
  let mut result: Vec<(Vec<InlineSpan>, String)> = Vec::new();
  let mut line_spans: Vec<InlineSpan> = Vec::new();
  let mut line_plain = String::new();
  let mut line_width = 0usize;

  for word in &words {
    let wlen = word.text.chars().count();
    let needed = if line_width == 0 { wlen } else { line_width + 1 + wlen };

    if line_width > 0 && needed > effective_width {
      result.push((std::mem::take(&mut line_spans), std::mem::take(&mut line_plain)));
      line_width = 0;
    }

    let prefix = if line_width > 0 { " " } else { "" };
    let token = format!("{}{}", prefix, word.text);
    line_plain.push_str(&token);
    line_width += token.chars().count();

    // Coalesce with previous span when style is identical.
    let coalesce = line_spans.last().map_or(false, |last| {
      last.bold == word.bold
        && last.italic == word.italic
        && last.underline == word.underline
        && last.strikethrough == word.strikethrough
        && last.monospace == word.monospace
        && last.color == word.color
        && last.url == word.url
    });

    if coalesce {
      line_spans.last_mut().unwrap().text.push_str(&token);
    } else {
      line_spans.push(InlineSpan {
        text: token,
        bold: word.bold,
        italic: word.italic,
        underline: word.underline,
        strikethrough: word.strikethrough,
        monospace: word.monospace,
        color: word.color,
        url: word.url.clone(),
      });
    }
  }

  if !line_plain.is_empty() {
    result.push((line_spans, line_plain));
  }

  result
}

/// Wrap a list item's content to `width`, prepending the indent+marker prefix.
/// Returns (line_spans, plain_text, is_continuation) per visual line.
fn wrap_list_item(
  depth: u8,
  marker: &str,
  content: &[InlineSpan],
  width: usize,
) -> Vec<(Vec<InlineSpan>, String, bool)> {
  let indent_len = depth as usize * 2;
  let prefix_len = indent_len + marker.len();
  let content_width = width.saturating_sub(prefix_len).max(1);

  let wrapped = wrap_spans(content, content_width);

  wrapped
    .into_iter()
    .enumerate()
    .map(|(i, (spans, plain))| {
      let is_continuation = i > 0;
      let prefix = if is_continuation {
        format!("{}{}", "  ".repeat(depth as usize), " ".repeat(marker.len()))
      } else {
        format!("{}{}", "  ".repeat(depth as usize), marker)
      };
      let plain_with_prefix = format!("{}{}", prefix, plain);
      let mut all_spans = vec![InlineSpan::plain(prefix)];
      all_spans.extend(spans);
      (all_spans, plain_with_prefix, is_continuation)
    })
    .collect()
}

/// Convert a flat Vec<String> into Vec<Block> with no behavioral change.
/// Empty strings become Block::Blank; all others become Block::Line.
pub fn from_lines(lines: Vec<String>) -> Vec<Block> {
  lines
    .into_iter()
    .map(|s| if s.is_empty() { Block::Blank } else { Block::Line(s) })
    .collect()
}
