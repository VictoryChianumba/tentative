use std::cell::Cell;

pub use ratatui::style::Color;
pub use ui_theme::Theme;

thread_local! {
  static CURRENT: Cell<Theme> = Cell::new(Theme::dark());
}

pub fn set_current(theme: &Theme) {
  CURRENT.with(|current| current.set(*theme));
}

pub fn current() -> Theme {
  CURRENT.with(Cell::get)
}
