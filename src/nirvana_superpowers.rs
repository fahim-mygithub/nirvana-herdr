//! Nirvana fork, plan 28: superpowers only on Sonnet (nirvana/plans/2026-10-23-28-superpowers-by-model.md).
//!
//! The owner, 2026-10-23 (L1): "agents perform poorer with superpowers attached to opus. Given the preload cost of
//! enabling superpowers if you are using opus level model for planning and building features, do not preload
//! supoerpowers." And L3: "If were constrained on tokens and have to start planning and building with a sonnet levle
//! model, superpowers seems to provide osme improvements."
//!
//! So a Claude Nirvana starts on an Opus model gets every `superpowers` plugin off, through the one `--settings` file
//! it already passes: Nirvana Claude (`nirvana_launch::launch_args`) and Claude workers (`nirvana_worker`). Sonnet and
//! every other model get no entry, and the launch is today's. The owner's own Claude panes and the Preload panel are
//! not touched: the rule writes its own files under `<home>/nirvana/launch/`, never the preset's.
//!
//! The rule outranks Preload's `core` (which lists `superpowers` as "never switched off") for these launches only:
//! `--settings` is Claude Code's flag-settings layer, above the user's and the project's, and `enabledPlugins` merges
//! per id, so an id set `false` there is off whatever the user's settings say. That is how the preset's own files turn
//! a plugin off (plan 13).
//!
//! Plan 25 found superpowers enabled twice, from two marketplaces (`superpowers@superpowers-marketplace` and
//! `superpowers@claude-plugins-official`), so the ids are every installed or enabled plugin whose name before `@` is
//! `superpowers`. Only names are read from Claude's files. A launch never fails because of this rule: any error keeps
//! today's argv and says so in one log line.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The plugin's name before the `@`.
pub const NAME: &str = "superpowers";
/// The id seen in fixtures and on the owner's machine, used when no file lists one.
pub const DEFAULT_ID: &str = "superpowers@superpowers-marketplace";
/// Under `<home>/nirvana`: the files this rule writes, one per folder and family.
pub const LAUNCH_DIR: &str = "launch";

/// The model families the rule tells apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Opus,
    Sonnet,
    Other,
}

impl Family {
    pub fn word(self) -> &'static str {
        match self {
            Self::Opus => "opus",
            Self::Sonnet => "sonnet",
            Self::Other => "other",
        }
    }
}

/// The family of a model id (`claude-opus-5-5`), alias (`opus`) or label (`Claude Opus 5.5`). Haiku, Fable, a local
/// model and an empty string are `Other`.
pub fn family(model: &str) -> Family {
    let model = model.to_ascii_lowercase();
    if model.contains("opus") {
        Family::Opus
    } else if model.contains("sonnet") {
        Family::Sonnet
    } else {
        Family::Other
    }
}

/// The rule (§2): for an Opus model, every superpowers id off; for any other model, no entry, so superpowers loads
/// as the user's settings say.
pub fn entries(model: &str, ids: &[String]) -> Vec<(String, bool)> {
    match family(model) {
        Family::Opus => ids.iter().map(|id| (id.clone(), false)).collect(),
        Family::Sonnet | Family::Other => Vec::new(),
    }
}

/// The ids among `names` whose name before `@` is `superpowers`, sorted and once each; `DEFAULT_ID` when none is.
/// `superpowers-lab@...` and the like are other plugins (§6: "Other plugins" are not in this plan).
pub fn superpowers_ids<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut ids: Vec<String> = names
        .into_iter()
        .filter(|id| {
            id.split('@')
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case(NAME))
        })
        .map(str::to_owned)
        .collect();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        ids.push(DEFAULT_ID.to_owned());
    }
    ids
}

/// The plugin ids Claude's `plugins/installed_plugins.json` lists: the keys of its `plugins` object, nothing else.
pub fn installed_names(installed: &Value) -> Vec<String> {
    installed
        .get("plugins")
        .and_then(Value::as_object)
        .map(|plugins| plugins.keys().cloned().collect())
        .unwrap_or_default()
}

/// The plugin ids a settings file names under `enabledPlugins`, whatever their value.
pub fn enabled_names(settings: &Value) -> Vec<String> {
    settings
        .get("enabledPlugins")
        .and_then(Value::as_object)
        .map(|plugins| plugins.keys().cloned().collect())
        .unwrap_or_default()
}

