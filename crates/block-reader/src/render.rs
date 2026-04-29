use doc_model::VisualLineKind;
use ratatui::{
  Frame,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Paragraph},
};

use crate::state::{Mode, Reader};

// ── Accent palette ────────────────────────────────────────────────────────────

const BABY_BLUE: Color = Color::Rgb(100, 181, 246);
const ACCENT_DIM: Color = Color::Rgb(70, 130, 180);
const MATH_COLOR: Color = Color::Rgb(80, 200, 160);

pub fn draw(frame: &mut Frame, reader: &Reader) {
  let area = frame.area();

  let (content_area, status_area, search_area) = split_layout(area, &reader.mode);

  draw_content(frame, reader, content_area);
  draw_status(frame, reader, status_area);
  if reader.mode == Mode::Search {
    draw_search_bar(frame, reader, search_area.unwrap());
  }
}

fn split_layout(area: Rect, mode: &Mode) -> (Rect, Rect, Option<Rect>) {
  match mode {
    Mode::Normal => {
      let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
      (chunks[0], chunks[1], None)
    }
    Mode::Search => {
      let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)])
        .split(area);
      (chunks[0], chunks[1], Some(chunks[2]))
    }
  }
}

fn draw_content(frame: &mut Frame, reader: &Reader, area: Rect) {
  let ch = area.height as usize;
  let total = reader.total_lines();
  let q = reader.search_query.to_lowercase();

  let lines: Vec<Line> = (0..ch)
    .map(|row| {
      let vl_idx = reader.offset + row;
      if vl_idx >= total {
        return Line::raw("");
      }
      let vl = &reader.visual_lines[vl_idx];
      let is_cursor = row == reader.cursor_y;
      render_visual_line(vl, is_cursor, &q, &reader.search_matches, vl_idx)
    })
    .collect();

  let paragraph = Paragraph::new(lines).block(Block::default());
  frame.render_widget(paragraph, area);
}

fn render_visual_line<'a>(
  vl: &'a doc_model::VisualLine,
  is_cursor: bool,
  query: &str,
  matches: &[usize],
  vl_idx: usize,
) -> Line<'a> {
  let text = &vl.text;
  let bg = if is_cursor { Color::Rgb(30, 40, 55) } else { Color::Reset };

  let base_style = Style::default().bg(bg);

  match &vl.kind {
    VisualLineKind::Blank => Line::styled("", base_style),

    VisualLineKind::Prose => {
      if !query.is_empty() && matches.contains(&vl_idx) {
        highlight_query(text, query, bg)
      } else {
        Line::styled(text.clone(), base_style)
      }
    }

    VisualLineKind::MathLine { .. } => {
      Line::styled(text.clone(), base_style.fg(MATH_COLOR))
    }

    VisualLineKind::Header(level) => {
      let (fg, modifier) = match level {
        1 => (BABY_BLUE, Modifier::BOLD),
        2 => (ACCENT_DIM, Modifier::BOLD),
        _ => (ACCENT_DIM, Modifier::empty()),
      };
      Line::styled(text.clone(), base_style.fg(fg).add_modifier(modifier))
    }

    VisualLineKind::MatrixLine { .. } => {
      Line::styled(format!("  {}", text), base_style.fg(MATH_COLOR))
    }
  }
}

fn highlight_query(text: &str, query: &str, bg: Color) -> Line<'static> {
  let lower = text.to_lowercase();
  let mut spans: Vec<Span<'static>> = Vec::new();
  let mut pos = 0;
  let ql = query.len();

  while let Some(start) = lower[pos..].find(query) {
    let abs = pos + start;
    if abs > pos {
      spans.push(Span::styled(text[pos..abs].to_string(), Style::default().bg(bg)));
    }
    spans.push(Span::styled(
      text[abs..abs + ql].to_string(),
      Style::default().bg(Color::Yellow).fg(Color::Black),
    ));
    pos = abs + ql;
  }
  if pos < text.len() {
    spans.push(Span::styled(text[pos..].to_string(), Style::default().bg(bg)));
  }

  Line::from(spans)
}

fn draw_status(frame: &mut Frame, reader: &Reader, area: Rect) {
  let cur = reader.current_line() + 1;
  let tot = reader.total_lines();
  let pct = if tot == 0 { 0 } else { cur * 100 / tot };
  let match_info = if !reader.search_matches.is_empty() {
    format!("  [{}/{}]", reader.search_idx + 1, reader.search_matches.len())
  } else {
    String::new()
  };
  let text = format!(" {cur}/{tot}  {pct}%{match_info}");
  let status = Paragraph::new(text)
    .style(Style::default().bg(Color::Rgb(25, 35, 50)).fg(Color::DarkGray));
  frame.render_widget(status, area);
}

fn draw_search_bar(frame: &mut Frame, reader: &Reader, area: Rect) {
  let text = format!("/{}", reader.search_query);
  let bar = Paragraph::new(text)
    .style(Style::default().bg(Color::Rgb(25, 35, 50)).fg(Color::White));
  frame.render_widget(bar, area);
}
