# Phone resume (fix phone-resume-sheet): the home-screen page reads the visual viewport again on each way back, so a
# keyboard that went while the page was away does not leave the column at its height. The owner, 2026-11-16: "switch
# to nirvana after I switch out on iOS causes this bug". Controls starting with p82_. One sabotage per assertion, each
# on a product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
# The behaviour is nirvana/test_phoneresume.py's (python, node underneath).
APP_JS = "src/nirvana_remote/page/app.js"

_P = "nirvana.test_phoneresume.PhoneResumeTest."

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
)
