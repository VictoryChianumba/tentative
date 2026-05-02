pub mod agent;
pub mod ai_query;
pub mod pipeline;
pub mod tools;

use crate::models::FeedItem;

pub enum DiscoveryMessage {
  /// One-line status update shown in the search bar banner.
  StatusUpdate(String),
  /// Batch of discovered papers to merge into the feed.
  Items(Vec<FeedItem>),
  Complete,
  Error(String),
}
