# ConPTY resize (alpha 0.71.3, upstream herdr PR #5381): a Windows pane keeps ConPTY's live buffer on resize.
# Controls starting with p73_conpty_. `Control` is provided by controls.py.
WINDOWS = "src/platform/windows.rs"

T_RESIZE = "pane::terminal::tests::windows_conpty_resize_preserves_live_buffer_and_scrollback"

CONTROLS = (
    Control("p73_conpty_policy_off", WINDOWS,
            "        .set_conpty_resize(true)\n",
            "        .set_conpty_resize(false)\n",
            T_RESIZE, "rust"),
)
