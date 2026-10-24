//! Nirvana fork, plan 25 §3 and §5: a leaner Nirvana Claude (nirvana/plans/2026-10-22-25-lean-orchestrator.md).
//!
//! The owner, 2026-10-22 (L2), picking "Lean startup context": "Measure Nirvana Claude's first-turn tokens and cut
//! what orchestration doesn't use." And "Browser batches": "Browser workers are told to use bladebro run batches so
//! only the result reaches the model."
//!
//! **The cuts** (§3) were decided by the probe in `nirvana/plans/2026-10-22-25-records/probe.py`, one `claude -p ok`
//! per variant with Nirvana Claude's argv on Haiku, Claude Code 2.1.310, 2026-10-23. The first turn read 46,369
//! tokens as today; a cut is taken only when it lowers that by 500 or more (§3.1):
//!
//! | `--disallowedTools` | reading | saving | taken |
//! |---|---|---|---|
//! | `Workflow` | 44,259 | 2,110 | yes |
//! | `ScheduleWakeup` | 44,553 | 1,816 | yes |
//! | `ReportFindings` | 45,421 | 948 | yes |
//! | `Artifact` | 46,238 | 131 | no: not in a `-p` session's tools |
//! | `SendFeedback` | 46,242 | 127 | no: not in a `-p` session's tools |
//! | `ReadNotifications` | 46,244 | 125 | no: not in a `-p` session's tools |
//! | the three taken, together | 41,735 | 4,634 | |
//!
//! The superpowers duplicate off (136) and every user skill off (1,971) were measured too; neither is taken here (the
//! record says why). The cuts go into the one argv `nirvana_launch::launch_args_in` builds, and a launch never fails
//! because of them: any doubt keeps today's argv and logs one line.

/// The built-in tools Nirvana Claude starts without (§3.2), in the probe's order.
pub const DISALLOWED: [&str; 3] = ["Workflow", "ScheduleWakeup", "ReportFindings"];

/// §3.3: "It needs Bash, Read, Edit, Write, Grep, Glob, the Agent tool for the guarded spawn, `AskUserQuestion`,
/// and the herdr skills." The herdr skills come through the `Skill` tool. `Task` is the Agent tool's other name (a
/// `-p` session's tool list calls it that).
pub const NEEDED: [&str; 10] = [
    "Bash",
    "Read",
    "Edit",
    "Write",
    "Grep",
    "Glob",
    "Agent",
    "Task",
    "AskUserQuestion",
    "Skill",
];

pub const DISALLOWED_FLAG: &str = "--disallowedTools";

/// The cut list as the launch passes it, one comma-joined value (the form the probe measured), or why no cut is
/// made: a list that names a tool Nirvana Claude needs, or an argv that already sets the flag. Pure.
pub fn cut_value(cuts: &[&str], args: &[String]) -> Result<Option<String>, String> {
    if let Some(needed) = cuts.iter().find(|tool| NEEDED.contains(tool)) {
        return Err(format!(
            "the cut list names {needed}, which Nirvana Claude needs"
        ));
    }
    if args.iter().any(|arg| arg.starts_with(DISALLOWED_FLAG)) {
        return Err(format!("the argv already sets {DISALLOWED_FLAG}"));
    }
    Ok((!cuts.is_empty()).then(|| cuts.join(",")))
}

/// `args` with the cuts added before `--resume`, which `launch_args_in` appends after. A doubt keeps `args` as they
/// are and logs one line: a launch never fails because of a cut.
pub fn with_cuts(mut args: Vec<String>, cuts: &[&str]) -> Vec<String> {
    match cut_value(cuts, &args) {
        Ok(Some(value)) => {
            args.extend([DISALLOWED_FLAG.to_owned(), value]);
            args
        }
        Ok(None) => args,
        Err(why) => {
            tracing::warn!("nirvana: Nirvana Claude starts with every tool: {why}");
            args
        }
    }
}

/// §5's line, given to a browser order's worker.
pub const BROWSER_LINE: &str = "Use bladebro `run` to batch steps; read the result, not each page.";

