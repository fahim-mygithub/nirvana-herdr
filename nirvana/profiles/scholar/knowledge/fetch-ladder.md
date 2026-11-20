---
name: Fetch ladder, how to read each kind of source, cheapest first
when: You are about to read a source, or a read failed: a web page, Reddit, YouTube, X, a PDF or paper, Hugging Face, GitHub, a wiki, or any host that blocks you. Open it before you choose a rung.
verdict: keep
source: the owner's fetch rulings of 2026-11-18, measured on headless agy 1.5.11 (Hydrogen medium); the owner's ruling of 2026-11-20 allowing clip downloads and browser screenshots; the owner's research method
tags: [fetch, ladder, reddit, youtube, pdf, browser]
---
The ladder is sorted by cost. Free rungs first; the browser only when a check fails. Keep a list of every URL you opened so you never open one twice, and a note of every host that blocked you, so you decide once per host.

1. Search with your own search tool. A snippet can settle a small fact; then stop.
2. Check the answer before you trust it: the page it names resolves, the field you asked for is present, nothing says "unavailable" or "I cannot access", and a second phrasing of the query agrees. Then open every page you will cite, in a command of its own.
3. Browser: only when step 2 fails, or when you need the whole thing. Read, never act. If your order gave you no browser tool, name the URL, what it would have answered and why the cheaper rungs could not, and put it under gotchas.

Per kind of source (measured 2026-11-18 unless dated otherwise):
- Reddit. Native search can return a thread's comments with their authors when you ask a directed question naming the thread (35 s, 19k tokens). Counts and completeness are unreliable: one thread came back as "2 comments" when it had 7. So use search for the gist and the browser for the full thread. Do not try the .json suffix, old.reddit or reader sites: all returned a 403 block page on 2026-08-13, and the 190 KB .json "response" was an HTML wall, not data.
- YouTube. A transcript comes from the browser's "Show transcript" panel (94 s, 56k tokens). Caption downloads came back as 200 with 0 bytes: that is a miss, not a success. To see frames, download the clip into your working folder with the download tool your session has, such as yt-dlp; agy reads a local clip's frames natively with its file read. Or take browser screenshots of the moments you need. A clip download was once refused by the permission check: if it is refused, say so and use screenshots. Name each frame you opened and what is in it; if you read only the transcript, say you saw no frames.
- PDFs and papers. Open them natively. Read the contents page, then the pages you need, not the whole file. Figures and tables count only if you opened that page.
- arXiv, Hugging Face, GitHub. Their own interfaces first: the arXiv API (sort by submission date), Hugging Face /api (models, datasets, daily papers), GitHub through gh when it is signed in (the bare search API allows 10 requests a minute). Search is poor for "newest"; these are not.
- A wiki. MediaWiki sites answer /api.php (Fandom) or /w/api.php (Wikipedia): action=parse&prop=text returns the article. prop=extracts does not exist on Fandom.
- X posts. Search fails on them. The browser is untried: if you try it, say what happened.
- Any other page: one fetch per page, with a browser user agent.

A read that looks fine can be false. Check what came back:
- an empty body, or an error page saved under a good name, is a failure;
- a page that never names the id, title or DOI you asked for is the wrong page; do not quote it;
- a big response is not proof: an error page can be larger than the article.

Stop at: paywall, login wall, CAPTCHA, bot challenge, a terms-of-use ban on automated access. Record the gap and move on. Never solve a challenge or sign in.
When a host blocks you, write one gap naming the host and every question it would have answered, not one gap per URL.
Images and video frames: open the file and say what you saw. A file you did not open supports no claim. Product screenshots are for reference only; do not reproduce them.
