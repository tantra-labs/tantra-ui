//! Chat transcript and message models for the Ratatui agent tab.

use serde::{Deserialize, Serialize};
use tantra_highlight::{HighlightSpan, Language, highlight_code};

/// The role of the speaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// The user asking questions.
    User,
    /// The agent answering.
    Agent,
    /// System notices.
    System,
    /// A tool-call status row.
    Tool,
}

/// Lifecycle state of a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    /// Still executing.
    Running,
    /// Finished successfully.
    Ok,
    /// Failed or was denied.
    Error,
}

/// A tool-call row shown in the transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRow {
    /// Tool name.
    pub name: String,
    /// Short human summary of the call.
    pub summary: String,
    /// Current status.
    pub status: ToolStatus,
    /// Elapsed milliseconds once finished.
    pub elapsed_ms: Option<u64>,
}

/// Update message emitted by the background agent turn task.
#[derive(Debug)]
pub enum AgentStreamUpdate {
    /// Token text fragment.
    Token(String),
    /// A tool call started.
    ToolStarted {
        /// Call id.
        call_id: String,
        /// Tool name.
        name: String,
        /// Short summary.
        summary: String,
    },
    /// A tool call finished.
    ToolFinished {
        /// Call id.
        call_id: String,
        /// Tool name.
        name: String,
        /// Whether the tool succeeded.
        ok: bool,
        /// Elapsed milliseconds.
        elapsed_ms: u64,
        /// Typed result blocks.
        blocks: Vec<tantra_agent_core::DisplayBlock>,
    },
    /// Turn completed normally.
    Finished,
    /// An error occurred during generation.
    Error(String),
}

/// A single message in the chat history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Who sent this message.
    pub role: AgentRole,
    /// The text content of the message.
    pub text: String,
    /// Tool-call details when `role == Tool`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<ToolRow>,
}

impl AgentMessage {
    /// Construct a new message.
    pub fn new(role: AgentRole, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            tool: None,
        }
    }

    /// Construct a tool-call row message.
    pub fn tool_row(row: ToolRow) -> Self {
        Self {
            role: AgentRole::Tool,
            text: row.name.clone(),
            tool: Some(row),
        }
    }

    /// Highlight this message with Markdown semantics.
    pub fn highlight(&self) -> Vec<HighlightSpan> {
        highlight_code(Language::Markdown, &self.text)
    }
}

/// The chat transcript (ordered messages).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatTranscript {
    /// Messages in display order (oldest first).
    pub messages: Vec<AgentMessage>,
}

impl ChatTranscript {
    /// Create an empty transcript.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Append a message.
    pub fn push(&mut self, message: AgentMessage) {
        self.messages.push(message);
    }

    /// Clear all messages from transcript.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// The number of messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the transcript is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// The last message, if any.
    pub fn last(&self) -> Option<&AgentMessage> {
        self.messages.last()
    }

    /// The last message as mutable, if any.
    pub fn last_mut(&mut self) -> Option<&mut AgentMessage> {
        self.messages.last_mut()
    }
}

impl Default for ChatTranscript {
    fn default() -> Self {
        Self::new()
    }
}
