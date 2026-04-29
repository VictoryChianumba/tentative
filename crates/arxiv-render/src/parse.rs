use doc_model::Block;
use math_render::{MathInput, render};
use std::collections::HashMap;

const WRAP_WIDTH: usize = 80;

/// Convert a set of `.tex` source files into a semantic block document.
pub fn to_blocks(sources: Vec<(String, String)>) -> Vec<Block> {
  let file_map: HashMap<String, String> = sources.into_iter().collect();
  let root = find_root(&file_map);
  let expanded = expand_inputs(&root, &file_map, 0);

  // Extract title/authors from the full source before stripping preamble.
  let title = extract_command_arg(&expanded, "title").map(clean_inline);
  let authors = extract_command_arg(&expanded, "author").map(clean_authors);

  let body = extract_body(&expanded);
  let body_blocks = process(&body);

  // Prepend title / authors so they lead the document.
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
  out
}

// ── Root selection ────────────────────────────────────────────────────────────

fn find_root(files: &HashMap<String, String>) -> String {
  for content in files.values() {
    if content.contains(r"\begin{document}") {
      return content.clone();
    }
  }
  files
    .values()
    .max_by_key(|c| c.len())
    .cloned()
    .unwrap_or_default()
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

fn process(body: &str) -> Vec<Block> {
  let mut out: Vec<Block> = Vec::new();

  // Abstract block first so it appears at the top.
  if let Some(abs) = extract_env(body, "abstract") {
    out.push(Block::Blank);
    out.push(Block::Header { level: 2, text: "Abstract".to_string() });
    for line in process_prose(&abs) {
      out.push(Block::Line(line));
    }
    out.push(Block::Blank);
  }

  let processed = process_body(body);
  out.extend(processed);
  out
}

// ── Body state machine ────────────────────────────────────────────────────────

fn process_body(body: &str) -> Vec<Block> {
  let mut out: Vec<Block> = Vec::new();
  let mut current_line = String::new();

  let skip_envs = [
    "figure", "figure*", "table", "table*", "lstlisting", "verbatim",
    "tikzpicture", "algorithm", "algorithmic", "wrapfigure", "subfigure",
    "minipage", "thebibliography",
    "tabular", "tabular*", "longtable", "tabularx", "tabulary",
  ];
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
      while i < len && text[i] != '\n' {
        i += 1;
      }
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
          if skip_envs.contains(&env.trim()) {
            flush_line(&mut current_line, &mut out);
            out.push(Block::Line(format!("[{env}]")));
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
            let rendered = render(MathInput::Latex(math.trim()));
            let lines: Vec<String> = rendered.lines().map(|l| l.to_string()).collect();
            if !lines.is_empty() {
              out.push(Block::DisplayMath(lines));
            }
            continue;
          }

          if env == "abstract" {
            let (_abs, adv) = read_until_end(&text, i, "abstract");
            i += adv;
            continue;
          }

          if skip_envs.contains(&env.as_str()) {
            let (_content, adv) = read_until_end(&text, i, &env);
            i += adv;
            continue;
          }

          continue;
        }

        // Section headers → semantic Header blocks.
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

        // Inline text commands — keep the argument, drop the command.
        "emph" | "textbf" | "textit" | "texttt" | "text" | "mathrm"
        | "mathbf" | "mathit" | "mathcal" | "mathbb"
        | "textsubscript" | "textsuperscript" | "textnormal"
        | "underline" | "overline" | "uline" => {
          let (content, skip) = read_braced_arg(&text, i);
          i += skip;
          current_line.push_str(&content);
        }

        // Two-arg formatting commands: skip first arg (option/color), keep second.
        "textcolor" | "colorbox" | "fbox" | "mbox" | "makebox" => {
          let (_opt, skip1) = read_braced_arg(&text, i);
          i += skip1;
          let (content, skip2) = read_braced_arg(&text, i);
          i += skip2;
          current_line.push_str(&content);
        }

        // Commands to completely discard (consume all following [] and {} args).
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
          // Optional note arg e.g. \citep[p.~3]{key}
          if i < len && text[i] == '[' {
            while i < len && text[i] != ']' { i += 1; }
            if i < len { i += 1; }
          }
          let (_key, skip) = read_braced_arg(&text, i);
          i += skip;
          if cmd != "nocite" {
            current_line.push_str("[ref]");
          }
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
          // Skip optional mark arg e.g. \footnotetext[2]{...}
          if i < len && text[i] == '[' {
            while i < len && text[i] != ']' { i += 1; }
            if i < len { i += 1; }
          }
          let (note, skip) = read_braced_arg(&text, i);
          i += skip;
          current_line.push_str(" [note: ");
          current_line.push_str(note.trim());
          current_line.push(']');
        }
        // \hyperref[label]{display text}
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
        // List items — bullet + flush.
        "item" => {
          // Skip optional [label] for description lists.
          if i < len && text[i] == '[' {
            while i < len && text[i] != ']' { i += 1; }
            if i < len { i += 1; }
          }
          flush_line(&mut current_line, &mut out);
          current_line.push_str("• ");
        }

        // Inline math \( ... \)
        "(" => {
          let (math, adv) = read_until_str(&text, i, r"\)");
          i += adv;
          let rendered = render(MathInput::Latex(math.trim()));
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
          let rendered = render(MathInput::Latex(math.trim()));
          let lines: Vec<String> = rendered.lines().map(|l| l.to_string()).collect();
          if !lines.is_empty() {
            out.push(Block::DisplayMath(lines));
          }
        }

        "\\" | "newline" | "hline" => {
          flush_line(&mut current_line, &mut out);
        }
        "par" | "medskip" | "bigskip" | "smallskip" | "vspace" | "vskip" => {
          let _ = if cmd == "vspace" || cmd == "vskip" {
            let (_arg, skip) = read_braced_arg(&text, i);
            i += skip;
          };
          flush_line(&mut current_line, &mut out);
          out.push(Block::Blank);
        }

        _ => {
          if i < len && text[i] == '{' {
            let (content, skip) = read_braced_arg(&text, i);
            i += skip;
            // Only output if it looks like prose (has spaces or punctuation).
            // Single-word args are usually identifiers/style names — discard.
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
        let rendered = render(MathInput::Latex(math.trim()));
        let lines: Vec<String> = rendered.lines().map(|l| l.to_string()).collect();
        if !lines.is_empty() {
          out.push(Block::DisplayMath(lines));
        }
      } else {
        i += 1;
        let (math, adv) = read_until_single_dollar(&text, i);
        i += adv;
        let rendered = render(MathInput::Latex(math.trim()));
        if rendered.contains('\n') {
          // Multi-line inline math (fractions etc.) — promote to display block.
          flush_line(&mut current_line, &mut out);
          let lines: Vec<String> = rendered.lines().map(|l| l.to_string()).collect();
          out.push(Block::DisplayMath(lines));
        } else {
          current_line.push_str(&rendered);
        }
      }
      continue;
    }

    // Newline in source — collapse multiple blanks.
    if c == '\n' {
      if i + 1 < len && text[i + 1] == '\n' {
        flush_line(&mut current_line, &mut out);
        out.push(Block::Blank);
        while i < len && text[i] == '\n' {
          i += 1;
        }
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
      // DisplayMath, Header, Matrix, Blank pass through unchanged.
      other => out.push(other),
    }
  }
  out
}

