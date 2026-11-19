use super::*;

use crate::nirvana_seat;

/// A project under a test home with the default seats, a `.mcp.json` naming one made-up server (a fake command, never
/// a definition from this machine), and one project skill, `alpha`.
fn project(name: &str) -> (crate::nirvana::TestHomeGuard, PathBuf) {
    let home = crate::nirvana::with_test_home(name);
    let seats = home.path.join("nirvana").join(nirvana_seat::SEATS_FILE);
    std::fs::create_dir_all(seats.parent().unwrap()).unwrap();
    std::fs::write(&seats, nirvana_seat::DEFAULT).unwrap();
    let root = home.path.join("project");
    std::fs::create_dir_all(root.join(".claude").join("skills").join("alpha")).unwrap();
    std::fs::write(
        root.join(".claude/skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: the alpha skill.\n---\nbody\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers": {"fake-tool": {"command": "fake-mcp-not-real"}}}"#,
    )
    .unwrap();
    (home, root)
}

const NEW: &str =
    "---\ndescription: Judges renders against the rubric card.\nclass: media.video\neffort: high\n\
needs: [video, fake-tool]\nskills: [alpha]\n---\nYou judge renders. Open every file yourself.\n";
const EVIDENCE: &str =
    "7 orders repeat a 412-character paragraph: \"Judge the rendered frame, never the plan or the code\"";

fn draft(kind: Kind, profile: &str, body: &str, base_sha: Option<String>) -> Draft {
    Draft {
        kind,
        profile: profile.into(),
        by: DETECTOR.into(),
        run: "run-20261119".into(),
        evidence: EVIDENCE.into(),
        key: "paragraph:3f2a9c".into(),
        body: body.into(),
        base_sha,
    }
}

fn refused(root: &Path, draft: &Draft) -> Vec<String> {
    write(root, draft).expect_err("refused").0
}

fn existing(root: &Path, name: &str, text: &str) -> PathBuf {
    let dir = nirvana_profiles::folder_of(root, name);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(nirvana_profiles::PROFILE_FILE);
    std::fs::write(&file, text).unwrap();
    file
}

#[test]
fn p92_a_new_proposal_is_written_listed_accepted_and_rolled_back_exactly() {
    let (_home, root) = project("p92-pp-new");
    let id = write(&root, &draft(Kind::New, "render-judge", NEW, None)).unwrap();
    assert_eq!(id, "pp-1");
    let waiting = list(&root);
    assert_eq!(waiting.len(), 1);
    assert_eq!(
        (
            waiting[0].kind,
            waiting[0].profile.as_str(),
            waiting[0].body.as_str()
        ),
        (Kind::New, "render-judge", NEW.trim())
    );
    assert_eq!(
        waiting[0].file,
        root.join(".nirvana/profile-proposals/pp-1.md")
    );
    // Inert: outside `profiles/`, so no profile is listed or seated by it.
    assert!(nirvana_profiles::list(&root).is_empty());
    let line = accept(&root, &id).unwrap();
    assert!(
        line.starts_with("op-1: accepted pp-1, new profile render-judge"),
        "{line}"
    );
    let file = nirvana_profiles::folder_of(&root, "render-judge").join("profile.md");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), NEW);
    assert!(nirvana_profiles::load(&root, "render-judge").is_ok());
    assert!(list(&root).is_empty(), "the proposal is no longer waiting");
    let rows = ledger(&root);
    assert_eq!(
        (
            rows[0].action.as_str(),
            rows[0].before.as_deref(),
            rows[0].after.as_deref()
        ),
        ("accept", None, Some(NEW))
    );
    // A card taught into the new profile since: the rollback refuses rather than take it out with the folder.
    let card = nirvana_profiles::folder_of(&root, "render-judge").join("knowledge/taught.md");
    std::fs::write(&card, "a card\n").unwrap();
    let why = rollback(&root, "op-1").unwrap_err();
    assert!(
        why.contains("holds more than the profile op-1 accepted"),
        "{why}"
    );
    assert!(card.is_file());
    std::fs::remove_file(&card).unwrap();
    let back = rollback(&root, "op-1").unwrap();
    assert!(back.starts_with("op-2: rolled back op-1"), "{back}");
    assert!(!nirvana_profiles::folder_of(&root, "render-judge").exists());
    assert_eq!(list(&root).len(), 1, "waiting again");
    let again = rollback(&root, "op-1").unwrap_err();
    assert!(again.contains("rolled back already"), "{again}");
}

