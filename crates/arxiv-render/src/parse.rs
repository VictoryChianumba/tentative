use doc_model::Block;
use math_render::{MathInput, render};
use std::collections::HashMap;

const WRAP_WIDTH: usize = 80;

#[derive(Clone)]
enum ListKind {
  Itemize,
  Enumerate(usize),
  Description,
}

const THEOREM_ENVS: &[&str] = &[
  "theorem", "lemma", "proposition", "corollary", "definition",
  "remark", "example", "proof", "claim", "conjecture",
];

const FULL_SKIP_ENVS: &[&str] = &[
  "lstlisting", "verbatim", "tikzpicture", "algorithm", "algorithmic",
  "minipage", "thebibliography", "pgfpicture",
];

const CAPTION_ENVS: &[&str] = &[
  "figure", "figure*", "table", "table*", "wrapfigure", "subfigure",
];

const TABULAR_ENVS: &[&str] = &[
  "tabular", "tabular*", "longtable", "tabularx", "tabulary", "array",
];

/// Convert a set of `.tex` source files into a semantic block document.
pub fn to_blocks(sources: Vec<(String, String)>) -> Vec<Block> {
  let file_map: HashMap<String, String> = sources.into_iter().collect();
  let root = find_root(&file_map);
  let expanded = expand_inputs(&root, &file_map, 0);

  let macros = extract_macros(&expanded);

  let title = extract_command_arg(&expanded, "title").map(clean_inline);
  let authors = extract_command_arg(&expanded, "author").map(clean_authors);

  let body = extract_body(&expanded);
  let mut footnotes: Vec<String> = Vec::new();
  let body_blocks = process(&body, &macros, &mut footnotes);

  let mut out: Vec<Block> = Vec::new();
  out.push(Block::Blank);
  if let Some(t) = title {
    if !t.is_empty() {
      out.push(Block::Header { level: 1, text: t });
    }
  }
  if let Some(a) = authors {
    if !a.is_empty() {
      out.push(Block::Line(a));
    }
  }
  out.push(Block::Blank);
  out.extend(body_blocks);

  if !footnotes.is_empty() {
    out.push(Block::Blank);
    out.push(Block::Header { level: 2, text: "Notes".to_string() });
    for (i, note) in footnotes.iter().enumerate() {
      out.push(Block::Line(format!("[{}] {}", i + 1, note)));
    }
  }

  out
}

// ── Root selection ────────────────────────────────────────────────────────────

fn find_root(files: &HashMap<String, String>) -> String {
  for content in files.values() {
    if content.contains(r"\begin{document}") {
      return content.clone();
    }
  }
  files.values().max_by_key(|c| c.len()).cloned().unwrap_or_default()
}

// ── \input{} resolution ───────────────────────────────────────────────────────

fn expand_inputs(content: &str, files: &HashMap<String, String>, depth: usize) -> String {
  if depth > 10 {
    return content.to_string();
  }
  let mut out = String::with_capacity(content.len());
  let mut rest = content;
  while let Some(pos) = rest.find(r"\input{") {
    out.push_str(&rest[..pos]);
    rest = &rest[pos + 7..];
    if let Some(end) = rest.find('}') {
      let filename = rest[..end].trim();
      rest = &rest[end + 1..];
      if let Some(included) = resolve_input(filename, files) {
        out.push_str(&expand_inputs(&included, files, depth + 1));
      }
    }
  }
  out.push_str(rest);
  out
}

fn resolve_input(name: &str, files: &HashMap<String, String>) -> Option<String> {
  let candidates = [
    name.to_string(),
    format!("{name}.tex"),
    std::path::Path::new(name)
      .file_name()
      .map(|n| n.to_string_lossy().to_string())
      .unwrap_or_default(),
    format!(
      "{}.tex",
      std::path::Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
    ),
  ];
  for c in &candidates {
    if let Some(content) = files.get(c.as_str()) {
      return Some(content.clone());
    }
  }
  None
}

// ── Document body extraction ──────────────────────────────────────────────────

fn extract_body(content: &str) -> String {
  let start = content
    .find(r"\begin{document}")
    .map(|p| p + r"\begin{document}".len())
    .unwrap_or(0);
  let end = content.rfind(r"\end{document}").unwrap_or(content.len());
  content[start..end].to_string()
}

// ── Main processor ────────────────────────────────────────────────────────────

fn process(
  body: &str,
  macros: &HashMap<String, (usize, String)>,
  footnotes: &mut Vec<String>,
) -> Vec<Block> {
  let mut out: Vec<Block> = Vec::new();

  if let Some(abs) = extract_env(body, "abstract") {
    out.push(Block::Blank);
    out.push(Block::Header { level: 2, text: "Abstract".to_string() });
    for line in process_prose(&abs, macros) {
      out.push(Block::Line(line));
    }
    out.push(Block::Blank);
  }

  let mut list_stack: Vec<ListKind> = Vec::new();
  out.extend(process_body(body, macros, footnotes, &mut list_stack));
  out
}