/// Read a LaTeX command name starting at position `start` in `text`.
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

/// Read `{...}` argument at position `start`. Returns (content, chars_consumed).
fn read_braced_arg(text: &[char], start: usize) -> (String, usize) {
  if start >= text.len() || text[start] != '{' {
    return (String::new(), 0);
  }
  let mut depth = 0usize;
  let mut content = String::new();
  let mut i = start;
  while i < text.len() {
    match text[i] {
      '{' => {
        depth += 1;
        if depth > 1 { content.push('{'); }
      }
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

/// Read content until `\end{env_name}`. Returns (content, chars_consumed).
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

/// Read content until a string marker. Returns (content, chars_consumed).
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

/// Read inline math until the next unescaped `$`.
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

/// Read display math until `$$`.
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

/// Extract the content of a named environment (used for abstract).
fn extract_env(body: &str, env: &str) -> Option<String> {
  let begin = format!(r"\begin{{{env}}}");
  let end = format!(r"\end{{{env}}}");
  let start = body.find(&begin)? + begin.len();
  let finish = body.find(&end)?;
  if start < finish { Some(body[start..finish].to_string()) } else { None }
}

/// Find `\cmd{...}` anywhere in `text` and return the braced content.
fn extract_command_arg(text: &str, cmd: &str) -> Option<String> {
  let pattern = format!(r"\{cmd}");
  let pos = text.find(&pattern)?;
  let after = text[pos + pattern.len()..].trim_start();
  if !after.starts_with('{') {
    return None;
  }
  let chars: Vec<char> = after.chars().collect();
  let (content, _) = read_braced_arg(&chars, 0);
  Some(content)
}

/// Strip LaTeX commands from inline text; keep argument content.
fn clean_inline(s: String) -> String {
  let chars: Vec<char> = s.chars().collect();
  let mut out = String::new();
  let mut i = 0;
  while i < chars.len() {
    if chars[i] == '\\' && i + 1 < chars.len() {
      let (cmd, consumed) = read_command(&chars, i + 1);
      i += 1 + consumed;
      match cmd.as_str() {
        // Discard footnote-style args completely.
        "thanks" | "footnote" | "footnotemark" => {
          if i < chars.len() && chars[i] == '{' {
            let (_, skip) = read_braced_arg(&chars, i);
            i += skip;
          }
        }
        // Keep content of common text commands.
        _ => {
          if i < chars.len() && chars[i] == '{' {
            let (content, skip) = read_braced_arg(&chars, i);
            i += skip;
            out.push_str(&content);
          }
        }
      }
    } else {
      out.push(chars[i]);
      i += 1;
    }
  }
  out.trim().to_string()
}

/// Clean an \author{...} value: collapse \and, \\, \inst etc. into a single line.
fn clean_authors(s: String) -> String {
  let s = s.replace(r"\and", ",").replace(r"\\", ",").replace(r"\AND", ",");
  // Strip everything else via clean_inline.
  let cleaned = clean_inline(s);
  // Collapse runs of commas/whitespace.
  let mut out = String::new();
  let mut last_comma = false;
  for part in cleaned.split(',') {
    let part = part.trim();
    if part.is_empty() {
      continue;
    }
    if !out.is_empty() {
      out.push_str(", ");
    }
    out.push_str(part);
    last_comma = false;
  }
  let _ = last_comma;
  out
}

/// Process prose text for the abstract — strips simple commands, wraps.
fn process_prose(text: &str) -> Vec<String> {
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
      out.push_str(&render(MathInput::Latex(math.trim())));
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