#[test]
fn p92_a_proposal_may_not_carry_pins_unknown_skills_needs_or_what_parse_refuses() {
    let (_home, root) = project("p92-pp-limits");
    let with = |from: &str, to: &str| draft(Kind::New, "judge", &NEW.replace(from, to), None);
    let found = refused(
        &root,
        &with("effort: high\n", "effort: high\nmodel: claude-opus-5-5\n"),
    );
    assert!(
        found
            .iter()
            .any(|why| why.contains("never pins a model or a family")),
        "{found:?}"
    );
    let found = refused(&root, &with("skills: [alpha]", "skills: [alpha, ghost]"));
    assert!(
        found == ["skills ghost: the project has no such skill in .claude/skills"],
        "{found:?}"
    );
    let found = refused(
        &root,
        &with("needs: [video, fake-tool]", "needs: [video, other-server]"),
    );
    assert!(
        found.len() == 1
            && found[0].starts_with("needs other-server: neither a seat capability nor a server"),
        "{found:?}"
    );
    let found = refused(
        &root,
        &with(
            "Open every file yourself.",
            "Run `herdr seat run --send-ok` when done.",
        ),
    );
    assert!(
        found
            .iter()
            .any(|why| why.contains("the body grants a Send")),
        "{found:?}"
    );
    existing(
        &root,
        "judge",
        "---\ndescription: The owner's own.\n---\nMine.\n",
    );
    let found = refused(&root, &draft(Kind::New, "judge", NEW, None));
    assert!(
        found
            .iter()
            .any(|why| why.contains("a profile judge exists already")),
        "{found:?}"
    );
    // The evidence: a count and a quote.
    let mut bare = draft(Kind::New, "other", NEW, None);
    bare.evidence = "many orders repeat a paragraph about judging frames".into();
    let found = refused(&root, &bare);
    assert!(
        found
            .iter()
            .any(|why| why.starts_with("the evidence holds no count")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|why| why.starts_with("the evidence quotes nothing")),
        "{found:?}"
    );
    // Accept checks again: a proposal edited by hand after it was written is refused.
    let id = write(&root, &draft(Kind::New, "other", NEW, None)).unwrap();
    let file = path(&root, &id);
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        text.replace("effort: high\n", "effort: high\nfamily: google\n"),
    )
    .unwrap();
    let why = accept(&root, &id).unwrap_err();
    assert!(why.contains("never pins a model or a family"), "{why}");
    assert!(!nirvana_profiles::folder_of(&root, "other").exists());
}

#[test]
fn p92_a_patch_replaces_the_body_keeps_the_keys_and_a_stale_one_is_refused() {
    let (_home, root) = project("p92-pp-patch");
    let before =
        "---\ndescription: Builds the engine.\nclass: build.bounded\neffort: high\n---\nOld body.\n";
    let file = existing(&root, "ue-builder", before);
    let sha = profile_sha(&root, "ue-builder").unwrap();
    let patch = draft(
        Kind::Patch,
        "ue-builder",
        "New body.\nmodel: claude-opus-5-5\n",
        Some(sha),
    );
    let id = write(&root, &patch).unwrap();
    let line = accept(&root, &id).unwrap();
    assert!(
        line.starts_with("op-1: accepted pp-1, a patch of ue-builder"),
        "{line}"
    );
    let after = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        after,
        "---\ndescription: Builds the engine.\nclass: build.bounded\neffort: high\n---\nNew body.\n\
         model: claude-opus-5-5\n"
    );
    // A body line is never a key: the model line is body text.
    assert_eq!(
        nirvana_profiles::load(&root, "ue-builder")
            .unwrap()
            .profile
            .model,
        None
    );
    let back = rollback(&root, "op-1").unwrap();
    assert!(back.starts_with("op-2: rolled back op-1"), "{back}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    // Stale: the profile changed between the proposal (waiting again) and the accept.
    let edited = before.replace("Old body.", "The owner's edit.");
    std::fs::write(&file, &edited).unwrap();
    let why = accept(&root, "pp-1").unwrap_err();
    assert!(
        why.contains("is stale: profile ue-builder changed since it was proposed"),
        "{why}"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), edited);
}

