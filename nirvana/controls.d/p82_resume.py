# Phone resume (fix phone-resume-sheet): the home-screen page reads the visual viewport again on each way back, so a
# keyboard that went while the page was away does not leave the column at its height. The owner, 2026-11-16: "switch
# to nirvana after I switch out on iOS causes this bug". Controls starting with p82_. One sabotage per assertion, each
# on a product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
# The behaviour is nirvana/test_phoneresume.py's (python, node underneath).
APP_JS = "src/nirvana_remote/page/app.js"
PROMPT_JS = "src/nirvana_remote/page/prompt.js"
APP_CSS = "src/nirvana_remote/page/app.css"

_P = "nirvana.test_phoneresume.PhoneResumeTest."
_D = "nirvana.test_phoneprompt.PromptSheetDrawTest."
_L = "nirvana.test_phoneprompt.PromptSheetLayoutTest."

CONTROLS = (
    # Shown again (the app switcher): the viewport is read
    Control("p82_resume_visible", APP_JS,
            "if (document.visibilityState === 'visible') again(); });\n",
            "if (document.visibilityState === 'hidden') again(); });\n",
            _P + "test_the_column_is_whole_again_when_the_page_is_shown", "python"),
    # Out of the page cache
    Control("p82_resume_pageshow", APP_JS,
            "      window.addEventListener('pageshow', again);\n",
            "      window.addEventListener('pagehide', again);\n",
            _P + "test_the_column_is_whole_again_out_of_the_page_cache", "python"),
    # A window resize (rotation)
    Control("p82_resume_resize", APP_JS,
            "      window.addEventListener('resize', again);\n",
            "      window.addEventListener('resized', again);\n",
            _P + "test_the_column_is_whole_again_on_a_window_resize", "python"),
    # The box lets go of the keyboard
    Control("p82_resume_focusout", APP_JS,
            "      document.addEventListener('focusout', again);\n",
            "      document.addEventListener('focusin', again);\n",
            _P + "test_the_column_is_whole_again_when_the_box_lets_go_of_the_keyboard", "python"),
    # A size iOS reports a frame late is read on that frame
    Control("p82_resume_frame", APP_JS,
            "const again = () => { fit(); if (window.requestAnimationFrame) window.requestAnimationFrame(fit); };\n",
            "const again = () => { fit(); if (window.requestAnimationFrame) window.requestAnimationFrame(() => {}); };\n",
            _P + "test_a_size_ios_reports_a_frame_late_is_read", "python"),

    # --- The prompt sheet reads on a phone (the owner, 2026-11-16: "Also the choice dialog doesn't read well on the
    # app"). Drawing: nirvana/test_phoneprompt.py PromptSheetDrawTest (node). Layout: PromptSheetLayoutTest reads
    # app.css's declarations; each sabotage keeps the property and changes its value, so the test FAILs, not ERRORs.
    # A raw screen is headed by its last question
    Control("p82_prompt_raw_title", PROMPT_JS,
            "          : lastQuestion(data.screen);\n",
            "          : '';\n",
            _D + "test_a_raw_screen_is_headed_by_its_last_question", "python"),
    # With no question, the agent's name heads it (not some other row)
    Control("p82_prompt_raw_fallback", PROMPT_JS,
            "      for (let i = rows.length - 1; i >= 0; i--) if (/\\?$/.test(rows[i])) return rows[i];\n      return '';\n",
            "      for (let i = rows.length - 1; i >= 0; i--) if (/\\?$/.test(rows[i])) return rows[i];\n      return rows[rows.length - 1] || '';\n",
            _D + "test_a_raw_screen_with_no_question_is_headed_by_the_agent", "python"),
    # A raw screen's box opens at its end, where the options are
    Control("p82_prompt_raw_end", PROMPT_JS,
            "box.scrollTop = box.scrollHeight;",
            "box.scrollTop = 0;",
            _D + "test_a_raw_screen_box_opens_at_its_end", "python"),
    # A menu's first choice is the marked one
    Control("p82_prompt_first_marked", PROMPT_JS,
            "'press answer-option' + (i === 0 ? ' first' : '')",
            "'press answer-option' + (i === 1 ? ' first' : '')",
            _D + "test_the_keypad_is_only_for_a_screen_not_read_as_a_menu", "python"),
    # The sheet ends below the status bar
    Control("p82_css_sheet_safe_top", APP_CSS,
            "max-height: calc(100% - var(--safe-top) - var(--gap-top));",
            "max-height: calc(100% - var(--gap-top));",
            _L + "test_the_sheet_stays_below_the_status_bar", "python"),
    # The question is pinned at the top of the sheet
    Control("p82_css_head_sticky", APP_CSS,
            "gap: 10px; position: sticky; top: 0;",
            "gap: 10px; position: static; top: 0;",
            _L + "test_the_question_stays_at_the_top_of_the_sheet", "python"),
    # The body shrinks to the sheet
    Control("p82_css_body_shrinks", APP_CSS,
            "flex: 1 1 auto; min-height: 0; max-height: 80vh;",
            "flex: 1 1 auto; min-height: auto; max-height: 80vh;",
            _L + "test_the_prompt_body_shrinks_to_the_sheet_and_scrolls", "python"),
    # The screen box is short
    Control("p82_css_box_short", APP_CSS,
            "max-height: 26vh; max-height: 26dvh;",
            "max-height: 26vh; max-height: 46dvh;",
            _L + "test_the_screen_box_is_short_and_scrolls_in_itself", "python"),
    # The choices are tall
    Control("p82_css_choice_height", APP_CSS,
            "gap: 12px; min-height: 56px; padding: 10px 14px;",
            "gap: 12px; min-height: 44px; padding: 10px 14px;",
            _L + "test_the_choices_are_large", "python"),
    # The choices' text is large
    Control("p82_css_choice_font", APP_CSS,
            "background: var(--hover); font-size: 16px; font-weight: 500; line-height: 1.3;",
            "background: var(--hover); font-size: 14px; font-weight: 500; line-height: 1.3;",
            _L + "test_the_choices_are_large", "python"),
)
