use crossterm::{
  QueueableCommand,
  cursor::MoveTo,
  style::{Color, Print, ResetColor, SetForegroundColor},
};

use super::core::Editor;
use crate::config::{AppConfig, load_config, save_config};

const FIELD_NAMES: [&str; 5] =
  ["ELEVENLABS_API_KEY", "VOICE_ID", "PLAYBACK_SPEED", "TTS_PROVIDER", "SAY_VOICE"];
const FIELD_HINTS: [&str; 5] = [
  "API key from elevenlabs.io",
  "voice ID from your ElevenLabs dashboard",
  "0.5 – 2.0  (applied on save)",
  "h/l or Enter to cycle: auto → elevenlabs → say → piper",
  "run  say -v ?  in a terminal to list all voices",
];
const TTS_OPTIONS: [&str; 4] = ["", "elevenlabs", "say", "piper"];
const POPUP_W: u16 = 60;
const POPUP_H: u16 = 20;

fn cycle_tts(field: &mut String, dir: i32) {
  let cur =
    TTS_OPTIONS.iter().position(|&o| o == field.as_str()).unwrap_or(0);
  let next =
    (cur as i32 + dir).rem_euclid(TTS_OPTIONS.len() as i32) as usize;
  *field = TTS_OPTIONS[next].to_string();
}

impl Editor {
  /// Open the settings popup, loading current values from config.
  pub fn open_settings_popup(&mut self) {
    let config = load_config();
    self.settings_fields[0] = config.elevenlabs_api_key.clone();
    self.settings_fields[1] = config.voice_id.clone();
    self.settings_fields[2] = format!("{:.1}", config.playback_speed);
    self.settings_fields[3] = config.tts_provider.clone();
    self.settings_fields[4] = config.say_voice.clone();
    self.settings_cursor = 0;
    self.settings_editing = false;
    self.show_settings = true;
    self.mark_dirty();
  }

  /// Close the settings popup without saving.
  pub fn close_settings_popup(&mut self) {
    self.show_settings = false;
    self.settings_editing = false;
    self.mark_dirty();
  }

  /// Save settings to disk and reload voice config.
  pub fn save_settings_popup(&mut self) {
    let speed: f32 =
      self.settings_fields[2].parse::<f32>().unwrap_or(1.0).clamp(0.5, 2.0);
    // Normalise the displayed speed value after clamping
    self.settings_fields[2] = format!("{speed:.1}");

    let config = AppConfig {
      elevenlabs_api_key: self.settings_fields[0].clone(),
      voice_id: self.settings_fields[1].clone(),
      playback_speed: speed,
      tts_provider: self.settings_fields[3].clone(),
      say_voice: self.settings_fields[4].clone(),
      ..Default::default()
    };
    let _ = save_config(&config);

    // Rebuild voice controller from updated config
    let updated_cfg = load_config();
    use crate::voice::playback::PlaybackController;
    self.voice_controller =
      Some(PlaybackController::new(crate::voice::make_provider(&updated_cfg)));

    self.settings_saved_until =
      Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
    self.mark_dirty();
  }

  /// Handle a key event while the settings popup is open.
  /// Returns `Some(true)` to quit, `Some(false)` to consume, `None` to fall through.
  pub fn handle_settings_key(
    &mut self,
    key_event: crossterm::event::KeyEvent,
  ) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    use crossterm::event::KeyCode;

    if self.settings_editing {
      // Text-entry mode for the active field
      match key_event.code {
        KeyCode::Esc | KeyCode::Enter => {
          // Validate PLAYBACK_SPEED on commit
          if self.settings_cursor == 2 {
            let v: f32 = self.settings_fields[2]
              .parse::<f32>()
              .unwrap_or(1.0)
              .clamp(0.5, 2.0);
            self.settings_fields[2] = format!("{v:.1}");
          }
          self.settings_editing = false;
          self.mark_dirty();
        }
        KeyCode::Char(c) => {
          self.settings_fields[self.settings_cursor].push(c);
          self.mark_dirty();
        }
        KeyCode::Backspace => {
          self.settings_fields[self.settings_cursor].pop();
          self.mark_dirty();
        }
        _ => {}
      }
      return Ok(Some(false));
    }

