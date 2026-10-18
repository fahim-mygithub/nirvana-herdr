---
name: jev
description: Ask Jev, herdr's cheap judge, about a big file, many files or long command output without reading it into your context. Use before reading a file over 50 KB for one fact, a glob of files over 200 KB, or output over 200 lines.
---

# Jev: keeping big reads out of your context

Jev answers a typed question about a file or a command's output in about a second, for a fraction of a cent, and
the text never enters your context (plan 12 §9 role 1). 74% of the Max week went to sessions over 150k tokens, and
every extra turn re-reads your whole context, so a read you skip is a read you never pay again.

`herdr` is `"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"`. The sizes below are today's defaults in the owner's
`<home>/jev/JEV.md`; `herdr jev guide` prints the file as it is now, and the owner's numbers win.

## When

- A word or a name finds the place (a function, an error message): search. Jev does not beat a search.
- One file under 20 KB, or one you will edit, call into or quote: read it.
- One file over 50 KB, and you need one fact from it: `herdr jev file <path> --ask "<yes or no question>"`.
- Candidate files over 200 KB in all, and a question about behaviour no word finds:
  `herdr jev files "<glob>" --ask "<question>"`.
- Output over 200 lines: `<command> 2>&1 | herdr jev text --ask "<question>" --choice "a,b,c"`. Read shorter
  output yourself.
- A video, audio or an input over 400k tokens is not Jev's: seat it on argon or flash (the `media` skill).

## Handoffs

- `jev files`, then open only its `read first` file, and another only if that one does not answer.
- `jev text --choice "bug_in_code,wrong_test,environment"`: on `bug_in_code`, find the file (a search, or
  `jev files`) and read it to fix it.
- Before an edit, read the file you change and any helper you will call.
- An answer of `failed`, or a confidence under 0.7, is no answer: read it yourself. Jev never decides alone.

Options: `--choice a,b,c` or `--score low,mid,high`, `--max-kb <n>` (default 256), `--cwd <folder>`.
`herdr jev stats` shows today's calls. Jev is not a seat: it writes no code and reads no repository.
