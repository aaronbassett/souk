//! AI-powered review support.
//!
//! This module provides a provider-agnostic abstraction for sending prompts
//! to frontier LLM APIs (Anthropic, OpenAI, Gemini) and receiving review
//! text. See [`provider::detect_provider`] for automatic API key detection.

pub mod marketplace;
pub mod plugin;
pub mod provider;
pub mod skill;

pub use plugin::{review_plugin, ReviewReport};
pub use provider::{
    detect_provider, AnthropicProvider, GeminiProvider, LlmProvider, MockProvider, OpenAiProvider,
};
pub use skill::{review_skills, SkillReviewReport};