// ── Body state machine ────────────────────────────────────────────────────────

fn process_body(
  body: &str,
  macros: &HashMap<String, (usize, String)>,
  footnotes: &mut Vec<String>,
  list_stack: &mut Vec<ListKind>,
) -> Vec<Block> {
  let mut out: Vec<Block> = Vec::new();
  let mut current_line = String::new();

  let display_math_envs = [
    "equation", "equation*", "align", "align*", "aligned",
    "gather", "gather*", "multline", "multline*", "eqnarray", "eqnarray*",
  ];

  let mut i = 0usize;
  let text: Vec<char> = body.chars().collect();
  let len = text.len();

  while i < len {
    let c = text[i];

    // LaTeX comment: skip to end of line.
    if c == '%' && (i == 0 || text[i - 1] != '\\') {
      while i < len && text[i] != '\n' { i += 1; }
      continue;
    }

    // Backslash command.
    if c == '\\' && i + 1 < len {
      let (cmd, consumed) = read_command(&text, i + 1);
      i += 1 + consumed;

      match cmd.as_str() {
        "end" => {
          let (env, skip) = read_braced_arg(&text, i);
          i += skip;
          if matches!(env.trim(), "itemize" | "enumerate" | "description") {
            list_stack.pop();
          }
          continue;
        }
        "begin" => {
          let (env, skip) = read_braced_arg(&text, i);
          i += skip;
          let env = env.trim().to_string();

          if display_math_envs.iter().any(|e| *e == env.as_str()) {
            flush_line(&mut current_line, &mut out);
            let (math, adv) = read_until_end(&text, i, &env);
            i += adv;
            let rendered = render_math(&math, macros);
            let lines: Vec<String> = rendered.lines().map(|l| l.to_string()).collect();
            if !lines.is_empty() { out.push(Block::DisplayMath(lines)); }
            continue;
          }

          if env == "abstract" {
            let (_abs, adv) = read_until_end(&text, i, "abstract");
            i += adv;
            continue;
          }

          if THEOREM_ENVS.contains(&env.as_str()) {
            flush_line(&mut current_line, &mut out);
            out.push(Block::Blank);
            out.push(Block::Header { level: 3, text: capitalize(&env) });
            continue;
          }

          if env == "itemize" || env == "description" {
            list_stack.push(ListKind::Itemize);
            continue;
          }
          if env == "enumerate" {
            list_stack.push(ListKind::Enumerate(0));
            continue;
          }

          if CAPTION_ENVS.contains(&env.as_str()) {
            let (body_text, adv) = read_until_end(&text, i, &env);
            i += adv;
            flush_line(&mut current_line, &mut out);
            if let Some(cap) = extract_caption(&body_text) {
              out.push(Block::Line(format!("[Figure: {}]", cap)));
            }
            continue;
          }

          if TABULAR_ENVS.contains(&env.as_str()) {
            let (body_text, adv) = read_until_end(&text, i, &env);
            i += adv;
            flush_line(&mut current_line, &mut out);
            if let Some(matrix) = parse_tabular(&body_text) {
              out.push(matrix);
            }
            continue;
          }

          if FULL_SKIP_ENVS.contains(&env.as_str()) {
            let (_body_text, adv) = read_until_end(&text, i, &env);
            i += adv;
            continue;
          }

          continue;
        }

        "section" => {
          let (title, skip) = read_braced_arg(&text, i);
          i += skip;
          flush_line(&mut current_line, &mut out);
          out.push(Block::Blank);
          out.push(Block::Header { level: 1, text: title.trim().to_string() });
          out.push(Block::Blank);
        }
        "subsection" => {
          let (title, skip) = read_braced_arg(&text, i);
          i += skip;
          flush_line(&mut current_line, &mut out);
          out.push(Block::Blank);
          out.push(Block::Header { level: 2, text: title.trim().to_string() });
          out.push(Block::Blank);
        }
        "subsubsection" | "paragraph" => {
          let (title, skip) = read_braced_arg(&text, i);
          i += skip;
          flush_line(&mut current_line, &mut out);
          out.push(Block::Header { level: 3, text: title.trim().to_string() });
        }

        "emph" | "textbf" | "textit" | "texttt" | "text" | "mathrm"
        | "mathbf" | "mathit" | "mathcal" | "mathbb"
        | "textsubscript" | "textsuperscript" | "textnormal"
        | "underline" | "overline" | "uline" => {
          let (content, skip) = read_braced_arg(&text, i);
          i += skip;
          current_line.push_str(&content);
        }

        "textcolor" | "colorbox" | "fbox" | "mbox" | "makebox" => {
          let (_opt, skip1) = read_braced_arg(&text, i);
          i += skip1;
          let (content, skip2) = read_braced_arg(&text, i);
          i += skip2;
          current_line.push_str(&content);
        }

        // Ellipsis commands → Unicode.
        "ldots" | "cdots" | "dots" | "dotsc" | "dotsb" | "dotsi" => {
          current_line.push('…');
        }

        // Special letter commands → Unicode.
        "ss" => current_line.push('ß'),
        "ae" => current_line.push('æ'),
        "AE" => current_line.push('Æ'),
        "oe" => current_line.push('œ'),
        "OE" => current_line.push('Œ'),
        "aa" => current_line.push('å'),
        "AA" => current_line.push('Å'),
        "o"  => current_line.push('ø'),
        "O"  => current_line.push('Ø'),
        "l"  => current_line.push('ł'),
        "L"  => current_line.push('Ł'),
        "i"  => current_line.push('ı'),

        // Non-alphabetic accent commands: \' \" \` \^ \~ \. \=
        "'" | "`" | "\"" | "^" | "~" | "." | "=" => {
          let (base, skip) = read_accent_arg(&text, i);
          i += skip;
          match accent_char(&cmd, base.trim()) {
            Some(ch) => current_line.push(ch),
            None => current_line.push_str(base.trim()),
          }
        }
        // Alphabetic accent commands: \c \H \v \k \u \r
        "c" | "H" | "v" | "k" | "u" | "r" => {
          if i < len && (text[i] == '{' || text[i].is_alphabetic()) {
            let (base, skip) = read_accent_arg(&text, i);
            i += skip;
            match accent_char(&cmd, base.trim()) {
              Some(ch) => current_line.push(ch),
              None => current_line.push_str(base.trim()),
            }
          }
        }

        // Backslash-space → literal space.
        " " => current_line.push(' '),

        "color" | "bibliography" | "bibliographystyle" | "maketitle"
        | "tableofcontents" | "newcommand" | "renewcommand" | "providecommand"
        | "setlength" | "addtolength" | "setcounter" | "addtocounter"
        | "usepackage" | "RequirePackage" | "PassOptionsToPackage"
        | "geometry" | "vspace*" | "hspace" | "hspace*" | "rule"
        | "includegraphics" | "captionsetup" | "caption" | "subcaption"
        | "pagestyle" | "thispagestyle" | "pagenumbering"
        | "definecolor" | "colorlet" | "DeclareMathOperator"
        | "theoremstyle" | "newtheorem" | "newenvironment" | "renewenvironment"
        | "crefname" | "Crefname" | "hypersetup" | "setcitestyle"
        | "IEEEauthorblockN" | "IEEEauthorblockA" | "institute"
        | "affil" | "address" | "email" | "date" => {
          while i < len && (text[i] == '{' || text[i] == '[') {
            if text[i] == '[' {
              while i < len && text[i] != ']' { i += 1; }
              if i < len { i += 1; }
            } else {
              let (_, skip) = read_braced_arg(&text, i);
              i += skip;
            }
          }
        }

        "cite" | "citep" | "citet" | "citealt" | "citealp" | "citeauthor"
        | "citeyear" | "nocite" => {
          if i < len && text[i] == '[' {
            while i < len && text[i] != ']' { i += 1; }
            if i < len { i += 1; }
          }
          let (_key, skip) = read_braced_arg(&text, i);
          i += skip;
          if cmd != "nocite" { current_line.push_str("[ref]"); }
        }
        "ref" | "eqref" | "cref" | "Cref" | "autoref" | "vref"
        | "nameref" | "pageref" => {
          let (_key, skip) = read_braced_arg(&text, i);
          i += skip;
          current_line.push_str("[§]");
        }
        "label" => {
          let (_key, skip) = read_braced_arg(&text, i);
          i += skip;
        }
        "footnote" | "footnotetext" => {
          if i < len && text[i] == '[' {
            while i < len && text[i] != ']' { i += 1; }
            if i < len { i += 1; }
          }
          let (note, skip) = read_braced_arg(&text, i);
          i += skip;
          let n = footnotes.len() + 1;
          footnotes.push(render_text_with_math(&note, macros));
          current_line.push_str(&format!("[{}]", n));
        }
        "hyperref" => {
          if i < len && text[i] == '[' {
            while i < len && text[i] != ']' { i += 1; }
            if i < len { i += 1; }
          }
          let (content, skip) = read_braced_arg(&text, i);
          i += skip;
          current_line.push_str(&content);
        }
        "url" | "href" => {
          let (url, skip) = read_braced_arg(&text, i);
          i += skip;
          if cmd == "href" {
            let (display, skip2) = read_braced_arg(&text, i);
            i += skip2;
            current_line.push_str(&display);
          } else {
            current_line.push_str(url.trim());
          }
        }
        "item" => {
          if i < len && text[i] == '[' {
            while i < len && text[i] != ']' { i += 1; }
            if i < len { i += 1; }
          }
          flush_line(&mut current_line, &mut out);
          let marker = match list_stack.last_mut() {
            Some(ListKind::Enumerate(n)) => { *n += 1; format!("{}. ", n) }
            _ => "• ".to_string(),
          };
          current_line.push_str(&marker);
        }

        // Inline math \( ... \)
        "(" => {
          let (math, adv) = read_until_str(&text, i, r"\)");
          i += adv;
          let rendered = render_math(&math, macros);
          if rendered.contains('\n') {
            flush_line(&mut current_line, &mut out);
            let lines: Vec<String> = rendered.lines().map(|l| l.to_string()).collect();
            out.push(Block::DisplayMath(lines));
          } else {
            current_line.push_str(&rendered);
          }
        }
        // Display math \[ ... \]
        "[" => {
          let (math, adv) = read_until_str(&text, i, r"\]");
          i += adv;
          flush_line(&mut current_line, &mut out);
          let rendered = render_math(&math, macros);
          let lines: Vec<String> = rendered.lines().map(|l| l.to_string()).collect();
          if !lines.is_empty() { out.push(Block::DisplayMath(lines)); }
        }

        "\\" | "newline" | "hline" => flush_line(&mut current_line, &mut out),
        "par" | "medskip" | "bigskip" | "smallskip" | "vspace" | "vskip" => {
          let _ = if cmd == "vspace" || cmd == "vskip" {
            let (_arg, skip) = read_braced_arg(&text, i);
            i += skip;
          };
          flush_line(&mut current_line, &mut out);
          out.push(Block::Blank);
        }

        _ => {
          if let Some((arity, def)) = macros.get(cmd.as_str()) {
            let def = def.clone();
            match arity {
              0 => {
                let expanded = if def.contains('\\') {
                  render_math(&def, macros)
                } else {
                  def
                };
                if expanded.contains('\n') {
                  flush_line(&mut current_line, &mut out);
                  let lines: Vec<String> =
                    expanded.lines().map(|l| l.to_string()).collect();
                  out.push(Block::DisplayMath(lines));
                } else {
                  current_line.push_str(&expanded);
                }
              }
              1 => {
                if i < len && text[i] == '{' {
                  let (arg, skip) = read_braced_arg(&text, i);
                  i += skip;
                  let substituted = def.replace("#1", &arg);
                  let expanded = if substituted.contains('\\') {
                    render_math(&substituted, macros)
                  } else {
                    substituted
                  };
                  if expanded.contains('\n') {
                    flush_line(&mut current_line, &mut out);
                    let lines: Vec<String> =
                      expanded.lines().map(|l| l.to_string()).collect();
                    out.push(Block::DisplayMath(lines));
                  } else {
                    current_line.push_str(&expanded);
                  }
                }
              }
              _ => {}
            }
          } else if i < len && text[i] == '{' {
            let (content, skip) = read_braced_arg(&text, i);
            i += skip;
            if content.contains(' ') || content.contains('\n') || content.contains(',') {
              current_line.push_str(&content);
            }
          }
        }
      }
      continue;
    }

    // Inline math $...$ (single dollar sign, not $$).
    if c == '$' {
      if i + 1 < len && text[i + 1] == '$' {
        i += 2;
        let (math, adv) = read_until_double_dollar(&text, i);
        i += adv;
        flush_line(&mut current_line, &mut out);
        let rendered = render_math(&math, macros);
        let lines: Vec<String> = rendered.lines().map(|l| l.to_string()).collect();
        if !lines.is_empty() { out.push(Block::DisplayMath(lines)); }
      } else {
        i += 1;
        let (math, adv) = read_until_single_dollar(&text, i);
        i += adv;
        let rendered = render_math(&math, macros);
        if rendered.contains('\n') {
          flush_line(&mut current_line, &mut out);
          let lines: Vec<String> = rendered.lines().map(|l| l.to_string()).collect();
          out.push(Block::DisplayMath(lines));
        } else {
          current_line.push_str(&rendered);
        }
      }
      continue;
    }

    // Non-breaking space → regular space.
    if c == '~' {
      current_line.push(' ');
      i += 1;
      continue;
    }

    // Dash ligatures: --- → em dash, -- → en dash.
    if c == '-' {
      if i + 2 < len && text[i + 1] == '-' && text[i + 2] == '-' {
        current_line.push('—');
        i += 3;
        continue;
      } else if i + 1 < len && text[i + 1] == '-' {
        current_line.push('–');
        i += 2;
        continue;
      }
    }

    // Strip bare grouping braces — content is handled when the command is read.
    if c == '{' || c == '}' {
      i += 1;
      continue;
    }

    if c == '\n' {
      if i + 1 < len && text[i + 1] == '\n' {
        flush_line(&mut current_line, &mut out);
        out.push(Block::Blank);
        while i < len && text[i] == '\n' { i += 1; }
        continue;
      } else {
        current_line.push(' ');
      }
    } else {
      current_line.push(c);
    }
    i += 1;
  }

  flush_line(&mut current_line, &mut out);
  wrap_blocks(out)
}

