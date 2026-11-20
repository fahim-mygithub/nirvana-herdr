---
name: Style competitive-analysis, map what the named field does, then filter it through the project
when: The order asks how named products, games, apps or tools handle something, or how others do it: a survey across named competitors, whose answer is what everyone does and what differs. Not a pick under the user's constraints.
verdict: keep
source: the owner's comparative research method, distilled 2026-11-20 for plan 47 P47b
tags: [style, comparative, survey, matrix, kano]
---
Method. A generic survey is a failed run. The result is what everyone does, filtered through this project, and the scoped direction that follows.
1. Frame the exact comparative question and this project's context: scope, theme, effort, stack. Read the project's own notes for it. State what you assume.
2. Before gathering, name 2 to 4 candidate directions you expect the field to show. Gather to confirm or deny them. Update the list when the field surprises you, and say so.
3. Plan sources by tier. Open-source products: read the code. Commercial products: their signal is in design retrospectives, talks, fan wikis, store pages and player discussion. Plan for the cheapest way in, not the browser.
4. Gather. Open and quote each source. Count forks and clones of one design as one data point.
5. Build the matrix: rows are products, columns are the features under study, each cell says how that product does it and is backed by a source. Cells without a source are marked reported, not confirmed. The insight is in the columns: what everyone has, what varies, what is rare.
6. Tag each feature column: table-stakes (users expect it), performance (more is better), or delighter (unexpected). Name the convention users already expect, so the project can follow it or break it on purpose.
7. Gap analysis: filter every finding through the scope, theme and effort of step 1. What survives is the todo.
8. Argue against it. Are the "common" patterns truly independent? Did you only collect examples of your own candidate directions? Is each secondhand feel claim marked as player-reported?

Delivery. In the summary: the matrix; the table-stakes list; a menu of 2 or 3 differentiators with the trade-offs, leading with a recommendation; the conventions; a scoped, prioritised todo. Tag each finding with its evidence (read the source, designer-stated, player-reported) and its tier, and list the gaps.

Avoid. You cannot use closed products, so their feel is secondhand: say so. Never write behaviour you inferred as architecture you saw. A visual claim needs an image you opened; with none, mark it reported, not confirmed.
