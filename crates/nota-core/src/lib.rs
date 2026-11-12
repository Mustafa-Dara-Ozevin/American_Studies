pub mod config;
pub mod frontmatter;
pub mod parser;
pub mod slug;
pub mod typing_rules;
pub mod vault;

pub use config::{LogicConfig, NotaConfig, SiteConfig, SrsConfig, VaultConfig};
pub use frontmatter::{parse_frontmatter, NoteFrontmatter};
pub use parser::{
    parse_epub_link_target, parse_note_body, parse_wikilink_inner, split_table_cells,
    EpubLinkTarget, ExternalLink, FencedBlock, HeadingInfo, HighlightIssue, MarkdownTable,
    ParsedNoteBody, TaskItem, WikiLink,
};
pub use slug::{normalize_lookup_key, slug_anchor, sluggify_path, sluggify_segment, slugify_file_path};
pub use typing_rules::{TypingRule, TypingRuleEngine};
pub use vault::{
    clean_note_metadata_and_highlights, GraphEdge, GraphNode, IndexedNote, ResolvedLink,
    VaultGraph, VaultIndex,
};
