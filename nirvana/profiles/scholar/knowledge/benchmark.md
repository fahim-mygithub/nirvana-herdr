---
name: Style benchmark, report measured numbers with their conditions
when: The order asks a quantitative question: how fast, how much, what are the numbers, benchmark X, the performance of Y under Z. The answer is measured data with its methodology, not adjectives.
verdict: keep
source: the owner's quantitative research method, distilled 2026-11-20 for plan 47 P47b
tags: [style, quantitative, benchmark, methodology, numbers]
---
Method. A number without its conditions is noise, and you never report one. Your core discipline is scepticism about method: whose numbers, on what hardware, under what load, reproducible or vendor marketing.
1. Frame the measurement: which quantity, under which realistic conditions, for which decision. "How fast is X" alone cannot be answered. "p99 latency at about 1k requests a second on ordinary hardware" can.
2. If you hold a prior, state the expected range first ("usually quoted as 2 to 5 times faster"). Gathering then confirms it, refutes it or shows the spread.
3. Hunt primary data: original write-ups with a published harness, repositories you can rerun, papers with a methods section. Prefer them to aggregator tables, vendor pages and blog round-ups. Tag each figure with its tier.
4. Interrogate every figure: hardware, versions, data or load shape, warm or cold, who ran it and what they sell. A number whose conditions you cannot recover is downgraded to anecdote, however widely quoted. Ten sites repeating one vendor chart are one measurement.
5. Where you have a shell and the workload is small enough, measure it yourself in your working folder: build the minimal harness, run it, and report your own figures marked as yours, with their conditions. A laptop microbenchmark does not settle a datacentre question; say so when scale limits apply.
6. Argue against it. Are the headline figures comparable at all (same versions, same load), or is this apples to oranges? Did you keep only figures that fit the expected range? Is the best-looking number the least sourced?

Delivery. In the summary:
- each figure with its method, conditions and tier;
- your own measurements, flagged, with the harness described;
- caveats on how far the figures can be compared;
- an honest bottom line, such as "2 to 3 times in memory-bound work, parity in IO-bound, no trustworthy data for your case";
- the numbers you could not get or reproduce.

Avoid. A single figure standing alone. A vendor's chart as a measurement.
