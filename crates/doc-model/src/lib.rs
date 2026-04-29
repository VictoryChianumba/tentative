/// Semantic block — the producer's view of the document.
#[derive(Debug, Clone)]
pub enum Block {
  /// A single line of prose, already word-wrapped by the producer.
  Line(String),
  /// A display math equation rendered as multiple Unicode lines, treated as one unit.
  DisplayMath(Vec<String>),
  /// A section header. level: 1=section, 2=subsection, 3=subsubsection/paragraph.
  Header { level: u8, text: String },
  /// A matrix rendered as a grid of cells (row-major).
  Matrix { rows: Vec<Vec<String>> },
  /// Explicit vertical space (blank line).
  Blank,
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
}

/// Expand a block list into the flat visual line table.
///
/// Called once at document load and again on terminal resize (only the
/// centering offset of MathLine entries changes on resize).
pub fn build_visual_lines(blocks: &[Block], terminal_width: usize) -> Vec<VisualLine> {
  let mut out = Vec::new();

  for (block_idx, block) in blocks.iter().enumerate() {
    match block {
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

      Block::DisplayMath(lines) => {
        let block_width = lines.iter().map(|l| visual_width(l)).max().unwrap_or(0);
        let n = lines.len();
        for (i, line) in lines.iter().enumerate() {
          let centered = center_line(line, block_width, terminal_width);
          out.push(VisualLine {
            block_idx,
            line_in_block: i,
            text: centered,
            kind: VisualLineKind::MathLine {
              block_width,
              is_first: i == 0,
              is_last: i == n - 1,
            },
          });
        }
      }

      Block::Matrix { rows } => {
        if rows.is_empty() {
          continue;
        }
        // Compute max width per column.
        let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut col_widths = vec![0usize; ncols];
        for row in rows {
          for (j, cell) in row.iter().enumerate() {
            col_widths[j] = col_widths[j].max(visual_width(cell));
          }
        }
        let n = rows.len();
        for (i, row) in rows.iter().enumerate() {
          let mut cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(j, cell)| {
              let w = col_widths.get(j).copied().unwrap_or(0);
              format!("{:<width$}", cell, width = w)
            })
            .collect();
          // Pad missing columns.
          while cells.len() < ncols {
            let j = cells.len();
            cells.push(" ".repeat(col_widths.get(j).copied().unwrap_or(0)));
          }
          let text = cells.join("  ");
          out.push(VisualLine {
            block_idx,
            line_in_block: i,
            text,
            kind: VisualLineKind::MatrixLine {
              is_first: i == 0,
              is_last: i == n - 1,
            },
          });
        }
      }
    }
  }

  out
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

/// Convert a flat Vec<String> into Vec<Block> with no behavioral change.
/// Empty strings become Block::Blank; all others become Block::Line.
pub fn from_lines(lines: Vec<String>) -> Vec<Block> {
  lines
    .into_iter()
    .map(|s| if s.is_empty() { Block::Blank } else { Block::Line(s) })
    .collect()
}