// ── Math rendering ────────────────────────────────────────────────────────────

/// Expand user-defined macros in a math string, then render to Unicode.
fn render_math(math: &str, macros: &HashMap<String, (usize, String)>) -> String {
  let expanded = expand_math_macros(math.trim(), macros, 0);
  // Normalize: \_ → _ (escaped underscore valid in text, redundant in math).
  let cleaned = expanded.replace(r"\_", "_");
  render(MathInput::Latex(cleaned.trim()))
}

/// Recursively expand user-defined macros inside a LaTeX math string.
fn expand_math_macros(
  math: &str,
  macros: &HashMap<String, (usize, String)>,
  depth: usize,
) -> String {
  if depth > 8 || macros.is_empty() {
    return math.to_string();
  }
  let chars: Vec<char> = math.chars().collect();
  let len = chars.len();
  let mut out = String::new();
  let mut i = 0;
  while i < len {
    if chars[i] != '\\' || i + 1 >= len {
      out.push(chars[i]);
      i += 1;
      continue;
    }
    let (cmd, consumed) = read_command(&chars, i + 1);
    if let Some((arity, def)) = macros.get(cmd.as_str()) {
      i += 1 + consumed;
      let def = def.clone();
      match arity {
        0 => out.push_str(&expand_math_macros(&def, macros, depth + 1)),
        1 => {
          if i < len && chars[i] == '{' {
            let (arg, skip) = read_braced_arg(&chars, i);
            i += skip;
            let substituted = def.replace("#1", &arg);
            out.push_str(&expand_math_macros(&substituted, macros, depth + 1));
          } else {
            out.push('\\');
            out.push_str(&cmd);
          }
        }
        _ => {
          out.push('\\');
          out.push_str(&cmd);
        }
      }
    } else {
      out.push('\\');
      out.push_str(&cmd);
      i += 1 + consumed;
    }
  }
  out
}