#[test]
fn p92_a_dismissed_proposal_is_not_made_again_until_the_dismissal_is_rolled_back() {
    let (_home, root) = project("p92-pp-dismiss");
    let first = draft(Kind::New, "render-judge", NEW, None);
    let id = write(&root, &first).unwrap();
    assert_eq!(
        refused(&root, &first),
        ["the same proposal is waiting already as pp-1"]
    );
    let line = dismiss(&root, &id, Some("  not this one  ")).unwrap();
    assert!(line.starts_with("op-1: dismissed pp-1"), "{line}");
    assert!(list(&root).is_empty());
    assert_eq!(ledger(&root)[0].reason.as_deref(), Some("not this one"));
    assert!(was_dismissed(&root, Kind::New, "render-judge", "paragraph:3f2a9c").is_some());
    // The same identity, whitespace and case aside, is refused; another key is not.
    let mut same = first.clone();
    same.key = "  Paragraph:3F2A9C ".into();
    let found = refused(&root, &same);
    assert!(
        found.len() == 1
            && found[0]
                .starts_with("the owner dismissed this proposal (new of render-judge) on 20"),
        "{found:?}"
    );
    let mut other = first.clone();
    other.key = "paragraph:other".into();
    assert_eq!(write(&root, &other).unwrap(), "pp-2");
    let back = rollback(&root, "op-1").unwrap();
    assert!(back.contains("pp-1 is waiting again"), "{back}");
    assert!(was_dismissed(&root, Kind::New, "render-judge", "paragraph:3f2a9c").is_none());
    assert_eq!(
        list(&root)
            .iter()
            .map(|proposal| proposal.id.as_str())
            .collect::<Vec<_>>(),
        ["pp-1", "pp-2"]
    );
}

