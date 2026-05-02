use std::io::Read;
use std::sync::mpsc::Sender;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::Config;
use crate::discovery::{DiscoveryMessage, tools};

const MAX_ITERATIONS: usize = 8;
const API_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const MAX_BODY_BYTES: u64 = 4 * 1024 * 1024;
const MODEL: &str = "claude-sonnet-4-6";

const SYSTEM: &str = "\
You are a research discovery agent for an AI/ML paper reader called Trench. \
Find the most relevant research papers for the user's query using the available tools.

Guidelines:
- For specific papers or authors: use fetch_arxiv_paper or search_arxiv with precise terms.
- For topic searches: call search_arxiv 2-3 times with different query angles.
- For implementation-focused queries: use search_papers_with_code.
- For recent context or news: use search_web if available.
- Aim for 5-25 relevant papers total. Stop when you have good coverage.
- After finding papers, write a concise 2-3 sentence summary of what you found.";

pub fn run(
  topic: &str,
  config: &Config,
  tx: &Sender<DiscoveryMessage>,
  prior_history: Option<Vec<Value>>,
) {
  if let Err(e) = run_inner(topic, config, tx, prior_history) {
    let _ = tx.send(DiscoveryMessage::Error(e));
  }
}

fn run_inner(
  topic: &str,
  config: &Config,
  tx: &Sender<DiscoveryMessage>,
  prior_history: Option<Vec<Value>>,
) -> Result<(), String> {
  let api_key = config
    .claude_api_key
    .as_deref()
    .filter(|k| !k.trim().is_empty())
    .ok_or_else(|| "No Claude API key configured".to_string())?;

  let tool_defs = tools::all_tool_defs(config);
  let tools_json: Vec<Value> = tool_defs
    .iter()
    .map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.schema }))
    .collect();

  let client = reqwest::blocking::Client::builder()
    .timeout(API_TIMEOUT)
    .build()
    .map_err(|e| e.to_string())?;

  let mut messages: Vec<Value> = match prior_history {
    Some(mut h) => {
      h.push(json!({ "role": "user", "content": topic }));
      h
    }
    None => vec![json!({ "role": "user", "content": topic })],
  };

  let _ = tx.send(DiscoveryMessage::StatusUpdate(format!(
    "Starting discovery for '{topic}'…"
  )));

  for step in 0..MAX_ITERATIONS {
    let response = call_claude(&client, api_key, SYSTEM, &messages, &tools_json)?;

    // Collect tool_use blocks before moving content into messages.
    let tool_uses: Vec<Value> = response
      .content
      .iter()
      .filter(|b| b["type"] == "tool_use")
      .cloned()
      .collect();

    // Always append the assistant turn.
    messages.push(json!({ "role": "assistant", "content": response.content }));

    if tool_uses.is_empty() {
      // No tools called — agent is done.
      emit_snapshot(&messages, tx);
      let _ = tx.send(DiscoveryMessage::Complete);
      return Ok(());
    }

    // Execute tools and collect results for the next user message.
    let mut tool_results = Vec::new();
    for tool_use in &tool_uses {
      let id = tool_use["id"].as_str().unwrap_or("").to_string();
      let name = tool_use["name"].as_str().unwrap_or("").to_string();
      let input = &tool_use["input"];

      let _ = tx.send(DiscoveryMessage::StatusUpdate(format!(
        "Calling {name}… (step {}/{})",
        step + 1,
        MAX_ITERATIONS
      )));

      let result = tools::execute(&name, input, config);

      if !result.items.is_empty() {
        let _ = tx.send(DiscoveryMessage::Items(result.items));
      }

      tool_results.push(json!({
        "type": "tool_result",
        "tool_use_id": id,
        "content": result.text
      }));
    }

    messages.push(json!({ "role": "user", "content": tool_results }));
  }

  emit_snapshot(&messages, tx);
  let _ = tx.send(DiscoveryMessage::Complete);
  Ok(())
}

fn emit_snapshot(messages: &[Value], tx: &Sender<DiscoveryMessage>) {
  let initial_query =
    messages[0]["content"].as_str().unwrap_or("").to_string();
  let mut snapshot = crate::discovery::SessionHistory {
    messages: messages.to_vec(),
    initial_query,
  };
  snapshot.truncate_to_limit();
  let _ = tx.send(DiscoveryMessage::SessionSnapshot(snapshot));
}

#[derive(Deserialize)]
struct ClaudeResponse {
  content: Vec<Value>,
}

fn call_claude(
  client: &reqwest::blocking::Client,
  api_key: &str,
  system: &str,
  messages: &[Value],
  tools: &[Value],
) -> Result<ClaudeResponse, String> {
  let body = json!({
    "model": MODEL,
    "max_tokens": 4096,
    "system": system,
    "tools": tools,
    "messages": messages
  });

  let resp = client
    .post("https://api.anthropic.com/v1/messages")
    .header("x-api-key", api_key)
    .header("anthropic-version", "2023-06-01")
    .header("content-type", "application/json")
    .json(&body)
    .send()
    .map_err(|e| format!("HTTP error: {e}"))?;

  let status = resp.status();
  let text = read_body(resp)?;

  if !status.is_success() {
    return Err(friendly_error(status.as_u16(), &text));
  }

  serde_json::from_str(&text)
    .map_err(|e| format!("Failed to parse Claude response: {e}"))
}

fn read_body(resp: reqwest::blocking::Response) -> Result<String, String> {
  let mut buf = Vec::new();
  resp
    .take(MAX_BODY_BYTES + 1)
    .read_to_end(&mut buf)
    .map_err(|e| e.to_string())?;
  if buf.len() as u64 > MAX_BODY_BYTES {
    return Err("response exceeds 4 MB limit".to_string());
  }
  String::from_utf8(buf).map_err(|e| e.to_string())
}

fn friendly_error(status: u16, body: &str) -> String {
  if let Ok(v) = serde_json::from_str::<Value>(body) {
    if let Some(msg) = v["error"]["message"].as_str() {
      let short = &msg[..msg.len().min(100)];
      return format!("Claude API — {short}");
    }
  }
  format!("Claude API error {status}")
}