/// Render a prose string that may contain inline `$...$` math.
/// Used for footnotes so that math in notes is rendered, not left as raw LaTeX.
fn render_text_with_math(s: &str, macros: &HashMap<String, (usize, String)>) -> String {
  let chars: Vec<char> = s.chars().collect();
  let mut out = String::new();
  let mut i = 0;
  while i < chars.len() {
    if chars[i] == '%' && (i == 0 || chars[i - 1] != '\\') {
      while i < chars.len() && chars[i] != '\n' { i += 1; }
      continue;
    }
    if chars[i] == '$' {
      i += 1;
      let (math, adv) = read_until_single_dollar(&chars, i);
      i += adv;
      let rendered = render_math(math.trim(), macros);
      // Collapse multi-line math to a single line for inline footnote context.
      let flat = rendered
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
      out.push_str(&flat);
      continue;
    }
    if chars[i] == '~' {
      out.push(' ');
      i += 1;
      continue;
    }
    if chars[i] == '\\' && i + 1 < chars.len() {
      let (cmd, consumed) = read_command(&chars, i + 1);
      i += 1 + consumed;
      match cmd.as_str() {
        "thanks" | "footnote" | "footnotemark" => {
          if i < chars.len() && chars[i] == '{' {
            let (_, skip) = read_braced_arg(&chars, i);
            i += skip;
          }
        }
        _ => {
          if i < chars.len() && chars[i] == '{' {
            let (content, skip) = read_braced_arg(&chars, i);
            i += skip;
            out.push_str(&content);
          }
        }
      }
      continue;
    }
    out.push(chars[i]);
    i += 1;
  }
  out.trim().to_string()
}

