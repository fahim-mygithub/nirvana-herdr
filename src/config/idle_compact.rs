//! Nirvana fork, plan 27 §3: `[ui.idle_compact]`, the switch and the two numbers for an idle Claude's `/compact`
//! (`nirvana_idle_compact.rs`). The owner chose "Settings toggle, on by default", "At 50 min idle" and "Skip under
//! 50k tokens", the numbers editable in the config.

use std::time::Duration;

use serde::Deserialize;

/// Claude Code's prompt cache lives an hour, so a compact at 60 minutes or later no longer reads from it.
/// `minutes = 59` leaves one check before the hour ends: the first, at 59 minutes; a pane left alone then is checked
/// again a minute later (`nirvana_idle_compact::RECHECK`), at the hour, and no check is due at or after the hour.
const MAX_MINUTES: i64 = 59;

/// Signed so a negative number reaches `diagnostics` as a clear message, not a parse error for the whole config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct IdleCompactConfig {
    pub enabled: bool,
    pub minutes: i64,
    pub min_tokens: i64,
}

impl Default for IdleCompactConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            minutes: 50,
            min_tokens: 50_000,
        }
    }
}

/// The setting as the server applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleCompactRule {
    pub minutes: Duration,
    pub min_tokens: u64,
}

impl IdleCompactConfig {
    pub fn diagnostics(&self) -> Vec<String> {
        let mut diagnostics = Vec::new();
        if !(1..=MAX_MINUTES).contains(&self.minutes) {
            diagnostics.push(format!(
                "ui.idle_compact.minutes must be 1 to {MAX_MINUTES} (got {}): Claude's prompt cache lives 60 minutes; idle compact is off until it is fixed",
                self.minutes
            ));
        }
        if self.min_tokens < 0 {
            diagnostics.push(format!(
                "ui.idle_compact.min_tokens must be 0 or more (got {}); idle compact is off until it is fixed",
                self.min_tokens
            ));
        }
        diagnostics
    }

    /// `None` when the setting is off or a number is out of range: a config in doubt sends nothing.
    pub fn rule(&self) -> Option<IdleCompactRule> {
        if !self.enabled || !self.diagnostics().is_empty() {
            return None;
        }
        Some(IdleCompactRule {
            minutes: Duration::from_secs(u64::try_from(self.minutes).ok()? * 60),
            min_tokens: u64::try_from(self.min_tokens).ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_compact_config_defaults_on_at_50_minutes_and_50k() {
        let config: crate::config::Config = toml::from_str("").unwrap();
        assert_eq!(config.ui.idle_compact, IdleCompactConfig::default());
        assert_eq!(
            config.ui.idle_compact.rule(),
            Some(IdleCompactRule {
                minutes: Duration::from_secs(50 * 60),
                min_tokens: 50_000,
            })
        );
    }

    #[test]
    fn idle_compact_config_reads_its_section() {
        let config: crate::config::Config = toml::from_str(
            "[ui.idle_compact]\nenabled = false\nminutes = 40\nmin_tokens = 20000\n",
        )
        .unwrap();
        assert_eq!(
            config.ui.idle_compact,
            IdleCompactConfig {
                enabled: false,
                minutes: 40,
                min_tokens: 20_000,
            }
        );
        // Off: no rule, so nothing is sent.
        assert_eq!(config.ui.idle_compact.rule(), None);
    }

    #[test]
    fn idle_compact_config_minutes_out_of_range_is_a_clear_error_and_sends_nothing() {
        for minutes in [0, 60, -5] {
            let config = IdleCompactConfig {
                minutes,
                ..IdleCompactConfig::default()
            };
            let diagnostics = config.diagnostics();
            assert_eq!(diagnostics.len(), 1, "{minutes}: {diagnostics:?}");
            assert!(diagnostics[0].contains("ui.idle_compact.minutes must be 1 to 59"));
            assert_eq!(config.rule(), None);
        }
        for minutes in [1, 59] {
            let config = IdleCompactConfig {
                minutes,
                ..IdleCompactConfig::default()
            };
            assert!(config.diagnostics().is_empty());
            assert!(config.rule().is_some());
        }
    }

    #[test]
    fn idle_compact_config_negative_tokens_is_a_clear_error_and_sends_nothing() {
        let config = IdleCompactConfig {
            min_tokens: -1,
            ..IdleCompactConfig::default()
        };
        let diagnostics = config.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].contains("ui.idle_compact.min_tokens must be 0 or more"));
        assert_eq!(config.rule(), None);
        let zero = IdleCompactConfig {
            min_tokens: 0,
            ..IdleCompactConfig::default()
        };
        assert_eq!(zero.rule().map(|rule| rule.min_tokens), Some(0));
    }

    #[test]
    fn idle_compact_config_errors_reach_the_config_diagnostics() {
        let config: crate::config::Config =
            toml::from_str("[ui.idle_compact]\nminutes = 90\n").unwrap();
        assert!(config
            .collect_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.contains("ui.idle_compact.minutes")));
    }

    #[test]
    fn idle_compact_config_edit_writes_the_switch() {
        let edit = crate::config::ConfigEdit::IdleCompact(false);
        let written = edit.apply("[ui]\naccent = \"cyan\"\n");
        let config: crate::config::Config = toml::from_str(&written).unwrap();
        assert!(!config.ui.idle_compact.enabled);
        let back = crate::config::ConfigEdit::IdleCompact(true).apply(&written);
        let config: crate::config::Config = toml::from_str(&back).unwrap();
        assert!(config.ui.idle_compact.enabled);
        assert_eq!(config.ui.accent, "cyan");
    }
}
