use chat::ChatSlashCommandSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
  Discovery,
  Sources,
  Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
  BuiltIn,
  Workflow,
  Stub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
  ClearChat,
  Discover,
  ClearDiscoveries,
  AddArxivCategory,
  AddFeed,
  Sota,
  ReadingList,
  Code,
  Compare,
  Digest,
  Author,
  Trending,
  Watch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
  pub id: CommandId,
  pub command: &'static str,
  pub completion: &'static str,
  pub description: &'static str,
  pub category: CommandCategory,
  pub kind: CommandKind,
}

pub const COMMAND_SPECS: &[CommandSpec] = &[
  CommandSpec {
    id: CommandId::ClearChat,
    command: "/clear",
    completion: "/clear",
    description: "Clear the current chat session view",
    category: CommandCategory::Discovery,
    kind: CommandKind::BuiltIn,
  },
  CommandSpec {
    id: CommandId::Discover,
    command: "/discover",
    completion: "/discover ",
    description: "Find papers and sources for a topic",
    category: CommandCategory::Discovery,
    kind: CommandKind::Workflow,
  },
  CommandSpec {
    id: CommandId::ClearDiscoveries,
    command: "/clear discoveries",
    completion: "/clear discoveries",
    description: "Clear the discovery feed",
    category: CommandCategory::Discovery,
    kind: CommandKind::BuiltIn,
  },
  CommandSpec {
    id: CommandId::AddArxivCategory,
    command: "/add",
    completion: "/add ",
    description: "Add an arXiv category permanently",
    category: CommandCategory::Sources,
    kind: CommandKind::BuiltIn,
  },
  CommandSpec {
    id: CommandId::AddFeed,
    command: "/add-feed",
    completion: "/add-feed ",
    description: "Add an RSS feed permanently",
    category: CommandCategory::Sources,
    kind: CommandKind::BuiltIn,
  },
  CommandSpec {
    id: CommandId::Sota,
    command: "/sota",
    completion: "/sota ",
    description: "State-of-the-art results and benchmark comparison for a topic",
    category: CommandCategory::Discovery,
    kind: CommandKind::Workflow,
  },
  CommandSpec {
    id: CommandId::ReadingList,
    command: "/reading-list",
    completion: "/reading-list ",
    description: "Ordered learning path for a topic",
    category: CommandCategory::Discovery,
    kind: CommandKind::Workflow,
  },
  CommandSpec {
    id: CommandId::Code,
    command: "/code",
    completion: "/code ",
    description: "Find implementations and code for a topic",
    category: CommandCategory::Discovery,
    kind: CommandKind::Workflow,
  },
  CommandSpec {
    id: CommandId::Compare,
    command: "/compare",
    completion: "/compare ",
    description: "Side-by-side comparison of two approaches or models",
    category: CommandCategory::Discovery,
    kind: CommandKind::Workflow,
  },
  CommandSpec {
    id: CommandId::Digest,
    command: "/digest",
    completion: "/digest",
    description: "What happened in AI/ML this week",
    category: CommandCategory::Discovery,
    kind: CommandKind::Workflow,
  },
  CommandSpec {
    id: CommandId::Author,
    command: "/author",
    completion: "/author ",
    description: "Find all papers by a specific researcher",
    category: CommandCategory::Discovery,
    kind: CommandKind::Workflow,
  },
  CommandSpec {
    id: CommandId::Trending,
    command: "/trending",
    completion: "/trending ",
    description: "Find trending papers on a topic",
    category: CommandCategory::Discovery,
    kind: CommandKind::Workflow,
  },
  CommandSpec {
    id: CommandId::Watch,
    command: "/watch",
    completion: "/watch ",
    description: "Planned: watch a topic over time",
    category: CommandCategory::Planned,
    kind: CommandKind::Stub,
  },
];

pub fn chat_slash_specs() -> Vec<ChatSlashCommandSpec> {
  COMMAND_SPECS
    .iter()
    .map(|spec| ChatSlashCommandSpec {
      command: spec.command.to_string(),
      completion: spec.completion.to_string(),
      description: spec.description.to_string(),
    })
    .collect()
}