/// §5: the line for an order whose class or `needs` name a browser (`browser`, `agent-browser`, `bladebro`), else
/// `None`. Pure.
pub fn browser_line(class: &str, needs: &[String]) -> Option<&'static str> {
    let names_browser = |text: &str| {
        let text = text.to_ascii_lowercase();
        text.contains("browser") || text.contains("bladebro")
    };
    (names_browser(class) || needs.iter().any(|need| names_browser(need))).then_some(BROWSER_LINE)
}

/// The worker's brief with §5's line after it when the order is a browser order; unchanged otherwise.
pub fn with_browser_line(mut brief: String, class: &str, needs: &[String]) -> String {
    if let Some(line) = browser_line(class, needs) {
        brief.push_str("\n\n");
        brief.push_str(line);
    }
    brief
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    /// §3 test: "a test names each tool Nirvana Claude needs and fails if any is in the list".
    #[test]
    fn no_tool_nirvana_claude_needs_is_cut() {
        for tool in [
            "Bash",
            "Read",
            "Edit",
            "Write",
            "Grep",
            "Glob",
            "Agent",
            "Task",
            "AskUserQuestion",
            "Skill",
        ] {
            assert!(!DISALLOWED.contains(&tool), "{tool} is cut");
            assert!(NEEDED.contains(&tool), "{tool} is not in NEEDED");
        }
    }

    #[test]
    fn the_cut_list_is_the_three_the_probe_passed() {
        assert_eq!(DISALLOWED, ["Workflow", "ScheduleWakeup", "ReportFindings"]);
        assert_eq!(
            cut_value(&DISALLOWED, &[]),
            Ok(Some("Workflow,ScheduleWakeup,ReportFindings".to_owned()))
        );
        assert_eq!(cut_value(&[], &[]), Ok(None));
    }

    /// "A launch never fails because of a cut. Any error keeps today's argv."
    #[test]
    fn a_doubt_keeps_todays_argv() {
        let today = strings(&["--agent", "nirvana"]);
        assert!(cut_value(&["Workflow", "Bash"], &today).is_err());
        assert_eq!(with_cuts(today.clone(), &["Workflow", "Bash"]), today);
        let set = strings(&["--disallowedTools", "Artifact"]);
        assert!(cut_value(&DISALLOWED, &set).is_err());
        assert_eq!(with_cuts(set.clone(), &DISALLOWED), set);
        let set_eq = strings(&["--disallowedTools=Artifact"]);
        assert_eq!(with_cuts(set_eq.clone(), &DISALLOWED), set_eq);
        assert_eq!(
            with_cuts(today.clone(), &DISALLOWED),
            strings(&[
                "--agent",
                "nirvana",
                "--disallowedTools",
                "Workflow,ScheduleWakeup,ReportFindings"
            ])
        );
    }

    /// §5 test: "the brief for a browser order holds the line; a build order's brief does not."
    #[test]
    fn only_a_browser_order_gets_the_bladebro_line() {
        assert_eq!(
            browser_line("research.web", &strings(&["browser"])),
            Some(BROWSER_LINE)
        );
        assert_eq!(browser_line("research.browser", &[]), Some(BROWSER_LINE));
        assert_eq!(
            browser_line("research.web", &strings(&["Bladebro"])),
            Some(BROWSER_LINE)
        );
        assert_eq!(
            browser_line("ops.terminal", &strings(&["agent-browser"])),
            Some(BROWSER_LINE)
        );
        assert_eq!(browser_line("build.bounded", &[]), None);
        assert_eq!(
            browser_line("research.web", &strings(&["context_1m"])),
            None
        );
        assert_eq!(
            BROWSER_LINE,
            "Use bladebro `run` to batch steps; read the result, not each page."
        );
    }

    #[test]
    fn a_browser_orders_brief_holds_the_line_and_a_build_orders_does_not() {
        let order = "Class: research.web\nOrder: o1\nRun: r1\n\nFind the price.".to_owned();
        let browser = with_browser_line(order.clone(), "research.web", &strings(&["browser"]));
        assert_eq!(browser, format!("{order}\n\n{BROWSER_LINE}"));
        let build = with_browser_line(order.clone(), "build.bounded", &[]);
        assert_eq!(build, order);
        assert!(!build.contains("bladebro"));
    }
}
