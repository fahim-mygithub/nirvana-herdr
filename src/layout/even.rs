//! Nirvana fork, plan 15 sizing fix: the even grid `TileLayout::tile_even` builds. Pure helpers, kept out of
//! `layout.rs` because that file is upstream's and this fork merges upstream often (review F4, 2026-11-19).
//!
//! The incident: the owner's Megajam Jobs tab on 2026-11-19 held six job panes at 1/2, 1/4, 1/8, 1/16, 1/32 and 1/32
//! of the width, each job splitting the last pane in half; the two still running were about ten columns wide and
//! wrapped their output one word a line.

use ratatui::layout::{Direction, Rect};

use super::{Node, PaneId};

/// The even grid's usual widest row. Three columns of a 200-column window are 66 wide; more panes stack into rows.
/// `even_columns` goes past it only to keep rows `EVEN_MIN_HEIGHT` tall.
pub const EVEN_MAX_COLUMNS: usize = 3;
/// The narrowest pane the even grid makes, outer width with its border: an 80-column window takes two columns of 40,
/// never three of 26.
pub const EVEN_MIN_WIDTH: u16 = 30;
/// The shortest pane the even grid makes when the area allows it, outer height with its border (review F2: twenty
/// panes on 120x40 were 5 rows tall at three columns; four columns of 30 make them 8).
pub const EVEN_MIN_HEIGHT: u16 = 6;
/// The area assumed when the server knows none (width or height 0: no view computed yet). 80x24 is the smallest
/// common terminal, so the grid it picks (two columns from two panes) is readable on any client that attaches later,
/// where three columns would be 26 wide on an 80-column one.
pub const EVEN_FALLBACK_AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};

/// How many panes a row of the even grid holds for `count` panes in `area`. The usual count is at most
/// `EVEN_MAX_COLUMNS` and no more than keeps every pane `EVEN_MIN_WIDTH` wide. While its rows would be shorter than
/// `EVEN_MIN_HEIGHT`, a column is added (past `EVEN_MAX_COLUMNS` if need be) as long as every pane stays
/// `EVEN_MIN_WIDTH` wide. When no count keeps both floors, the usual count stands: no pane is dropped or refused.
pub fn even_columns(count: usize, area: Rect) -> usize {
    let area = if area.width == 0 || area.height == 0 {
        EVEN_FALLBACK_AREA
    } else {
        area
    };
    let fit = usize::from(area.width / EVEN_MIN_WIDTH).max(1);
    let usual = count.clamp(1, EVEN_MAX_COLUMNS.min(fit));
    // A row's height is at least the area's height over the row count: `even_tree`'s rows differ by at most one.
    let tall_enough = |columns: usize| {
        usize::from(area.height) / count.div_ceil(columns).max(1) >= usize::from(EVEN_MIN_HEIGHT)
    };
    if tall_enough(usual) {
        return usual;
    }
    (usual + 1..=fit.min(count))
        .find(|&columns| tall_enough(columns))
        .unwrap_or(usual)
}

/// The panes in each row of the even grid, top to bottom: as few rows as `columns` allows, the counts at most one
/// apart, the fuller rows first, so the newest panes (the last, at the bottom) are the wider ones.
pub fn even_rows(count: usize, columns: usize) -> Vec<usize> {
    if count == 0 {
        return Vec::new();
    }
    let columns = columns.max(1);
    let rows = count.div_ceil(columns);
    let (base, extra) = (count / rows, count % rows);
    (0..rows)
        .map(|row| base + usize::from(row < extra))
        .collect()
}

/// `nodes` side by side (`Horizontal`) or stacked (`Vertical`) in equal shares, as a balanced tree: each split's
/// ratio is its first half's share of the panes (1/2 or 1/3 at the deepest), so `valid_split_ratio`'s 0.1 floor is
/// never reached, which a chain of 1/n splits would hit at eleven panes.
fn balanced(mut nodes: Vec<Node>, direction: Direction) -> Option<Node> {
    match nodes.len() {
        0 => None,
        1 => nodes.pop(),
        len => {
            let second = nodes.split_off(len / 2);
            let ratio = (len / 2) as f32 / len as f32;
            Some(Node::Split {
                direction,
                ratio,
                first: Box::new(balanced(nodes, direction)?),
                second: Box::new(balanced(second, direction)?),
            })
        }
    }
}