// ── Accent helpers ────────────────────────────────────────────────────────────

/// Read an accent argument: either `{letter}` or a bare letter.
fn read_accent_arg(text: &[char], start: usize) -> (String, usize) {
  if start >= text.len() {
    return (String::new(), 0);
  }
  if text[start] == '{' {
    read_braced_arg(text, start)
  } else {
    (text[start].to_string(), 1)
  }
}

/// Map a LaTeX accent command + base letter to a Unicode character.
fn accent_char(accent: &str, base: &str) -> Option<char> {
  match (accent, base) {
    // Acute \'
    ("'","a")=>Some('á'),("'","e")=>Some('é'),("'","i")=>Some('í'),
    ("'","o")=>Some('ó'),("'","u")=>Some('ú'),("'","y")=>Some('ý'),
    ("'","A")=>Some('Á'),("'","E")=>Some('É'),("'","I")=>Some('Í'),
    ("'","O")=>Some('Ó'),("'","U")=>Some('Ú'),("'","Y")=>Some('Ý'),
    ("'","n")=>Some('ń'),("'","c")=>Some('ć'),("'","s")=>Some('ś'),
    ("'","z")=>Some('ź'),("'","l")=>Some('ĺ'),
    // Grave \`
    ("`","a")=>Some('à'),("`","e")=>Some('è'),("`","i")=>Some('ì'),
    ("`","o")=>Some('ò'),("`","u")=>Some('ù'),
    ("`","A")=>Some('À'),("`","E")=>Some('È'),("`","I")=>Some('Ì'),
    ("`","O")=>Some('Ò'),("`","U")=>Some('Ù'),
    // Umlaut \"
    ("\"","a")=>Some('ä'),("\"","e")=>Some('ë'),("\"","i")=>Some('ï'),
    ("\"","o")=>Some('ö'),("\"","u")=>Some('ü'),("\"","y")=>Some('ÿ'),
    ("\"","A")=>Some('Ä'),("\"","E")=>Some('Ë'),("\"","I")=>Some('Ï'),
    ("\"","O")=>Some('Ö'),("\"","U")=>Some('Ü'),
    // Circumflex \^
    ("^","a")=>Some('â'),("^","e")=>Some('ê'),("^","i")=>Some('î'),
    ("^","o")=>Some('ô'),("^","u")=>Some('û'),
    ("^","A")=>Some('Â'),("^","E")=>Some('Ê'),("^","I")=>Some('Î'),
    ("^","O")=>Some('Ô'),("^","U")=>Some('Û'),
    // Tilde \~
    ("~","a")=>Some('ã'),("~","n")=>Some('ñ'),("~","o")=>Some('õ'),
    ("~","A")=>Some('Ã'),("~","N")=>Some('Ñ'),("~","O")=>Some('Õ'),
    // Cedilla \c
    ("c","c")=>Some('ç'),("c","C")=>Some('Ç'),
    ("c","s")=>Some('ş'),("c","S")=>Some('Ş'),
    // Double acute \H
    ("H","o")=>Some('ő'),("H","O")=>Some('Ő'),
    ("H","u")=>Some('ű'),("H","U")=>Some('Ű'),
    // Caron \v
    ("v","s")=>Some('š'),("v","S")=>Some('Š'),
    ("v","c")=>Some('č'),("v","C")=>Some('Č'),
    ("v","z")=>Some('ž'),("v","Z")=>Some('Ž'),
    ("v","r")=>Some('ř'),("v","R")=>Some('Ř'),
    ("v","n")=>Some('ň'),("v","N")=>Some('Ň'),
    ("v","e")=>Some('ě'),("v","E")=>Some('Ě'),
    // Ogonek \k
    ("k","a")=>Some('ą'),("k","A")=>Some('Ą'),
    ("k","e")=>Some('ę'),("k","E")=>Some('Ę'),
    // Ring \r
    ("r","a")=>Some('å'),("r","A")=>Some('Å'),
    // Breve \u
    ("u","a")=>Some('ă'),("u","A")=>Some('Ă'),
    ("u","e")=>Some('ĕ'),("u","o")=>Some('ŏ'),("u","u")=>Some('ŭ'),
    // Macron \=
    ("=","a")=>Some('ā'),("=","e")=>Some('ē'),("=","i")=>Some('ī'),
    ("=","o")=>Some('ō'),("=","u")=>Some('ū'),
    ("=","A")=>Some('Ā'),("=","E")=>Some('Ē'),("=","I")=>Some('Ī'),
    ("=","O")=>Some('Ō'),("=","U")=>Some('Ū'),
    _ => None,
  }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn flush_line(line: &mut String, out: &mut Vec<Block>) {
  let trimmed = line.trim().to_string();
  if !trimmed.is_empty() {
    out.push(Block::Line(trimmed));
  }
  line.clear();
}

fn wrap_blocks(blocks: Vec<Block>) -> Vec<Block> {
  let mut out = Vec::new();
  for block in blocks {
    match block {
      Block::Line(s) => {
        for wrapped in textwrap::wrap(&s, WRAP_WIDTH) {
          out.push(Block::Line(wrapped.to_string()));
        }
      }
      other => out.push(other),
    }
  }
  out
}

fn capitalize(s: &str) -> String {
  let mut chars = s.chars();
  match chars.next() {
    None => String::new(),
    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
  }
}

/// Extract `\newcommand` / `\renewcommand` / `\providecommand` definitions.
fn extract_macros(content: &str) -> HashMap<String, (usize, String)> {
  let mut map = HashMap::new();
  let chars: Vec<char> = content.chars().collect();
  let len = chars.len();
  let mut i = 0;
  while i < len {
    if chars[i] != '\\' { i += 1; continue; }
    let (cmd, consumed) = read_command(&chars, i + 1);
    i += 1 + consumed;
    if !matches!(cmd.as_str(), "newcommand" | "renewcommand" | "providecommand") {
      continue;
    }
    if i < len && chars[i] == '*' { i += 1; }
    while i < len && chars[i] == ' ' { i += 1; }

    let name = if i < len && chars[i] == '{' {
      let (raw, skip) = read_braced_arg(&chars, i);
      i += skip;
      raw.trim_start_matches('\\').trim().to_string()
    } else if i < len && chars[i] == '\\' {
      let (n, c2) = read_command(&chars, i + 1);
      i += 1 + c2;
      n
    } else {
      continue;
    };
    if name.is_empty() { continue; }

    while i < len && chars[i] == ' ' { i += 1; }

    let arity = if i < len && chars[i] == '[' {
      i += 1;
      let mut n_str = String::new();
      while i < len && chars[i] != ']' { n_str.push(chars[i]); i += 1; }
      if i < len { i += 1; }
      n_str.trim().parse::<usize>().unwrap_or(0)
    } else {
      0
    };

    while i < len && chars[i] == ' ' { i += 1; }

    // Skip optional default value [default] for 1-arg commands.
    if i < len && chars[i] == '[' {
      while i < len && chars[i] != ']' { i += 1; }
      if i < len { i += 1; }
    }

    if i < len && chars[i] == '{' {
      let (def, skip) = read_braced_arg(&chars, i);
      i += skip;
      map.insert(name, (arity, def));
    }
  }
  map
}

/// Extract the `\caption{...}` text from a raw environment body.
fn extract_caption(body: &str) -> Option<String> {
  extract_command_arg(body, "caption").map(clean_inline)
}

/// Parse a raw tabular body into a `Block::Matrix`.
fn parse_tabular(body: &str) -> Option<Block> {
  let body = body.trim_start();
  let body = if body.starts_with('{') {
    match body.find('}') {
      Some(p) => &body[p + 1..],
      None => body,
    }
  } else {
    body
  };

  let rows: Vec<Vec<String>> = body
    .split(r"\\")
    .map(|row| {
      row.split('&')
        .map(|cell| clean_inline(cell.trim().to_string()))
        .filter(|c| !c.is_empty())
        .collect()
    })
    .filter(|row: &Vec<String>| !row.is_empty())
    .collect();

  if rows.is_empty() { return None; }
  Some(Block::Matrix { rows })
}

// ── Low-level parsers ─────────────────────────────────────────────────────────

fn read_command(text: &[char], start: usize) -> (String, usize) {
  let mut cmd = String::new();
  let mut i = start;
  if i < text.len() && !text[i].is_alphabetic() {
    return (text[i].to_string(), 1);
  }
  while i < text.len() && text[i].is_alphabetic() {
    cmd.push(text[i]);
    i += 1;
  }
  while i < text.len() && text[i] == ' ' {
    i += 1;
  }
  (cmd, i - start)
}

fn read_braced_arg(text: &[char], start: usize) -> (String, usize) {
  if start >= text.len() || text[start] != '{' {
    return (String::new(), 0);
  }
  let mut depth = 0usize;
  let mut content = String::new();
  let mut i = start;
  while i < text.len() {
    match text[i] {
      '{' => { depth += 1; if depth > 1 { content.push('{'); } }
      '}' => {
        depth -= 1;
        if depth == 0 { i += 1; break; }
        content.push('}');
      }
      c => content.push(c),
    }
    i += 1;
  }
  (content, i - start)
}

fn read_until_end(text: &[char], start: usize, env: &str) -> (String, usize) {
  let end_marker: Vec<char> = format!(r"\end{{{env}}}").chars().collect();
  let mut content = String::new();
  let mut i = start;
  while i < text.len() {
    if text[i..].starts_with(&end_marker) {
      i += end_marker.len();
      break;
    }
    content.push(text[i]);
    i += 1;
  }
  (content, i - start)
}

fn read_until_str(text: &[char], start: usize, marker: &str) -> (String, usize) {
  let marker_chars: Vec<char> = marker.chars().collect();
  let mut content = String::new();
  let mut i = start;
  while i < text.len() {
    if text[i..].starts_with(&marker_chars) {
      i += marker_chars.len();
      break;
    }
    content.push(text[i]);
    i += 1;
  }
  (content, i - start)
}

fn read_until_single_dollar(text: &[char], start: usize) -> (String, usize) {
  let mut content = String::new();
  let mut i = start;
  while i < text.len() {
    if text[i] == '$' && (i == 0 || text[i - 1] != '\\') {
      i += 1;
      break;
    }
    content.push(text[i]);
    i += 1;
  }
  (content, i - start)
}

fn read_until_double_dollar(text: &[char], start: usize) -> (String, usize) {
  let mut content = String::new();
  let mut i = start;
  while i + 1 < text.len() {
    if text[i] == '$' && text[i + 1] == '$' {
      i += 2;
      break;
    }
    content.push(text[i]);
    i += 1;
  }
  (content, i - start)
}

fn extract_env(body: &str, env: &str) -> Option<String> {
  let begin = format!(r"\begin{{{env}}}");
  let end = format!(r"\end{{{env}}}");
  let start = body.find(&begin)? + begin.len();
  let finish = body.find(&end)?;
  if start < finish { Some(body[start..finish].to_string()) } else { None }
}

fn extract_command_arg(text: &str, cmd: &str) -> Option<String> {
  let pattern = format!(r"\{cmd}");
  let pos = text.find(&pattern)?;
  let after = text[pos + pattern.len()..].trim_start();
  if !after.starts_with('{') { return None; }
  let chars: Vec<char> = after.chars().collect();
  let (content, _) = read_braced_arg(&chars, 0);
  Some(content)
}

fn clean_inline(s: String) -> String {
  let chars: Vec<char> = s.chars().collect();
  let mut out = String::new();
  let mut i = 0;
  while i < chars.len() {
    if chars[i] == '\\' && i + 1 < chars.len() {
      let (cmd, consumed) = read_command(&chars, i + 1);
      i += 1 + consumed;
      match cmd.as_str() {
        "thanks" | "footnote" | "footnotemark" => {
          if i < chars.len() && chars[i] == '{' {
            let (_, skip) = read_braced_arg(&chars, i);
            i += skip;
          }
        }
        _ => {
          if i < chars.len() && chars[i] == '{' {
            let (content, skip) = read_braced_arg(&chars, i);
            i += skip;
            out.push_str(&content);
          }
        }
      }
    } else if chars[i] == '~' {
      out.push(' ');
      i += 1;
    } else {
      out.push(chars[i]);
      i += 1;
    }
  }
  out.trim().to_string()
}

fn clean_authors(s: String) -> String {
  let s = s.replace(r"\and", ",").replace(r"\\", ",").replace(r"\AND", ",");
  let cleaned = clean_inline(s);
  let mut out = String::new();
  for part in cleaned.split(',') {
    let part = part.trim();
    if part.is_empty() { continue; }
    if !out.is_empty() { out.push_str(", "); }
    out.push_str(part);
  }
  out
}

fn process_prose(text: &str, macros: &HashMap<String, (usize, String)>) -> Vec<String> {
  let mut out = String::new();
  let chars: Vec<char> = text.chars().collect();
  let mut i = 0;
  while i < chars.len() {
    if chars[i] == '%' && (i == 0 || chars[i - 1] != '\\') {
      while i < chars.len() && chars[i] != '\n' { i += 1; }
      continue;
    }
    if chars[i] == '\\' && i + 1 < chars.len() {
      let (cmd, consumed) = read_command(&chars, i + 1);
      i += 1 + consumed;
      match cmd.as_str() {
        "emph" | "textbf" | "textit" | "texttt" => {
          let (content, skip) = read_braced_arg(&chars, i);
          i += skip;
          out.push_str(&content);
        }
        _ => {
          if i < chars.len() && chars[i] == '{' {
            let (content, skip) = read_braced_arg(&chars, i);
            i += skip;
            out.push_str(&content);
          }
        }
      }
      continue;
    }
    if chars[i] == '$' {
      i += 1;
      let (math, adv) = read_until_single_dollar(&chars, i);
      i += adv;
      out.push_str(&render_math(math.trim(), macros));
      continue;
    }
    if chars[i] == '~' {
      out.push(' ');
      i += 1;
      continue;
    }
    out.push(chars[i]);
    i += 1;
  }
  textwrap::wrap(out.trim(), WRAP_WIDTH)
    .into_iter()
    .map(|l| l.to_string())
    .collect()
}
