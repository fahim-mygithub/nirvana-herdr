# Subagent record fix (branch subagent-record-fix): the record counts every token of a response written as many
# lines, and reads the report from a SubagentHandback call. One sabotage per assertion; each test named must print
# FAIL with its sabotage in place. `Control` is provided by controls.py.
CLAUDE = "src/nirvana_claude.rs"
TOKENS = "a_response_written_as_many_lines_counts_its_last_usage"
HANDBACK = "a_handback_report_is_read_from_the_call_and_prose_is_its_own_outcome"

CONTROLS = (
    # Expected red: the first line of each id wins again, output reads 98 instead of 3541.
    Control("srf_tokens_first_line_wins", CLAUDE,
            "*kept = (*kept).max(now);", "if *kept == 0 {\n                        *kept = now;\n                    }",
            TOKENS, "rust"),
    # Expected red: a handback that carries a fenced report is no longer read, so it is not subagent-done.
    Control("srf_handback_not_read", CLAUDE,
            "Some(message) => report_in(message),", "Some(_) => Err(err),",
            HANDBACK, "rust"),
    # Expected red: prose in a handback is recorded as no-report again.
    Control("srf_prose_is_no_report", CLAUDE,
            '"subagent-prose-report".to_owned()', '"subagent-no-report".to_owned()',
            HANDBACK, "rust"),
    # Expected red: the summary is cut at 20 characters, not 200.
    Control("srf_summary_cut_short", CLAUDE,
            ".take(200)", ".take(20)",
            HANDBACK, "rust"),
    # Expected red: one agent file loses the line.
    Control("srf_agent_line_missing", "nirvana/claude/agents/worker-sonnet-low.md",
            "whether you end with a text reply or a SubagentHandback call", "whether you end",
            "every_worker_agent_says_the_work_report_is_in_the_final_message", "rust"),
)
