---
name: Style hypothesis-elimination, rule causes out until one is left
when: The order asks why something happens: what causes Y, why it fails only when Z, a bug, a regression or a failure, a root cause. The answer is one isolated cause among candidates, not a list of possible causes.
verdict: keep
source: the owner's causal research method, distilled 2026-11-20 for plan 47 P47b
tags: [style, causal, diagnosis, root-cause, ach]
---
Method. This is strong inference: at each step do the cheapest thing that would rule a candidate out. The stop is "cause isolated", never "sources exhausted".
1. Frame the effect exactly: what happens, and under which discriminating conditions (only in CI, only after the deploy, only on a cold start). Those conditions cost nothing and already remove whole families of causes.
2. List every plausible cause before you gather: environment, timing, configuration, a code change, data, an external dependency. A cause you did not list cannot be tested.
3. Score evidence by how well it discriminates, not by how on-topic it is. Evidence that fits every candidate is worthless however much there is.
4. Eliminate cheapest split first. At each step choose the search, log check, repro or experiment that most cheaply divides the candidates that remain. Look in issue trackers, changelogs, bug threads and reports shaped like "only in X". If you can reproduce or bisect in your own working folder, do that instead of reading opinions; a five-minute repro outranks an hour of forum posts.
5. Stop when one candidate survives and the others were ruled out by evidence. If several survive, say so and name the test that would separate them. Never pick a "most likely" by feel.
6. Argue against it. Consistency is not confirmation: is the survivor supported by evidence against its rivals, or only consistent with itself? Was each rejected candidate really ruled out, or never investigated?

Delivery. In the summary:
- the isolated cause, or the survivors plus the separating test;
- for each rejected candidate, the evidence that eliminated it, with URL and quote;
- confidence, as a function of how cleanly the rivals were ruled out;
- candidates you could not test, and why; the next diagnostic step if unresolved.

Avoid. Gather-and-summarise: a tidy list of possible causes is not an answer to this order.
