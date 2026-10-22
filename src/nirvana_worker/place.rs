//! Where a run's next worker pane goes in the run's tab.
//!
//! Until 0.35.1 every worker split the last one to the right: the owner's screenshot of a `plan-ahead` run showed five
//! columns, the last two about ten letters wide, with three of the five workers long finished. Now a new worker splits
//! the pane that leaves the roomiest halves, and once no split leaves a readable pane it takes over the pane of the
//! run's oldest finished worker, whose lines stay in that pane's scrollback and in `herdr seat read`.

/// A split must leave each half at least this many columns, so a Claude Code line still reads.
pub const MIN_COLS: u16 = 80;
/// And at least this many rows.
pub const MIN_ROWS: u16 = 16;

/// A pane of the run's tab, as `herdr pane layout` measures it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub pane: String,
    pub cols: u16,
    pub rows: u16,
    /// Its worker's order ended and only a shell runs in it: the one kind of pane taken over.
    pub finished: bool,
    /// The order of its worker in the run, oldest first; a pane with no worker of this run has none.
    pub age: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    Split { pane: String, right: bool },
    Reuse(String),
}

/// A cell is about twice as tall as it is wide: a half's size as it looks, its shorter side in column widths.
fn look(cols: u16, rows: u16) -> u32 {
    u32::from(cols).min(u32::from(rows) * 2)
}

/// The next worker's place, or `None` when the tab has no pane to place it by.
pub fn place(slots: &[Slot]) -> Option<Placement> {
    // A split's halves lose a column or row to the border.
    let halves = |slot: &Slot, right: bool| {
        if right {
            (slot.cols.saturating_sub(1) / 2, slot.rows)
        } else {
            (slot.cols, slot.rows.saturating_sub(1) / 2)
        }
    };
    let best_split = slots
        .iter()
        .flat_map(|slot| [(slot, true), (slot, false)])
        .filter(|(slot, right)| {
            let (cols, rows) = halves(slot, *right);
            cols >= MIN_COLS && rows >= MIN_ROWS
        })
        .max_by_key(|(slot, right)| {
            let (cols, rows) = halves(slot, *right);
            // The roomiest half wins; between equals, the side-by-side split, as before, then the bigger pane.
            (
                look(cols, rows),
                *right,
                u32::from(slot.cols) * u32::from(slot.rows),
            )
        });
    if let Some((slot, right)) = best_split {
        return Some(Placement::Split {
            pane: slot.pane.clone(),
            right,
        });
    }
    if let Some(oldest) = slots
        .iter()
        .filter(|slot| slot.finished)
        .min_by_key(|slot| slot.age.unwrap_or(usize::MAX))
    {
        return Some(Placement::Reuse(oldest.pane.clone()));
    }
    // Every pane is too small and busy: split the biggest along its longer side, as readable as it gets.
    let biggest = slots
        .iter()
        .max_by_key(|slot| u32::from(slot.cols) * u32::from(slot.rows))?;
    Some(Placement::Split {
        pane: biggest.pane.clone(),
        right: biggest.cols >= biggest.rows.saturating_mul(2),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(pane: &str, cols: u16, rows: u16, finished: bool, age: usize) -> Slot {
        Slot {
            pane: pane.into(),
            cols,
            rows,
            finished,
            age: Some(age),
        }
    }

    #[test]
    fn a_wide_pane_splits_side_by_side_then_the_halves_split_down() {
        // The owner's tab: about 230 columns by 68 rows.
        assert_eq!(
            place(&[slot("a", 230, 68, false, 0)]),
            Some(Placement::Split {
                pane: "a".into(),
                right: true
            })
        );
        // Two columns of 115: a third column would be 57 wide, so one of them splits down.
        assert_eq!(
            place(&[slot("a", 115, 68, false, 0), slot("b", 114, 68, false, 1)]),
            Some(Placement::Split {
                pane: "a".into(),
                right: false
            })
        );
    }

    #[test]
    fn a_full_tab_takes_over_the_oldest_finished_pane() {
        // Four quarters of 30 rows: a split down would leave 14.
        let tab = [
            slot("a", 116, 30, true, 0),
            slot("b", 115, 30, true, 2),
            slot("c", 114, 30, false, 3),
            slot("d", 114, 30, true, 1),
        ];
        assert_eq!(place(&tab), Some(Placement::Reuse("a".into())));
        // A half under 80 columns or 16 rows is not a split: 150 by 30 is taken over, not halved.
        assert_eq!(
            place(&[slot("a", 150, 30, true, 0)]),
            Some(Placement::Reuse("a".into()))
        );
        // A busy pane is never taken over: with none finished, the biggest splits however small.
        let busy: Vec<Slot> = tab
            .iter()
            .cloned()
            .map(|slot| Slot {
                finished: false,
                ..slot
            })
            .collect();
        assert_eq!(
            place(&busy),
            Some(Placement::Split {
                pane: "a".into(),
                right: true
            })
        );
        assert_eq!(place(&[]), None);
    }
}
