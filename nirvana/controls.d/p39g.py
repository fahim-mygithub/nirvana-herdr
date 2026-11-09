# Nirvana Remote: the message box grows with its text (the owner, 2026-11-09; record
# nirvana/plans/2026-11-04-39-records/p39g.md). Controls starting with p39g_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.

CHAT_JS = "src/nirvana_remote/page/chat.js"
APP_CSS = "src/nirvana_remote/page/app.css"
T_GROW = "nirvana_remote::tests_live::the_message_box_grows_with_its_text_and_comes_back_at_the_end"

CONTROLS = (
    Control("p39g_textarea", CHAT_JS,
            "      const input = h('textarea', { id: 'nv-draft', class: 'draft', rows: '1', autocomplete: 'off',\n",
            "      const input = h('input', { id: 'nv-draft', class: 'draft', type: 'text', autocomplete: 'off',\n",
            T_GROW, "rust"),
    Control("p39g_fit", CHAT_JS,
            "        input.style.height = Math.min(input.scrollHeight, DRAFT_MOST) + 'px';\n",
            "        input.style.height = '44px';\n",
            T_GROW, "rust"),
    Control("p39g_fit_on_input", CHAT_JS,
            "      input.addEventListener('input', () => { fitDraft(); drawPending(); });\n",
            "      input.addEventListener('input', drawPending);\n",
            T_GROW, "rust"),
    Control("p39g_fit_on_return", CHAT_JS,
            "      document.addEventListener('visibilitychange', refit);\n",
            "",
            T_GROW, "rust"),
    Control("p39g_caret_end", CHAT_JS,
            "      input.addEventListener('focus', () => requestAnimationFrame(toEnd));\n",
            "",
            T_GROW, "rust"),
    Control("p39g_no_handle", APP_CSS,
            "resize: none; overflow-y: hidden;",
            "overflow-y: hidden;",
            T_GROW, "rust"),
)