    // Navigation mode
    match key_event.code {
      KeyCode::Esc => {
        self.close_settings_popup();
      }
      KeyCode::Char('j') | KeyCode::Down => {
        self.settings_cursor =
          (self.settings_cursor + 1).min(FIELD_NAMES.len() - 1);
        self.mark_dirty();
      }
      KeyCode::Char('k') | KeyCode::Up => {
        self.settings_cursor = self.settings_cursor.saturating_sub(1);
        self.mark_dirty();
      }
      KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right
        if self.settings_cursor == 3 =>
      {
        cycle_tts(&mut self.settings_fields[3], 1);
        self.mark_dirty();
      }
      KeyCode::Char('h') | KeyCode::Left if self.settings_cursor == 3 => {
        cycle_tts(&mut self.settings_fields[3], -1);
        self.mark_dirty();
      }
      KeyCode::Enter => {
        self.settings_editing = true;
        self.mark_dirty();
      }
      KeyCode::Char('s') => {
        self.save_settings_popup();
      }
      _ => {}
    }
    Ok(Some(false))
  }

  /// Draw the settings popup into `buf` using crossterm queued commands.
  pub fn draw_settings_popup_buffered(
    &self,
    buf: &mut Vec<u8>,
  ) -> std::io::Result<()> {
    let term_w = self.width as u16;
    let term_h = self.height as u16;

    // Centre the popup
    let left = term_w.saturating_sub(POPUP_W) / 2;
    let top = term_h.saturating_sub(POPUP_H) / 2;

    // ── top border ──────────────────────────────────────────────────────────
    buf.queue(MoveTo(left, top))?;
    buf.queue(Print(format!(
      "┌─ Settings {:─<width$}┐",
      "",
      width = (POPUP_W as usize).saturating_sub(13)
    )))?;

    // ── inner rows ──────────────────────────────────────────────────────────
    let inner_w = (POPUP_W as usize).saturating_sub(2); // between │ borders

    for row in 1..POPUP_H - 1 {
      buf.queue(MoveTo(left, top + row))?;
      buf.queue(Print(format!("│{:<inner_w$}│", "")))?;
    }

    // ── bottom border ───────────────────────────────────────────────────────
    buf.queue(MoveTo(left, top + POPUP_H - 1))?;
    buf.queue(Print(format!(
      "└{:─<width$}┘",
      "",
      width = (POPUP_W as usize).saturating_sub(2)
    )))?;

    // ── field labels + values ───────────────────────────────────────────────
    // Layout per field: label row / value row / hint row (3 rows each)
    // Hint row is blank when not selected; shows dimmed hint when selected.

    for (i, name) in FIELD_NAMES.iter().enumerate() {
      let label_row = top + 1 + (i as u16) * 3 + 1;
      let value_row = label_row + 1;
      let hint_row = value_row + 1;

      let selected = i == self.settings_cursor;

      // Label
      buf.queue(MoveTo(left + 2, label_row))?;
      if selected {
        buf.queue(SetForegroundColor(Color::Yellow))?;
        buf.queue(Print(format!("▸ {name}")))?;
        buf.queue(ResetColor)?;
      } else {
        buf.queue(Print(format!("  {name}")))?;
      }

      // Value
      let raw = &self.settings_fields[i];
      buf.queue(MoveTo(left + 4, value_row))?;
      if i == 3 {
        // Cycle-select: show arrows when selected
        let label = if raw.is_empty() { "auto" } else { raw.as_str() };
        if selected {
          buf.queue(SetForegroundColor(Color::Cyan))?;
          buf.queue(Print(format!("◀  {label}  ▶")))?;
          buf.queue(ResetColor)?;
        } else {
          buf.queue(Print(format!("   {label}   ")))?;
        }
      } else {
        // Text-edit field
        let display: String =
          if i == 0 && !raw.is_empty() { "*".repeat(raw.len()) } else { raw.clone() };
        let max_val = inner_w.saturating_sub(4);
        let display = if display.len() > max_val {
          format!("{}…", &display[..max_val.saturating_sub(1)])
        } else {
          display
        };
        if selected && self.settings_editing {
          buf.queue(SetForegroundColor(Color::Cyan))?;
          buf.queue(Print(format!("{display}_")))?;
          buf.queue(ResetColor)?;
        } else {
          buf.queue(Print(&display))?;
        }
      }

      // Per-field hint (shown in blank row when selected)
      if selected {
        buf.queue(MoveTo(left + 4, hint_row))?;
        buf.queue(SetForegroundColor(Color::DarkGrey))?;
        buf.queue(Print(FIELD_HINTS[i]))?;
        buf.queue(ResetColor)?;
      }
    }

    // ── footer hint ──────────────────────────────────────────────────────────
    let hint_row = top + POPUP_H - 3;
    buf.queue(MoveTo(left + 2, hint_row))?;
    buf.queue(SetForegroundColor(Color::DarkGrey))?;
    if self.settings_editing {
      buf.queue(Print("Type to edit  Enter/Esc: confirm"))?;
    } else if self.settings_cursor == 3 {
      buf.queue(Print("h/l: cycle  j/k: move  s: save  Esc: close"))?;
    } else {
      buf.queue(Print("j/k: move  Enter: edit  s: save  Esc: close"))?;
    }
    buf.queue(ResetColor)?;

    // ── saved confirmation ───────────────────────────────────────────────────
    let saved_row = top + POPUP_H - 2;
    if self
      .settings_saved_until
      .map_or(false, |t| std::time::Instant::now() < t)
    {
      buf.queue(MoveTo(left + 2, saved_row))?;
      buf.queue(SetForegroundColor(Color::Green))?;
      buf.queue(Print("Saved."))?;
      buf.queue(ResetColor)?;
    }

    Ok(())
  }
}
