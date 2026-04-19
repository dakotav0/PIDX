use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::models::profile::ProfileDocument;

// ── ProfileStore ──────────────────────────────────────────────────────────────

/// File-per-user JSON persistence for PIDX profiles.
///
/// Each profile lives at `{profiles_dir}/{safe_user_id}.pidx.json`.
/// The store does not hold any in-memory cache — every call reads from or
/// writes to disk directly. Caching can be layered on top later.
///
/// ## Rust lesson: PathBuf
///
/// `PathBuf` is the owned, heap-allocated path type — Python's `pathlib.Path`
/// equivalent. It's separate from `&Path` (the borrowed slice) the same way
/// `String` is separate from `&str`. `impl Into<PathBuf>` on `new()` lets you
/// pass either a `&str` or a `PathBuf` without extra conversion at the call site.
pub struct ProfileStore {
    profiles_dir: PathBuf,
}

impl ProfileStore {
    /// Construct a store rooted at the given directory.
    ///
    /// ```rust,ignore
    /// let store = ProfileStore::new("profiles");          // &str works
    /// let store = ProfileStore::new(PathBuf::from("…")); // PathBuf works too
    /// ```
    pub fn new(profiles_dir: impl Into<PathBuf>) -> Self {
        Self {
            profiles_dir: profiles_dir.into(),
        }
    }

    /// Return the directory this store is rooted at.
    pub fn dir(&self) -> &std::path::Path {
        &self.profiles_dir
    }

    /// Resolve the default profiles directory.
    ///
    /// Priority: `PIDX_PROFILES_DIR` env var → platform data dir.
    /// - Windows:  `%APPDATA%\pidx\profiles`
    /// - Linux/WSL: `~/.local/share/pidx/profiles`
    /// - macOS:    `~/Library/Application Support/pidx/profiles`
    pub fn default_dir() -> PathBuf {
        std::env::var("PIDX_PROFILES_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("pidx")
                    .join("profiles")
            })
    }

    /// Resolve the default mailbox directory (watched for incoming `.bridge.json` drops).
    ///
    /// Priority: `PIDX_MAILBOX_DIR` env var → platform data dir.
    /// - Windows:  `%APPDATA%\pidx\mailbox`
    /// - Linux/WSL: `~/.local/share/pidx/mailbox`
    /// - macOS:    `~/Library/Application Support/pidx/mailbox`
    pub fn default_mailbox_dir() -> PathBuf {
        std::env::var("PIDX_MAILBOX_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("pidx")
                    .join("mailbox")
            })
    }

    /// Load a profile by user id.
    ///
    /// Returns `Ok(None)` if the file does not exist. Any other I/O or parse
    /// error is returned as `Err` with the file path included in the message
    /// (via `anyhow::Context`).
    pub fn load(&self, user_id: &str) -> Result<Option<ProfileDocument>> {
        let path = self.path_for(user_id);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading profile at {}", path.display()))?;
        let doc = serde_json::from_str(&content)
            .with_context(|| format!("parsing profile at {}", path.display()))?;
        Ok(Some(doc))
    }

    /// Load if the file exists; otherwise return a fresh blank profile.
    ///
    /// The fresh profile is **not** written to disk — call `save()` explicitly
    /// when you're ready to persist it. This matches the Python pattern of
    /// creating the profile object in memory and only saving on confirmed changes.
    pub fn load_or_create(&self, user_id: &str) -> Result<ProfileDocument> {
        match self.load(user_id)? {
            Some(doc) => Ok(doc),
            None => Ok(ProfileDocument::new(user_id)),
        }
    }

    /// Persist a profile to disk.
    ///
    /// Refreshes `meta.updated` to the current UTC time before writing.
    /// Creates the profiles directory if it doesn't already exist.
    /// Output is pretty-printed JSON (2-space indent via serde_json).
    pub fn save(&self, profile: &mut ProfileDocument) -> Result<()> {
        profile.meta.updated = Utc::now().to_rfc3339();

        std::fs::create_dir_all(&self.profiles_dir).with_context(|| {
            format!(
                "creating profiles directory {}",
                self.profiles_dir.display()
            )
        })?;

        let path = self.path_for(&profile.meta.id);
        let json = serde_json::to_string_pretty(profile).context("serializing profile to JSON")?;

        std::fs::write(&path, json)
            .with_context(|| format!("writing profile to {}", path.display()))?;

        Ok(())
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn path_for(&self, user_id: &str) -> PathBuf {
        self.profiles_dir
            .join(format!("{}.pidx.json", safe_id(user_id)))
    }
}

/// Sanitize a user id for safe use as a filename.
///
/// Keeps only alphanumeric characters, underscores, and hyphens. Truncates to
/// 64 characters. An empty result after stripping is a logic error in the
/// caller — upstream validation should reject it before reaching here.
///
/// Mirrors `_safe_id()` in `services/storage.py` without needing a regex crate.
fn safe_id(user_id: &str) -> String {
    user_id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .take(64)
        .collect()
}
