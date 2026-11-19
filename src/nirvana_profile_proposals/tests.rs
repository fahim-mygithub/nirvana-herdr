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
    assert!(why.contains("holds more than the profile op-1 accepted"), "{why}");
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
