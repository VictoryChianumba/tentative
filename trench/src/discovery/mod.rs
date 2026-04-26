pub mod ai_query;
pub mod pipeline;

use crate::models::FeedItem;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveryPlan {
  #[serde(default)]
  pub topic: String,
  #[serde(default)]
  pub arxiv_categories: Vec<String>,
  #[serde(default)]
  pub paper_ids: Vec<String>,
  #[serde(default)]
  pub rss_urls: Vec<DiscoveredRssFeed>,
  #[serde(default)]
  pub github_sources: Vec<DiscoveredSource>,
  #[serde(default)]
  pub huggingface_sources: Vec<DiscoveredSource>,
  #[serde(default)]
  pub search_terms: Vec<String>,
  #[serde(default)]
  pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredRssFeed {
  pub url: String,
  pub name: String,
  #[serde(default)]
  pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredSource {
  pub url: String,
  #[serde(default)]
  pub kind: String,
  #[serde(default)]
  pub reason: String,
}

pub enum DiscoveryMessage {
  PlanReady(DiscoveryPlan),
  Items(Vec<FeedItem>),
  Complete,
  Error(String),
}

pub fn format_plan_message(plan: &DiscoveryPlan) -> String {
  let cats = if plan.arxiv_categories.is_empty() {
    "none".to_string()
  } else {
    plan.arxiv_categories.join(" · ")
  };
  let terms = if plan.search_terms.is_empty() {
    "none".to_string()
  } else {
    plan.search_terms.join(" · ")
  };

  let mut lines = vec![
    format!("Discovery: \"{}\"", plan.topic),
    String::new(),
    format!("arXiv categories:  {cats}"),
    format!("Search terms:      {terms}"),
    format!("Papers targeted:   {} specific IDs", plan.paper_ids.len()),
  ];

  lines.push(String::new());
  lines.push("Suggested sources:".to_string());
  if plan.arxiv_categories.is_empty()
    && plan.rss_urls.is_empty()
    && plan.github_sources.is_empty()
    && plan.huggingface_sources.is_empty()
  {
    lines.push("  none".to_string());
  }
  for cat in &plan.arxiv_categories {
    lines.push(format!("  /add {cat}"));
  }
  for feed in &plan.rss_urls {
    if ai_query::is_http_url(&feed.url) {
      lines.push(format!("  /add-feed {}", feed.url));
    }
  }
  for source in &plan.github_sources {
    if ai_query::is_http_url(&source.url) {
      lines.push(format!("  [ ] GitHub {} {}", source.kind, source.url));
    }
  }
  for source in &plan.huggingface_sources {
    if ai_query::is_http_url(&source.url) {
      lines.push(format!("  [ ] HuggingFace {} {}", source.kind, source.url));
    }
  }

  lines.extend([
    String::new(),
    "To add sources permanently:".to_string(),
    "  /add cs.LG".to_string(),
    "  /add-feed URL".to_string(),
    "  /clear discoveries".to_string(),
  ]);

  lines.join("\n")
}