/// `base` with the rule's entries under `enabledPlugins`; every other key, and every other plugin's entry, kept. A
/// base that is not an object, or whose `enabledPlugins` is not one, is refused rather than overwritten: the launch
/// then keeps today's file.
pub fn merged(base: &Value, entries: &[(String, bool)]) -> Result<Value, String> {
    let mut value = base.clone();
    let root = value
        .as_object_mut()
        .ok_or("the settings file is not a JSON object")?;
    let plugins = root
        .entry("enabledPlugins")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("its enabledPlugins is not a JSON object")?;
    for (id, on) in entries {
        plugins.insert(id.clone(), Value::Bool(*on));
    }
    Ok(value)
}

/// `<dir>/launch/<folder hash>.<family>.settings.json`: Nirvana Claude's launch file for the folder (§3).
pub fn launch_settings_path(dir: &Path, folder: &Path, family: Family) -> PathBuf {
    dir.join(LAUNCH_DIR).join(format!(
        "{}.{}.settings.json",
        crate::nirvana_launch::folder_hash(folder),
        family.word()
    ))
}

/// `<dir>/launch/<folder hash>.<family>.worker.settings.json`: a Claude worker's, kept apart from Nirvana Claude's
/// because its base is the worker preset (no `NIRVANA_ORCHESTRATOR` env), not the plugin's settings.
pub fn worker_settings_path(dir: &Path, folder: &Path, family: Family) -> PathBuf {
    dir.join(LAUNCH_DIR).join(format!(
        "{}.{}.worker.settings.json",
        crate::nirvana_launch::folder_hash(folder),
        family.word()
    ))
}

/// Claude Code's config folder, where `plugins/installed_plugins.json` is. `None` in tests: no test reads the
/// owner's `~/.claude` (plan 28 §4); tests pass a folder of their own to the `*_in` functions.
pub fn claude_dir() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    crate::nirvana::claude_config_dir()
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// The superpowers ids for a launch in `folder`: from Claude's installed list, the folder's own settings, and the
/// file the launch passes. Each file that is missing or unreadable adds nothing.
pub fn ids_for(claude_dir: Option<&Path>, folder: &Path, base: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(installed) =
        claude_dir.and_then(|dir| read_json(&dir.join("plugins").join("installed_plugins.json")))
    {
        names.extend(installed_names(&installed));
    }
    for file in ["settings.json", "settings.local.json"] {
        if let Some(settings) = read_json(&folder.join(".claude").join(file)) {
            names.extend(enabled_names(&settings));
        }
    }
    names.extend(enabled_names(base));
    superpowers_ids(names.iter().map(String::as_str))
}

/// Through a temporary file, so a Claude starting never reads half of it.
fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, text).map_err(|err| format!("{}: {err}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|err| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {err}", path.display())
    })
}

/// Nirvana Claude (§3): the launch file for `model` in `folder`, built from `base`, the file the launch would have
/// passed. `Ok(None)` when the rule has no entry for the model, so the launch passes `base` as today. A missing or
/// broken `base` is an error: there is nothing to merge into.
pub fn launch_settings_in(
    dir: &Path,
    folder: &Path,
    model: &str,
    base: &Path,
    claude_dir: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let family = family(model);
    if family != Family::Opus {
        return Ok(None);
    }
    let base_value =
        read_json(base).ok_or_else(|| format!("{} is missing or not JSON", base.display()))?;
    let entries = entries(model, &ids_for(claude_dir, folder, &base_value));
    let path = launch_settings_path(dir, folder, family);
    write_json(&path, &merged(&base_value, &entries)?)?;
    Ok(Some(path))
}

