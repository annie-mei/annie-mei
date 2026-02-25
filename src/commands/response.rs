//! Transport-agnostic command response types.
//!
//! This module defines [`CommandResponse`], which represents the *intent* of a
//! command handler without coupling to Discord transport (defer / respond / edit).
//!
//! ## Adding a new command (testable pattern)
//!
//! 1. Write a **core handler** function that accepts plain data and returns
//!    `CommandResponse`. This function must not touch `Context` or
//!    `CommandInteraction`.
//! 2. Write a thin **adapter** (`run`) that extracts arguments from the
//!    interaction, calls the core handler, and maps `CommandResponse` to the
//!    appropriate Serenity calls.
//! 3. Write tests against the core handler — no Discord token needed.

use serenity::all::CreateEmbed;

/// The intended response from a slash-command handler.
///
/// Adapters inspect this value and translate it into the correct Serenity call
/// (immediate reply, deferred edit, etc.).
#[derive(Debug)]
pub enum CommandResponse {
    /// A plain-text immediate reply (no defer required).
    ///
    /// Used by lightweight commands like `/ping`.
    Message(String),

    /// A plain-text deferred reply (adapter should defer first, then edit).
    ///
    /// Used when the command did async work but the result is just text
    /// (e.g. "No such anime").
    Content(String),

    /// An embed deferred reply (adapter should defer first, then edit).
    ///
    /// Used when the command produces a rich embed (e.g. `/anime` success).
    ///
    /// Boxed to avoid a large size difference between enum variants.
    Embed(Box<CreateEmbed>),
}

#[cfg(test)]
impl CommandResponse {
    /// Returns `true` if this is a [`CommandResponse::Message`].
    pub fn is_message(&self) -> bool {
        matches!(self, Self::Message(_))
    }

    /// Returns `true` if this is a [`CommandResponse::Content`].
    pub fn is_content(&self) -> bool {
        matches!(self, Self::Content(_))
    }

    /// Returns `true` if this is a [`CommandResponse::Embed`].
    pub fn is_embed(&self) -> bool {
        matches!(self, Self::Embed(_))
    }

    /// Unwraps the inner text of a [`CommandResponse::Message`].
    ///
    /// # Panics
    ///
    /// Panics if the variant is not `Message`.
    pub fn unwrap_message(self) -> String {
        match self {
            Self::Message(text) => text,
            other => panic!("expected Message, got {other:?}"),
        }
    }

    /// Unwraps the inner text of a [`CommandResponse::Content`].
    ///
    /// # Panics
    ///
    /// Panics if the variant is not `Content`.
    pub fn unwrap_content(self) -> String {
        match self {
            Self::Content(text) => text,
            other => panic!("expected Content, got {other:?}"),
        }
    }

    /// Unwraps the inner embed of a [`CommandResponse::Embed`].
    ///
    /// # Panics
    ///
    /// Panics if the variant is not `Embed`.
    pub fn unwrap_embed(self) -> CreateEmbed {
        match self {
            Self::Embed(embed) => *embed,
            other => panic!("expected Embed, got {other:?}"),
        }
    }
}
