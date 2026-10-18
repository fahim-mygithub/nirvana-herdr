# Package controls

One file per package, `<package>.py`, defining `CONTROLS = (Control(...), ...)`. `controls.py` loads every file here
after its own tuple. Parallel packages that inserted into one tuple lost a `    ),` at merges; files merge cleanly.