#[test]
fn p92_the_cli_lists_shows_accepts_dismisses_and_takes_only_proposal_ids() {
    let (_home, root) = project("p92-pp-cli");
    let run = |args: &[&str]| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        let code = cli_to(&root, &args, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    };
    let (code, out, _) = run(&["proposals"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("no proposal waiting in "), "{out}");
    write(&root, &draft(Kind::New, "render-judge", NEW, None)).unwrap();
    let (_, out, _) = run(&["proposals"]);
    assert!(
        out.starts_with(
            "pp-1 \u{b7} new \u{b7} render-judge \u{b7} 7 orders repeat a 412-character paragraph"
        ),
        "{out}"
    );
    let (code, out, _) = run(&["proposal", "show", "pp-1"]);
    assert!(
        code == 0 && out.contains("\nkind: new\nprofile: render-judge\n"),
        "{out}"
    );
    let (code, _, err) = run(&["proposal", "show", "../../secret"]);
    assert!(
        code == 1 && err.contains("is not a proposal id (pp-<n>)"),
        "{err}"
    );
    let (code, out, _) = run(&["accept", "pp-1"]);
    assert!(code == 0 && out.starts_with("op-1: accepted pp-1"), "{out}");
    let (code, out, _) = run(&["proposals", "rollback", "op-1"]);
    assert!(
        code == 0 && out.starts_with("op-2: rolled back op-1"),
        "{out}"
    );
    let (code, out, _) = run(&["dismiss", "pp-1", "--reason", "later"]);
    assert!(
        code == 0 && out.starts_with("op-3: dismissed pp-1"),
        "{out}"
    );
    let (code, _, err) = run(&["dismiss"]);
    assert!(
        code == 2 && err.starts_with("usage: herdr profile proposals"),
        "{err}"
    );
}

// ---- P45c fix round ----

/// Review F4, confirmed by running: a ledger line torn mid-write (no newline) swallowed the next dismissal, which was
/// then dropped with it, so the same proposal would be made again and an op id could repeat. Now the next row goes on
/// a line of its own and is remembered, the torn line is reported, and no op number it may hold is used again.
#[test]
fn p92_a_torn_ledger_line_loses_no_later_row_and_its_op_is_not_reused() {
    let (_home, root) = project("p92-pp-torn");
    let first = write(&root, &draft(Kind::New, "render-judge", NEW, None)).unwrap();
    let mut other = draft(Kind::New, "render-judge", NEW, None);
    other.key = "paragraph:other".into();
    let second = write(&root, &other).unwrap();
    assert!(dismiss(&root, &first, None)
        .unwrap()
        .starts_with("op-1: dismissed"));
    let ledger_file = folder(&root).join(LEDGER);
    let mut text = std::fs::read_to_string(&ledger_file).unwrap();
    text.push_str("{\"v\":1,\"op\":\"op-9\",\"at\":1,\"action\":\"dis");
    std::fs::write(&ledger_file, &text).unwrap();
    let line = dismiss(&root, &second, None).unwrap();
    assert!(line.starts_with("op-10: dismissed pp-2"), "{line}");
    assert!(
        was_dismissed(&root, Kind::New, "render-judge", "paragraph:other").is_some(),
        "the dismissal after the torn line is remembered"
    );
    assert_eq!(ledger(&root).len(), 2);
    let torn = broken(&root);
    assert!(torn.len() == 1 && torn[0].0 == 2, "{torn:?}");
    let shown = unreadable(&root);
    assert!(
        shown.iter().any(|(file, why)| *file == ledger_file
            && why.starts_with("line 2 does not parse (")
            && !why.contains("\"dis")),
        "{shown:?}"
    );
    // And the same proposal is refused again, not made a second time.
    let found = refused(&root, &other);
    assert!(
        found
            .iter()
            .any(|why| why.starts_with("the owner dismissed this proposal")),
        "{found:?}"
    );
}

/// Review F5: the writer's key is redacted before it lands, as the evidence and the body are, and keeps its identity
/// (two paragraph shas stay two); `accept` redacts the body it writes, so a hand-edited proposal cannot put a secret
/// into a profile.
#[test]
fn p92_the_key_and_an_accepted_body_are_redacted() {
    let (_home, root) = project("p92-pp-redact");
    let token = "ghp_FAKEFAKEFAKEFAKEFAKE1234567890";
    let mut secret = draft(Kind::New, "render-judge", NEW, None);
    secret.key = format!("header:{token}");
    let id = write(&root, &secret).unwrap();
    let text = std::fs::read_to_string(path(&root, &id)).unwrap();
    assert!(!text.contains(token), "{text}");
    assert!(was_dismissed(&root, Kind::New, "render-judge", &secret.key).is_none());
    dismiss(&root, &id, None).unwrap();
    assert!(
        was_dismissed(&root, Kind::New, "render-judge", &secret.key).is_some(),
        "a raw key and the key on disk are one identity"
    );
    let shas = ["a".repeat(63) + "1", "b".repeat(63) + "2"];
    let ids: Vec<String> = shas
        .iter()
        .map(|sha| {
            let mut one = draft(Kind::New, "render-judge", NEW, None);
            one.key = format!("paragraph:{sha}");
            write(&root, &one).unwrap()
        })
        .collect();
    let keys: Vec<String> = ids.iter().map(|id| get(&root, id).unwrap().key).collect();
    assert!(
        keys[0] != keys[1] && keys.iter().all(|key| key.starts_with("paragraph:h:")),
        "{keys:?}"
    );
    assert!(keys
        .iter()
        .all(|key| !shas.iter().any(|sha| key.contains(sha.as_str()))));
    // A body edited by hand to carry a token: accept writes it redacted.
    let file = path(&root, &ids[0]);
    let edited = std::fs::read_to_string(&file).unwrap().replace(
        "Open every file yourself.",
        "Open every file yourself. Use sk-FAKEFAKEFAKEFAKEFAKE1234.",
    );
    std::fs::write(&file, edited).unwrap();
    accept(&root, &ids[0]).unwrap();
    let written = std::fs::read_to_string(
        nirvana_profiles::folder_of(&root, "render-judge").join("profile.md"),
    )
    .unwrap();
    assert!(
        written.contains("Use [redacted].") && !written.contains("sk-FAKE"),
        "{written}"
    );
}

/// Review section 5: a patch of a legacy one-file profile, `<name>.md` with CRLF line ends, is accepted into that
/// file and rolled back to its exact bytes.
#[test]
fn p92_a_patch_of_a_legacy_one_file_profile_rolls_back_to_the_exact_bytes() {
    let (_home, root) = project("p92-pp-legacy");
    let bytes = b"---\r\ndescription: Builds the engine.\r\neffort: high\r\n---\r\nOld body.\r\n";
    let file = nirvana_profiles::path(&root, "legacy");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, bytes).unwrap();
    let sha = profile_sha(&root, "legacy").unwrap();
    let id = write(&root, &draft(Kind::Patch, "legacy", "New body.", Some(sha))).unwrap();
    accept(&root, &id).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "---\ndescription: Builds the engine.\neffort: high\n---\nNew body.\n"
    );
    assert!(
        !nirvana_profiles::folder_of(&root, "legacy").exists(),
        "no folder made"
    );
    rollback(&root, "op-1").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
}

