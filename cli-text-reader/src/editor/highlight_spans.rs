use ratatui::prelude::*;

use super::core::Editor;

#[derive(Clone)]
pub struct StyleRange {
  pub start: usize,
  pub end: usize,
  pub style: Style,
  pub priority: u8,
}

pub fn build_styled_spans(
  editor: &Editor,
  document_line_index: Option<usize>,
  content: &str,
  padding: &str,
  base_style: Style,
  extra_ranges: Vec<StyleRange>,
) -> Vec<Span<'static>> {
  let mut ranges = extra_ranges;

  if let Some(document_line_index) = document_line_index {
    if let Some((start, end)) =
      editor.search_match_range_for_line(document_line_index, content)
    {
      ranges.push(StyleRange {
        start,
        end,
        style: Style::default().bg(Color::Yellow).fg(Color::Black),
        priority: 20,
      });
    }

    for (start, end) in
      editor.persistent_highlight_ranges_for_line(document_line_index, content)
    {
      ranges.push(StyleRange {
        start,
        end,
        style: Style::default().bg(Color::Yellow).fg(Color::Black),
        priority: 10,
      });
    }

    for (start, end) in
      editor.selection_ranges_for_line(document_line_index, content)
    {
      ranges.push(StyleRange {
        start,
        end,
        style: Style::default().bg(Color::Blue).fg(Color::White),
        priority: 30,
      });
    }
  }

  let mut spans = Vec::new();
  if !padding.is_empty() {
    spans.push(Span::styled(padding.to_string(), base_style));
  }

  let mut boundaries = vec![0, content.len()];
  let valid_ranges = ranges
    .into_iter()
    .filter_map(|range| {
      let start = range.start.min(content.len());
      let end = range.end.min(content.len());
      (start < end).then_some(StyleRange { start, end, ..range })
    })
    .collect::<Vec<_>>();

  for range in &valid_ranges {
    boundaries.push(range.start);
    boundaries.push(range.end);
  }

  boundaries.sort_unstable();
  boundaries.dedup();

  for window in boundaries.windows(2) {
    let start = window[0];
    let end = window[1];
    if start >= end {
      continue;
    }

    let mut segment_style = base_style;
    if let Some(range) = valid_ranges
      .iter()
      .filter(|range| range.start <= start && range.end >= end)
      .max_by_key(|range| range.priority)
    {
      segment_style = segment_style.patch(range.style);
    }

    spans.push(Span::styled(content[start..end].to_string(), segment_style));
  }

  if spans.is_empty() {
    spans.push(Span::styled(String::new(), base_style));
  }

  spans
}