/// A Claude worker (§3): the file its argv passes. `preset` is the worker preset `with_preset_settings` would have
/// added. With no entry for the model it is `preset` unchanged; with entries, a launch file holding the preset merged
/// with them, or the entries alone when the folder has no preset. An agy worker is never touched: its `g-opus` seat's
/// model names Opus but agy reads no Claude settings (plan 13 §10).
pub fn worker_settings_in(
    dir: &Path,
    cli: &str,
    model: &str,
    folder: &Path,
    preset: Option<&Path>,
    claude_dir: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let family = family(model);
    if cli != "claude" || family != Family::Opus {
        return Ok(preset.map(Path::to_path_buf));
    }
    let base_value = match preset {
        Some(file) => {
            read_json(file).ok_or_else(|| format!("{} is missing or not JSON", file.display()))?
        }
        None => Value::Object(Map::new()),
    };
    let entries = entries(model, &ids_for(claude_dir, folder, &base_value));
    let path = worker_settings_path(dir, folder, family);
    write_json(&path, &merged(&base_value, &entries)?)?;
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn opus_turns_every_superpowers_off_and_sonnet_and_the_rest_add_nothing() {
        // §2's table.
        let two = ids(&[
            "superpowers@claude-plugins-official",
            "superpowers@superpowers-marketplace",
        ]);
        let off = vec![
            ("superpowers@claude-plugins-official".to_owned(), false),
            ("superpowers@superpowers-marketplace".to_owned(), false),
        ];
        for model in ["claude-opus-5-5", "opus", "Claude Opus 5.5"] {
            assert_eq!(family(model), Family::Opus, "{model}");
            assert_eq!(entries(model, &two), off, "{model}");
        }
        for model in ["claude-sonnet-5-5", "sonnet"] {
            assert_eq!(family(model), Family::Sonnet, "{model}");
            assert!(entries(model, &two).is_empty(), "{model}");
        }
        for model in ["claude-haiku-5-5", "claude-fable-5-1", ""] {
            assert_eq!(family(model), Family::Other, "{model:?}");
            assert!(entries(model, &two).is_empty(), "{model:?}");
        }
    }

    #[test]
    fn the_ids_are_every_plugin_named_superpowers_before_the_at_or_the_default() {
        assert_eq!(
            superpowers_ids([
                "superpowers@superpowers-marketplace",
                "superpowers-lab@superpowers-marketplace",
                "superpowers-chrome@superpowers-marketplace",
                "github@claude-plugins-official",
                "superpowers@claude-plugins-official",
                "superpowers@superpowers-marketplace",
            ]),
            ids(&[
                "superpowers@claude-plugins-official",
                "superpowers@superpowers-marketplace",
            ])
        );
        assert_eq!(superpowers_ids([]), ids(&[DEFAULT_ID]));
        assert_eq!(
            superpowers_ids(["github@claude-plugins-official"]),
            ids(&[DEFAULT_ID])
        );
        // Only names are read from Claude's files: the keys, never what they hold.
        let installed = json!({"version": 2, "plugins": {
            "superpowers@superpowers-marketplace": [{"scope": "user", "installPath": "C:\\x"}],
            "github@claude-plugins-official": [{"scope": "user"}],
        }});
        assert_eq!(
            installed_names(&installed),
            ids(&[
                "github@claude-plugins-official",
                "superpowers@superpowers-marketplace"
            ])
        );
        assert!(installed_names(&json!({"plugins": []})).is_empty());
        let settings = json!({"enabledPlugins": {"superpowers@claude-plugins-official": true}, "env": {"A": "1"}});
        assert_eq!(
            enabled_names(&settings),
            ids(&["superpowers@claude-plugins-official"])
        );
    }

    #[test]
    fn the_merge_keeps_every_other_key_and_refuses_what_is_not_an_object() {
        let base = json!({
            "env": {"NIRVANA_ORCHESTRATOR": "1"},
            "statusLine": {"type": "command"},
            "enabledPlugins": {"github@claude-plugins-official": false, "superpowers@superpowers-marketplace": true},
            "skillOverrides": {"x": "off"},
        });
        let entries = vec![("superpowers@superpowers-marketplace".to_owned(), false)];
        let out = merged(&base, &entries).unwrap();
        assert_eq!(
            out,
            json!({
                "env": {"NIRVANA_ORCHESTRATOR": "1"},
                "statusLine": {"type": "command"},
                "enabledPlugins": {"github@claude-plugins-official": false, "superpowers@superpowers-marketplace": false},
                "skillOverrides": {"x": "off"},
            })
        );
        assert_eq!(
            merged(&json!({}), &entries).unwrap(),
            json!({"enabledPlugins": {"superpowers@superpowers-marketplace": false}})
        );
        // No entries: the base, unchanged.
        assert_eq!(merged(&base, &[]).unwrap(), base);
        assert!(merged(&json!([1]), &entries).is_err());
        assert!(merged(&json!({"enabledPlugins": []}), &entries).is_err());
    }

    #[test]
    fn no_test_reads_the_owners_claude_folder() {
        // Plan 28 §4: the launch's own reads go through `claude_dir`, which a test build never resolves.
        assert_eq!(claude_dir(), None);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nirvana-p28-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_ids_come_from_the_installed_list_the_folders_settings_and_the_base() {
        let root = scratch("ids");
        let claude = root.join("claude-config");
        let folder = root.join("project");
        std::fs::create_dir_all(claude.join("plugins")).unwrap();
        std::fs::create_dir_all(folder.join(".claude")).unwrap();
        std::fs::write(
            claude.join("plugins").join("installed_plugins.json"),
            json!({"version": 2, "plugins": {"superpowers@a": []}}).to_string(),
        )
        .unwrap();
        std::fs::write(
            folder.join(".claude").join("settings.json"),
            json!({"enabledPlugins": {"superpowers@b": true}}).to_string(),
        )
        .unwrap();
        std::fs::write(
            folder.join(".claude").join("settings.local.json"),
            json!({"enabledPlugins": {"superpowers@c": true}}).to_string(),
        )
        .unwrap();
        let base = json!({"enabledPlugins": {"superpowers@d": true}});
        assert_eq!(
            ids_for(Some(&claude), &folder, &base),
            ids(&[
                "superpowers@a",
                "superpowers@b",
                "superpowers@c",
                "superpowers@d"
            ])
        );
        // Nothing lists one: the default.
        assert_eq!(
            ids_for(None, &root.join("none"), &json!({})),
            ids(&[DEFAULT_ID])
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_opus_launch_writes_the_merged_file_and_sonnet_writes_nothing() {
        let root = scratch("launch");
        let dir = root.join("nirvana");
        let folder = Path::new(r"C:\work\print");
        let base = dir.join("claude").join("settings.json");
        std::fs::create_dir_all(base.parent().unwrap()).unwrap();
        std::fs::write(&base, r#"{"env":{"NIRVANA_ORCHESTRATOR":"1"}}"#).unwrap();
        let path = launch_settings_in(&dir, folder, "claude-opus-5-5", &base, None)
            .unwrap()
            .expect("an opus launch has a file");
        assert_eq!(path, launch_settings_path(&dir, folder, Family::Opus));
        assert_eq!(
            path,
            dir.join("launch").join(format!(
                "{}.opus.settings.json",
                crate::nirvana_launch::folder_hash(folder)
            ))
        );
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            written,
            json!({"env": {"NIRVANA_ORCHESTRATOR": "1"}, "enabledPlugins": {"superpowers@superpowers-marketplace": false}})
        );
        for model in ["claude-sonnet-5-5", "claude-haiku-5-5", ""] {
            assert_eq!(
                launch_settings_in(&dir, folder, model, &base, None).unwrap(),
                None
            );
        }
        // A base that is gone is an error, so the caller keeps today's argv.
        assert!(launch_settings_in(&dir, folder, "opus", &dir.join("missing.json"), None).is_err());
        std::fs::write(&base, "{not json").unwrap();
        assert!(launch_settings_in(&dir, folder, "opus", &base, None).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_opus_claude_worker_gets_the_rule_with_or_without_a_preset() {
        let root = scratch("worker");
        let dir = root.join("nirvana");
        let folder = root.join("project");
        std::fs::create_dir_all(&folder).unwrap();
        let preset = dir.join("preload").join("x.worker.settings.json");
        std::fs::create_dir_all(preset.parent().unwrap()).unwrap();
        std::fs::write(&preset, r#"{"skillOverrides":{"docx":"off"}}"#).unwrap();
        let expected = worker_settings_path(&dir, &folder, Family::Opus);
        // Its base is the worker preset, not the plugin's settings, so it never shares Nirvana Claude's file.
        assert_ne!(expected, launch_settings_path(&dir, &folder, Family::Opus));
        // With a preset: the preset merged with the rule.
        let with = worker_settings_in(
            &dir,
            "claude",
            "claude-opus-5-5",
            &folder,
            Some(&preset),
            None,
        )
        .unwrap();
        assert_eq!(with.as_deref(), Some(expected.as_path()));
        let read = |path: &Path| -> Value {
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
        };
        assert_eq!(
            read(&expected),
            json!({"skillOverrides": {"docx": "off"}, "enabledPlugins": {"superpowers@superpowers-marketplace": false}})
        );
        // No preset: the rule's entries alone.
        let alone = worker_settings_in(&dir, "claude", "opus", &folder, None, None).unwrap();
        assert_eq!(alone.as_deref(), Some(expected.as_path()));
        assert_eq!(
            read(&expected),
            json!({"enabledPlugins": {"superpowers@superpowers-marketplace": false}})
        );
        // Sonnet: the preset as today, or nothing.
        assert_eq!(
            worker_settings_in(
                &dir,
                "claude",
                "claude-sonnet-5-5",
                &folder,
                Some(&preset),
                None
            )
            .unwrap()
            .as_deref(),
            Some(preset.as_path())
        );
        assert_eq!(
            worker_settings_in(&dir, "claude", "claude-sonnet-5-5", &folder, None, None).unwrap(),
            None
        );
        // agy on an Opus model reads no Claude settings: untouched.
        assert_eq!(
            worker_settings_in(&dir, "agy", "claude-opus-5-5-high", &folder, None, None).unwrap(),
            None
        );
        // A broken preset is an error, so the caller keeps today's argv.
        std::fs::write(&preset, "[").unwrap();
        assert!(worker_settings_in(&dir, "claude", "opus", &folder, Some(&preset), None).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