/// Review section 5: the front matter of a `new` body edited by hand at accept: ` model:` (a leading space), a
/// duplicate key, and a second front-matter block; and a proposal's own front matter with a key twice. Accept never
/// writes an `mcp.json`, whatever the body says.
#[test]
fn p92_hand_edited_front_matter_at_accept_and_no_mcp_json() {
    let (_home, root) = project("p92-pp-front");
    let edit = |id: &str, from: &str, to: &str| {
        let file = path(&root, id);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains(from), "{text}");
        std::fs::write(&file, text.replacen(from, to, 1)).unwrap();
    };
    let id = write(&root, &draft(Kind::New, "one", NEW, None)).unwrap();
    edit(
        &id,
        "effort: high\n",
        "effort: high\n model: claude-opus-5-5\n",
    );
    let why = accept(&root, &id).unwrap_err();
    assert!(why.contains("never pins a model or a family"), "{why}");
    let id = write(&root, &draft(Kind::New, "two", NEW, None)).unwrap();
    edit(
        &id,
        "effort: high\n",
        "effort: high\ndescription: A second one.\n",
    );
    let why = accept(&root, &id).unwrap_err();
    assert!(why.contains("duplicate `description`"), "{why}");
    let id = write(&root, &draft(Kind::New, "three", NEW, None)).unwrap();
    edit(&id, "evidence:", "key: another\nevidence:");
    let why = accept(&root, &id).unwrap_err();
    assert!(why.contains("duplicate `key`"), "{why}");
    for name in ["one", "two", "three"] {
        assert!(!nirvana_profiles::folder_of(&root, name).exists(), "{name}");
    }
    // A second block lands as body text: the keys are the first block's, and no mcp.json is written.
    let id = write(&root, &draft(Kind::New, "four", NEW, None)).unwrap();
    edit(
        &id,
        "You judge renders.",
        "---\nmodel: claude-opus-5-5\n---\n{\"mcpServers\": {\"fake-x\": {\"command\": \"fake\"}}}\nYou judge renders.",
    );
    accept(&root, &id).unwrap();
    let loaded = nirvana_profiles::load(&root, "four").unwrap();
    assert_eq!(
        (loaded.profile.model, loaded.servers.is_none()),
        (None, true)
    );
    let mut entries: Vec<String> = std::fs::read_dir(nirvana_profiles::folder_of(&root, "four"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    assert_eq!(entries, ["knowledge", "profile.md"]);
}

/// Review F7: a new profile named for a Windows device is refused at write and at accept, and the failed accept
/// leaves no folder.
#[test]
fn p92_a_proposal_for_a_windows_device_name_is_refused() {
    let (_home, root) = project("p92-pp-device");
    let found = refused(&root, &draft(Kind::New, "con", NEW, None));
    assert!(
        found.len() == 1 && found[0].contains("is a Windows device name"),
        "{found:?}"
    );
    let id = write(&root, &draft(Kind::New, "ok-name", NEW, None)).unwrap();
    let file = path(&root, &id);
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("profile: ok-name\n", "profile: nul\n")).unwrap();
    let why = accept(&root, &id).unwrap_err();
    assert!(why.contains("is a Windows device name"), "{why}");
    assert!(nirvana_profiles::list(&root).is_empty());
}

/// Concurrent writers: four threads write four proposals at once; the lock gives each its own id and file.
#[test]
fn p92_concurrent_writers_get_distinct_proposals() {
    let (_home, root) = project("p92-pp-threads");
    let handles: Vec<_> = (0..4)
        .map(|n| {
            let root = root.clone();
            std::thread::spawn(move || {
                let mut one = draft(Kind::New, "render-judge", NEW, None);
                one.key = format!("paragraph:thread-{n}");
                write(&root, &one).unwrap()
            })
        })
        .collect();
    let mut ids: Vec<String> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    ids.sort();
    assert_eq!(ids, ["pp-1", "pp-2", "pp-3", "pp-4"]);
    assert_eq!(list(&root).len(), 4);
}
