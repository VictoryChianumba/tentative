/// The format of the math input.
pub enum MathInput<'a> {
  /// Raw LaTeX source, e.g. `\frac{x^2 + 1}{y}`.
  Latex(&'a str),
  /// MathML XML string, e.g. from an EPUB `<math>` block.
  MathMl(&'a str),
}

/// Render math to a Unicode string suitable for terminal display.
///
/// On any rendering error the raw source is cleaned and returned so the
/// caller always gets displayable text — never a blank or a panic.
pub fn render(input: MathInput<'_>) -> String {
  match input {
    MathInput::Latex(src) => {
      let preprocessed = preprocess(src);
      match tui_math::render_latex(&preprocessed) {
        Ok(s) if !s.contains("[PARSE ERROR:") => s,
        // tui-math partially succeeded but embedded error markers — strip instead.
        _ => strip_latex(src),
      }
    }
    MathInput::MathMl(src) => src.to_string(),
  }
}

// ── Preprocessing ─────────────────────────────────────────────────────────────

/// Replace or clean commands that tui-math does not support before rendering.
fn preprocess(src: &str) -> String {
  let mut s = src.to_string();

  // \mathbb{X} → Unicode double-struck equivalents.
  for (letter, sym) in MATHBB {
    s = s.replace(&format!("\\mathbb{{{letter}}}"), sym);
  }

  // Commands where content should be kept, wrapper dropped.
  for cmd in &["mathcal", "mathbf", "mathit", "mathsf", "mathrm", "text",
               "mbox", "hbox", "boldsymbol", "bm", "operatorname"] {
    s = replace_braced_cmd(&s, cmd);
  }

  // Commands where content should be silently dropped.
  for cmd in &["label", "tag", "nonumber", "notag"] {
    s = remove_braced_cmd(&s, cmd);
  }

  // Simple token replacements.
  let tokens: &[(&str, &str)] = &[
    ("\\nonumber", ""),
    ("\\notag", ""),
    ("\\quad", " "),
    ("\\qquad", "  "),
    ("\\,", " "),
    ("\\;", " "),
    ("\\!", ""),
    ("\\dots", "..."),
    ("\\cdots", "..."),
    ("\\ldots", "..."),
    ("\\vdots", ":"),
    ("\\ddots", "..."),
    ("\\lvert", "|"),
    ("\\rvert", "|"),
    ("\\lVert", "‖"),
    ("\\rVert", "‖"),
    ("\\mid", "|"),
    ("\\colon", ":"),
  ];
  for (from, to) in tokens {
    s = s.replace(from, to);
  }

  s
}

const MATHBB: &[(&str, &str)] = &[
  ("R", "ℝ"), ("N", "ℕ"), ("Z", "ℤ"), ("Q", "ℚ"), ("C", "ℂ"),
  ("P", "ℙ"), ("E", "𝔼"), ("F", "𝔽"), ("H", "ℍ"), ("1", "𝟙"),
];

/// Replace `\cmd{content}` with just `content`.
fn replace_braced_cmd(src: &str, cmd: &str) -> String {
  let marker = format!("\\{cmd}{{");
  transform_braced_cmd(src, &marker, true)
}

/// Replace `\cmd{content}` with `""` (drop everything).
fn remove_braced_cmd(src: &str, cmd: &str) -> String {
  let marker = format!("\\{cmd}{{");
  transform_braced_cmd(src, &marker, false)
}

fn transform_braced_cmd(src: &str, marker: &str, keep_content: bool) -> String {
  let mut out = String::with_capacity(src.len());
  let mut rest = src;
  while let Some(pos) = rest.find(marker) {
    out.push_str(&rest[..pos]);
    rest = &rest[pos + marker.len()..];
    let mut depth = 1usize;
    let mut content = String::new();
    let mut consumed = 0usize;
    for c in rest.chars() {
      consumed += c.len_utf8();
      match c {
        '{' => { depth += 1; content.push(c); }
        '}' => {
          depth -= 1;
          if depth == 0 { break; }
          content.push(c);
        }
        _ => content.push(c),
      }
    }
    rest = &rest[consumed..];
    if keep_content {
      out.push_str(&content);
    }
  }
  out.push_str(rest);
  out
}

// ── Fallback strip ────────────────────────────────────────────────────────────

/// Last-resort fallback: strip LaTeX commands and return readable plain text.
/// Better than raw LaTeX or an error string; worse than proper rendering.
fn strip_latex(src: &str) -> String {
  let mut out = String::new();
  let chars: Vec<char> = src.chars().collect();
  let len = chars.len();
  let mut i = 0;

  while i < len {
    let c = chars[i];

    // Skip comments.
    if c == '%' && (i == 0 || chars[i - 1] != '\\') {
      while i < len && chars[i] != '\n' { i += 1; }
      continue;
    }

    if c == '\\' && i + 1 < len {
      i += 1;
      // Read command name.
      let start = i;
      while i < len && chars[i].is_alphabetic() { i += 1; }
      let cmd: String = chars[start..i].iter().collect();
      // Skip trailing spaces after command.
      while i < len && chars[i] == ' ' { i += 1; }

      match cmd.as_str() {
        // Keep braced content.
        "mathbb" | "mathcal" | "mathbf" | "mathit" | "mathsf" | "mathrm"
        | "text" | "mbox" | "hbox" | "operatorname" | "boldsymbol" | "bm"
        | "tilde" | "hat" | "bar" | "vec" | "dot" | "ddot" | "widehat"
        | "widetilde" | "overline" | "underline" | "overbrace" | "underbrace"
        | "sqrt" => {
          if i < len && chars[i] == '{' {
            i += 1; // skip {
            let mut depth = 1usize;
            while i < len {
              match chars[i] {
                '{' => { depth += 1; out.push(chars[i]); }
                '}' => { depth -= 1; if depth == 0 { i += 1; break; } else { out.push(chars[i]); } }
                c => out.push(c),
              }
              i += 1;
            }
          }
        }
        // Drop braced content.
        "label" | "tag" | "nonumber" | "notag" | "vspace" | "hspace" => {
          if i < len && chars[i] == '{' {
            i += 1;
            let mut depth = 1usize;
            while i < len {
              match chars[i] { '{' => depth += 1, '}' => { depth -= 1; if depth == 0 { i += 1; break; } } _ => {} }
              i += 1;
            }
          }
        }
        // Spacing → single space.
        "quad" | "qquad" => out.push(' '),
        // Newlines.
        "\\" | "newline" => out.push('\n'),
        // Everything else: try to keep braced content.
        _ => {
          if i < len && chars[i] == '{' {
            i += 1;
            let mut depth = 1usize;
            while i < len {
              match chars[i] {
                '{' => { depth += 1; out.push(chars[i]); }
                '}' => { depth -= 1; if depth == 0 { i += 1; break; } else { out.push(chars[i]); } }
                c => out.push(c),
              }
              i += 1;
            }
          }
          // If single non-alpha char (like \\, \[) just skip.
        }
      }
      continue;
    }

    // Strip bare braces.
    if c == '{' || c == '}' { i += 1; continue; }
    // Alignment char in aligned/array — treat as newline.
    if c == '&' { out.push(' '); i += 1; continue; }

    out.push(c);
    i += 1;
  }

  let result = out.trim().to_string();
  if result.is_empty() {
    "[equation]".to_string()
  } else {
    result
  }
}
