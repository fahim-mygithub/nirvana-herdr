# Old Nirvana's "Open Nirvana here" retired with a backup (R8, added at the O4a merge, 2026-10-18).
CONTROLS = (
    Control("o4a_py_old_nirvana_verb_kept", "nirvana/windows_shell.py",
            'return {sub: reg.delete_tree(rf"{base}\\{sub}") for sub in found}',
            "return {sub: False for sub in found}",
            "nirvana.test_windows_shell.VerbTest.test_install_writes_the_verb_and_toast_id_and_removes_nirvana_cs_verb",
            "python"),
    Control("o4a_py_old_nirvana_no_backup", "nirvana/windows_shell.py",
            'backup.write_text(json.dumps(found, indent=2) + "\\n", encoding="utf-8")',
            'pass',
            "nirvana.test_windows_shell.VerbTest.test_install_writes_the_verb_and_toast_id_and_removes_nirvana_cs_verb",
            "python"),
    Control("o4a_py_other_nirvana_key_removed", "nirvana/windows_shell.py",
            "if command is not None and OLD_NIRVANA_LAUNCHER in command:",
            "if command is not None:",
            "nirvana.test_windows_shell.VerbTest.test_a_nirvana_key_of_another_program_is_left_alone",
            "python"),
)
