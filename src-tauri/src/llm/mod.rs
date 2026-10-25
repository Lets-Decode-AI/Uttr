//! Text intelligence: AI cleanup of dictations and Command Mode, with an on-device model
//! (llama.cpp) or a cloud provider (OpenAI-compatible or Anthropic).

pub mod cloud;
pub mod local;
pub mod prompt;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::app_state::Settings;
use crate::models::Manager;
use crate::style::{Style, style_for};
pub use local::LocalLlm;
use prompt::Prompt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiProvider {
    /// No AI: only the dictionary, snippets and basic fixes.
    #[default]
    Off,
    /// A GGUF model run by llama.cpp on this computer.
    Local,
    OpenAi,
    Anthropic,
}

/// Dictations shorter than this many words are inserted without AI cleanup: there is little
/// to fix, and waiting for a model would only slow them down.
pub const MIN_CLEANUP_WORDS: usize = 3;

/// Cloud request timeout.
const CLOUD_TIMEOUT: Duration = Duration::from_secs(30);

pub const KEYRING_OPENAI: &str = "openai_api_key";
pub const KEYRING_ANTHROPIC: &str = "anthropic_api_key";

pub const NEEDS_PROVIDER: &str = "Command Mode needs AI. Choose an AI provider on the AI page.";

/// What the dictation controller needs from the AI layer. Calls are blocking.
pub trait TextAi: Send + Sync {
    /// Writing style for the app the text goes to.
    fn style_for(&self, app: Option<&str>) -> Style;
    /// Cleans up `text`. `Ok(None)`: AI cleanup is off. `keep` lists words to spell as given.
    fn clean(&self, text: &str, style: Style, keep: &[String]) -> Result<Option<String>, String>;
    /// Command Mode: applies the spoken `instruction` to `selection`, or writes new text.
    fn command(&self, instruction: &str, selection: Option<&str>) -> Result<String, String>;
    /// `Err` with a message for the user when Command Mode cannot run.
    fn command_ready(&self) -> Result<(), String>;
}

/// No AI at all (tests, and the fallback before the AI layer exists).
pub struct NoAi;

impl TextAi for NoAi {
    fn style_for(&self, _app: Option<&str>) -> Style {
        Style::Formal
    }
    fn clean(&self, _: &str, _: Style, _: &[String]) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn command(&self, _: &str, _: Option<&str>) -> Result<String, String> {
        Err(NEEDS_PROVIDER.into())
    }
    fn command_ready(&self) -> Result<(), String> {
        Err(NEEDS_PROVIDER.into())
    }
}

/// API keys from the system keychain, cached in memory.
#[derive(Clone, Default)]
pub struct Keys(Arc<Mutex<HashMap<AiProvider, String>>>);

impl Keys {
    pub fn get(&self, p: AiProvider) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&p)
            .cloned()
    }
    pub fn set(&self, p: AiProvider, key: Option<String>) {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match key.filter(|k| !k.is_empty()) {
            Some(k) => m.insert(p, k),
            None => m.remove(&p),
        };
    }
}

pub fn keyring_user(p: AiProvider) -> Option<&'static str> {
    match p {
        AiProvider::OpenAi => Some(KEYRING_OPENAI),
        AiProvider::Anthropic => Some(KEYRING_ANTHROPIC),
        _ => None,
    }
}

/// The real AI layer, configured from the live settings.
pub struct Ai {
    settings: Arc<Mutex<Settings>>,
    local: LocalLlm,
    models: Manager,
    keys: Keys,
    http: reqwest::Client,
    anthropic_base: String,
}

impl Ai {
    pub fn new(settings: Arc<Mutex<Settings>>, local: LocalLlm, models: Manager, keys: Keys) -> Ai {
        Ai {
            settings,
            local,
            models,
            keys,
            http: reqwest::Client::builder()
                .timeout(CLOUD_TIMEOUT)
                .build()
                .unwrap_or_default(),
            anthropic_base: cloud::ANTHROPIC_URL.into(),
        }
    }

    fn settings(&self) -> Settings {
        self.settings
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Why the configured provider cannot run, if it cannot.
    fn not_ready(&self, s: &Settings) -> Option<String> {
        match s.ai_provider {
            AiProvider::Off => Some("AI is turned off.".into()),
            AiProvider::Local => self
                .models
                .ensure_active()
                .is_none()
                .then(|| "Download an on-device AI model on the AI page first.".into()),
            AiProvider::OpenAi => (self.keys.get(AiProvider::OpenAi).is_none()
                && s.openai_base_url.trim_end_matches('/') == cloud::OPENAI_URL)
                .then(|| "Add your OpenAI API key on the AI page.".into()),
            AiProvider::Anthropic => self
                .keys
                .get(AiProvider::Anthropic)
                .is_none()
                .then(|| "Add your Anthropic API key on the AI page.".into()),
        }
    }

    /// Blocking: runs `prompt` on the configured provider. Must not run on an async thread.
    pub fn complete(&self, prompt: Prompt) -> Result<String, String> {
        let s = self.settings();
        if let Some(why) = self.not_ready(&s) {
            return Err(why);
        }
        let out = match s.ai_provider {
            AiProvider::Off => unreachable!("checked by not_ready"),
            AiProvider::Local => {
                let m = self
                    .models
                    .ensure_active()
                    .ok_or("No on-device AI model.")?;
                let file = m
                    .entry
                    .files
                    .iter()
                    .find(|f| f.path.ends_with(".gguf"))
                    .ok_or("The AI model has no GGUF file.")?;
                self.local.complete(&m.dir.join(&file.path), prompt)?
            }
            AiProvider::OpenAi => {
                let key = self.keys.get(AiProvider::OpenAi);
                tauri::async_runtime::block_on(cloud::openai(
                    &self.http,
                    &s.openai_base_url,
                    key.as_deref(),
                    &s.openai_model,
                    &prompt,
                ))?
            }
            AiProvider::Anthropic => {
                let key = self.keys.get(AiProvider::Anthropic).unwrap_or_default();
                tauri::async_runtime::block_on(cloud::anthropic(
                    &self.http,
                    &self.anthropic_base,
                    &key,
                    &s.anthropic_model,
                    &prompt,
                ))?
            }
        };
        Ok(prompt::tidy(&out))
    }
}

impl TextAi for Ai {
    fn style_for(&self, app: Option<&str>) -> Style {
        let s = self.settings();
        style_for(app, &s.app_styles, s.default_style)
    }

    fn clean(&self, text: &str, style: Style, keep: &[String]) -> Result<Option<String>, String> {
        if self.settings().ai_provider == AiProvider::Off
            || text.split_whitespace().count() < MIN_CLEANUP_WORDS
        {
            return Ok(None);
        }
        let out = self.complete(prompt::cleanup(text, style, keep))?;
        if prompt::plausible_cleanup(text, &out) {
            Ok(Some(out))
        } else {
            Err(format!(
                "the cleanup did not look like the dictation: {out:?}"
            ))
        }
    }

    fn command(&self, instruction: &str, selection: Option<&str>) -> Result<String, String> {
        let out = self.complete(prompt::command(instruction, selection))?;
        if out.trim().is_empty() {
            return Err("The AI sent back no text.".into());
        }
        Ok(out)
    }

    fn command_ready(&self) -> Result<(), String> {
        let s = self.settings();
        match s.ai_provider {
            AiProvider::Off => Err(NEEDS_PROVIDER.into()),
            _ => self.not_ready(&s).map_or(Ok(()), Err),
        }
    }
}
