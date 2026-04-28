use ratatui::{
  layout::Rect,
  prelude::*,
  widgets::{Block, Clear, Paragraph},
};

use crate::editor::Editor;

const POPUP_W: u16 = 60;
const POPUP_H: u16 = 20;
const FIELD_NAMES: [&str; 5] =
  ["ELEVENLABS_API_KEY", "VOICE_ID", "PLAYBACK_SPEED", "TTS_PROVIDER", "SAY_VOICE"];
const FIELD_HINTS: [&str; 5] = [
  "API key from elevenlabs.io",
  "voice ID from your ElevenLabs dashboard",
  "0.5 – 2.0  (applied on save)",
  "h/l or Enter to cycle: auto → elevenlabs → say → piper",
  "run  say -v ?  in a terminal to list all voices",
];

pub fn draw_settings_popup(frame: &mut Frame, editor: &Editor, area: Rect) {
  let left = area.x + area.width.saturating_sub(POPUP_W) / 2;
  let top = area.y + area.height.saturating_sub(POPUP_H) / 2;
  let popup_area = Rect { x: left, y: top, width: POPUP_W, height: POPUP_H };

  frame.render_widget(Clear, popup_area);
  frame.render_widget(Block::bordered().title(" Settings "), popup_area);

  let inner_w = (POPUP_W as usize).saturating_sub(2);
  let max_val = inner_w.saturating_sub(4);

  for (i, name) in FIELD_NAMES.iter().enumerate() {
    let label_row = top + 1 + (i as u16) * 3 + 1;
    let value_row = label_row + 1;
    let hint_row = value_row + 1;
    let selected = i == editor.settings_cursor;

    let label_style =
      if selected { Style::default().fg(Color::Yellow) } else { Style::default() };
    let label =
      if selected { format!("▸ {name}") } else { format!("  {name}") };
    frame.render_widget(
      Paragraph::new(label).style(label_style),
      Rect { x: left + 2, y: label_row, width: POPUP_W - 4, height: 1 },
    );

    let raw = &editor.settings_fields[i];
    let (value_text, value_style) = if i == 3 {
      let label = if raw.is_empty() { "auto" } else { raw.as_str() };
      if selected {
        (format!("◀  {label}  ▶"), Style::default().fg(Color::Cyan))
      } else {
        (format!("   {label}   "), Style::default())
      }
    } else {
      let display: String =
        if i == 0 && !raw.is_empty() { "*".repeat(raw.len()) } else { raw.clone() };
      let display = if display.len() > max_val {
        format!("{}…", &display[..max_val.saturating_sub(1)])
      } else {
        display
      };
      if selected && editor.settings_editing {
        (format!("{display}_"), Style::default().fg(Color::Cyan))
      } else {
        (display, Style::default())
      }
    };
    frame.render_widget(
      Paragraph::new(value_text).style(value_style),
      Rect { x: left + 4, y: value_row, width: POPUP_W - 8, height: 1 },
    );

    if selected {
      frame.render_widget(
        Paragraph::new(FIELD_HINTS[i])
          .style(Style::default().fg(Color::DarkGray)),
        Rect { x: left + 4, y: hint_row, width: POPUP_W - 8, height: 1 },
      );
    }
  }

  let hint_row = top + POPUP_H - 3;
  let hint = if editor.settings_editing {
    "Type to edit  Enter/Esc: confirm"
  } else if editor.settings_cursor == 3 {
    "h/l: cycle  j/k: move  s: save  Esc: close"
  } else {
    "j/k: move  Enter: edit  s: save  Esc: close"
  };
  frame.render_widget(
    Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)),
    Rect { x: left + 2, y: hint_row, width: POPUP_W - 4, height: 1 },
  );

  let saved_row = top + POPUP_H - 2;
  if editor
    .settings_saved_until
    .map_or(false, |t| std::time::Instant::now() < t)
  {
    frame.render_widget(
      Paragraph::new("Saved.").style(Style::default().fg(Color::Green)),
      Rect { x: left + 2, y: saved_row, width: POPUP_W - 4, height: 1 },
    );
  }
}