/// The even grid of `ids` in their order, left to right then top to bottom, rows of `even_rows(ids.len(), columns)`.
pub fn even_tree(ids: &[PaneId], columns: usize) -> Option<Node> {
    let mut rest = ids;
    let rows = even_rows(ids.len(), columns)
        .into_iter()
        .filter_map(|take| {
            let (row, after) = rest.split_at(take);
            rest = after;
            balanced(
                row.iter().map(|id| Node::Pane(*id)).collect(),
                Direction::Horizontal,
            )
        })
        .collect();
    balanced(rows, Direction::Vertical)
}

/// The even grid of `ids` at `columns` a row, or `None` when `root` already is it (the tile reports no change).
pub(super) fn regrid(root: &Node, ids: &[PaneId], columns: usize) -> Option<Node> {
    let tree = even_tree(ids, columns)?;
    (!same_tree(&tree, root)).then_some(tree)
}

/// Whether two trees have the same panes, splits and ratios.
fn same_tree(a: &Node, b: &Node) -> bool {
    match (a, b) {
        (Node::Pane(x), Node::Pane(y)) => x == y,
        (
            Node::Split {
                direction: da,
                ratio: ra,
                first: fa,
                second: sa,
            },
            Node::Split {
                direction: db,
                ratio: rb,
                first: fb,
                second: sb,
            },
        ) => da == db && (ra - rb).abs() < f32::EPSILON && same_tree(fa, fb) && same_tree(sa, sb),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::TileLayout;

    /// A layout of `count` panes the way master's jobs built it: each new pane split off the last one to the right.
    fn p96_chain(count: usize) -> (TileLayout, Vec<PaneId>) {
        let (mut layout, root) = TileLayout::new();
        let mut ids = vec![root];
        for _ in 1..count {
            let last = *ids.last().unwrap();
            ids.push(layout.split_pane(last, Direction::Horizontal, 0.5).unwrap());
        }
        (layout, ids)
    }

    fn p96_widths(layout: &TileLayout, area: Rect) -> Vec<u16> {
        layout.panes(area).iter().map(|p| p.rect.width).collect()
    }

    fn p96_heights(layout: &TileLayout, area: Rect) -> Vec<u16> {
        layout.panes(area).iter().map(|p| p.rect.height).collect()
    }

    fn p96_area(width: u16, height: u16) -> Rect {
        Rect::new(0, 0, width, height)
    }

    #[test]
    fn p96_chained_splits_halve_each_pane_the_defect() {
        let area = p96_area(320, 40);
        let (layout, _) = p96_chain(6);
        assert_eq!(p96_widths(&layout, area), vec![160, 80, 40, 20, 10, 10]);
    }

    #[test]
    fn p96_even_columns_cap_at_three_and_the_readable_width() {
        for (count, width, height, columns) in [
            (1, 200, 50, 1),
            (2, 200, 50, 2),
            (3, 200, 50, 3),
            (7, 200, 50, 3),
            (12, 200, 50, 3),
            (2, 80, 24, 2),
            (3, 80, 24, 2),
            (4, 50, 24, 1),
            (0, 200, 50, 1),
            // Twelve on 80x24 are 4 rows tall at two columns, and three would be 26 wide: the usual two stand.
            (12, 80, 24, 2),
            // Thirty on 160x20: five columns of 32 still give rows of 3, so no count keeps both floors.
            (30, 160, 20, 3),
        ] {
            assert_eq!(
                even_columns(count, p96_area(width, height)),
                columns,
                "{count} panes in {width}x{height}"
            );
        }
    }

    /// Review F2: rows under `EVEN_MIN_HEIGHT` take a column more while every pane stays `EVEN_MIN_WIDTH` wide.
    #[test]
    fn p96_even_columns_add_a_column_to_keep_rows_six_tall() {
        for (count, width, height, columns) in [
            // Three columns give 7 rows of 5; four give 5 rows of 8, each 30 wide.
            (20, 120, 40, 4),
            // Seven at three columns are 3 rows of 6 on 120x18; nothing to add.
            (7, 120, 18, 3),
            // Ten on 120x18: three columns are 4 rows of 4, four columns 3 rows of 6.
            (10, 120, 18, 4),
            // Forty on 320x80: three columns are 14 rows of 5, four are 10 rows of 8.
            (40, 320, 80, 4),
        ] {
            assert_eq!(
                even_columns(count, p96_area(width, height)),
                columns,
                "{count} panes in {width}x{height}"
            );
        }
    }

    /// Review F3: a server with no view computed has a 0x0 area; the grid is the one for 80x24, two columns.
    #[test]
    fn p96_even_columns_of_an_unknown_area_are_80x24s() {
        assert_eq!(even_columns(5, Rect::default()), 2);
        assert_eq!(even_columns(5, p96_area(0, 40)), 2);
        assert_eq!(even_columns(1, Rect::default()), 1);
    }

    #[test]
    fn p96_even_rows_fill_the_first_rows_and_differ_by_one() {
        for (count, columns, rows) in [
            (0, 3, vec![]),
            (1, 3, vec![1]),
            (3, 3, vec![3]),
            (4, 3, vec![2, 2]),
            (5, 3, vec![3, 2]),
            (7, 3, vec![3, 2, 2]),
            (12, 3, vec![3, 3, 3, 3]),
            (5, 2, vec![2, 2, 1]),
        ] {
            assert_eq!(even_rows(count, columns), rows, "{count} in {columns}");
        }
    }

    #[test]
    fn p96_every_pane_is_readable_and_even_up_to_twelve() {
        for area in [p96_area(200, 50), p96_area(80, 24)] {
            for count in 1..=12 {
                let (mut layout, ids) = p96_chain(count);
                layout.tile_even(area);
                let panes = layout.panes(area);
                assert_eq!(
                    panes.iter().map(|p| p.id).collect::<Vec<_>>(),
                    ids,
                    "order kept"
                );
                let columns = even_columns(count, area);
                let mut at = 0;
                for take in even_rows(count, columns) {
                    let row = &panes[at..at + take];
                    at += take;
                    let widths: Vec<u16> = row.iter().map(|p| p.rect.width).collect();
                    let (least, most) =
                        (*widths.iter().min().unwrap(), *widths.iter().max().unwrap());
                    assert!(
                        least >= EVEN_MIN_WIDTH,
                        "{count} panes in {area:?}: {widths:?}"
                    );
                    assert!(most - least <= 1, "{count} panes in {area:?}: {widths:?}");
                    assert_eq!(widths.iter().sum::<u16>(), area.width);
                }
                let heights: Vec<u16> = panes.iter().map(|p| p.rect.height).collect();
                let (least, most) = (
                    *heights.iter().min().unwrap(),
                    *heights.iter().max().unwrap(),
                );
                assert!(most - least <= 1, "{count} panes in {area:?}: {heights:?}");
            }
        }
    }

    /// Review F2, as the owner would see it: every pane's width and height on three windows, written out. Rows are
    /// listed top to bottom, the fuller first, and the panes keep their order.
    #[test]
    fn p96_even_grid_sizes_on_three_windows() {
        for (width, height, count, widths, heights) in P96_SIZES {
            let area = p96_area(*width, *height);
            let (mut layout, ids) = p96_chain(*count);
            layout.tile_even(area);
            assert_eq!(
                layout.panes(area).iter().map(|p| p.id).collect::<Vec<_>>(),
                ids,
                "{count} in {width}x{height}: order kept"
            );
            assert_eq!(
                p96_widths(&layout, area),
                widths.to_vec(),
                "{count} in {width}x{height}: widths"
            );
            assert_eq!(
                p96_heights(&layout, area),
                heights.to_vec(),
                "{count} in {width}x{height}: heights"
            );
        }
    }

    /// (width, height, panes, each pane's width, each pane's height), panes in order. The column counts follow
    /// `even_columns`'s rule worked by hand (80x24: two columns, the height floor out of reach at 11 and 20; 120x40:
    /// three, four at 20; 320x80: three); the order of the one-cell remainders is the renderer's, read off a probe.
    const P96_SIZES: &[(u16, u16, usize, &[u16], &[u16])] = &[
        (80, 24, 5, &[40, 40, 40, 40, 80], &[8, 8, 8, 8, 8]),
        (
            80,
            24,
            7,
            &[40, 40, 40, 40, 40, 40, 80],
            &[6, 6, 6, 6, 6, 6, 6],
        ),
        (
            80,
            24,
            11,
            &[40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 80],
            &[4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4],
        ),
        (
            80,
            24,
            20,
            &[40; 20],
            &[3, 3, 2, 2, 2, 2, 3, 3, 2, 2, 3, 3, 2, 2, 2, 2, 3, 3, 2, 2],
        ),
        (120, 40, 5, &[40, 40, 40, 60, 60], &[20, 20, 20, 20, 20]),
        (
            120,
            40,
            7,
            &[40, 40, 40, 60, 60, 60, 60],
            &[13, 13, 13, 14, 14, 13, 13],
        ),
        (
            120,
            40,
            11,
            &[40, 40, 40, 40, 40, 40, 40, 40, 40, 60, 60],
            &[10; 11],
        ),
        (120, 40, 20, &[30; 20], &[8; 20]),
        (
            320,
            80,
            7,
            &[107, 107, 106, 160, 160, 160, 160],
            &[27, 27, 27, 27, 27, 26, 26],
        ),
        (
            320,
            80,
            11,
            &[107, 107, 106, 107, 107, 106, 107, 107, 106, 160, 160],
            &[20; 11],
        ),
        (
            320,
            80,
            20,
            &[
                107, 107, 106, 107, 107, 106, 107, 107, 106, 107, 107, 106, 107, 107, 106, 107,
                107, 106, 160, 160,
            ],
            &[
                11, 11, 11, 12, 12, 12, 11, 11, 11, 12, 12, 12, 11, 11, 11, 12, 12, 12, 11, 11,
            ],
        ),
    ];

    #[test]
    fn p96_six_jobs_tile_as_two_rows_of_three() {
        let area = p96_area(200, 50);
        let (mut layout, ids) = p96_chain(6);
        layout.focus_pane(ids[5]);
        assert!(layout.tile_even(area));
        assert_eq!(p96_widths(&layout, area), vec![67, 67, 66, 67, 67, 66]);
        assert_eq!(layout.focused(), ids[5], "focus kept");
        assert!(!layout.tile_even(area), "an even tab is unchanged");
    }

    #[test]
    fn p96_a_close_in_an_even_tab_grows_the_survivors_back() {
        let area = p96_area(200, 50);
        let (mut layout, ids) = p96_chain(6);
        // Focus on the newest, so the first closes take `close_pane`'s unfocused path and the last `close_focused`.
        layout.focus_pane(ids[5]);
        layout.tile_even(area);
        assert!(layout.close_pane(ids[0]));
        assert!(layout.close_pane(ids[1]));
        assert_eq!(p96_widths(&layout, area), vec![100, 100, 100, 100]);
        // From the 2x2 grid, a close with no re-tile would leave [200, 100, 100].
        layout.focus_pane(ids[2]);
        assert!(layout.close_focused());
        assert_eq!(p96_widths(&layout, area), vec![67, 67, 66]);
        assert!(layout.close_pane(ids[3]));
        assert_eq!(p96_widths(&layout, area), vec![100, 100]);
    }

    /// Review F3: a close counts the columns again for the survivors. Twenty panes on 120x40 take four columns of 30
    /// (`EVEN_MIN_HEIGHT`); the eight left after twelve closes take three of 40, where the tile's four would leave
    /// two rows of four 30-wide panes.
    #[test]
    fn p96_a_close_counts_the_columns_for_the_survivors() {
        let area = p96_area(120, 40);
        let (mut layout, ids) = p96_chain(20);
        layout.focus_pane(ids[19]);
        layout.tile_even(area);
        assert_eq!(p96_widths(&layout, area), vec![30; 20]);
        for id in &ids[..12] {
            assert!(layout.close_pane(*id));
        }
        assert_eq!(
            p96_widths(&layout, area),
            vec![40, 40, 40, 40, 40, 40, 60, 60]
        );
    }

    /// Review section 5: a close that re-tiles leaves the focus where it was. Six panes, so the close does rebuild:
    /// closing the first of a 3+2 grid leaves the even 2+2 as it stands, and the control on this test stayed green on
    /// that shape.
    #[test]
    fn p96_a_close_that_retiles_keeps_the_focus() {
        let area = p96_area(200, 50);
        let (mut layout, ids) = p96_chain(6);
        layout.focus_pane(ids[4]);
        layout.tile_even(area);
        assert!(layout.close_pane(ids[0]));
        assert_eq!(p96_widths(&layout, area), vec![67, 67, 66, 100, 100]);
        assert_eq!(layout.focused(), ids[4]);
    }

    #[test]
    fn p96_the_owners_own_split_or_resize_ends_the_even_grid() {
        let area = p96_area(200, 50);
        let (mut layout, ids) = p96_chain(3);
        layout.tile_even(area);
        assert!(layout.keeps_even());
        layout
            .split_pane(ids[2], Direction::Horizontal, 0.5)
            .unwrap();
        assert!(!layout.keeps_even(), "a split ends it");
        layout.tile_even(area);
        assert!(layout.set_ratio_at(&[], 0.7));
        assert!(!layout.keeps_even(), "a ratio change ends it");
        let before = p96_widths(&layout, area);
        assert!(layout.close_pane(ids[0]));
        assert_ne!(p96_widths(&layout, area).len(), before.len());
        assert!(!layout.keeps_even());
    }
}
