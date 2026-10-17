//! Nirvana fork: the sidebar's brand row, as Nirvana's sidebar header draws it (the owner's Image #11,
//! 2026-10-15): the mark, "Nirvana", a version badge, and the sidebar toggle at the right edge.
//!
//! The badge is the running build (`crate::nirvana::VERSION`). When the server reports a newer
//! installed build (herdr's `update_available`, fed by `crate::nirvana::check_installed`), the badge
//! becomes a reload button: `↻ <version>`, then `restart?` after one press, and a second press
//! reloads. Drawn in the client only: it is TUI presentation (AGENTS.md runtime/client boundary).

use super::*;
use ratatui::style::Color;

/// Nirvana's teal (`--accent` in Nirvana's dark theme), the mark's colour in the owner's image.
pub(super) const NIRVANA_TEAL: Color = Color::Rgb(0x2A, 0x8C, 0x96);
pub(super) const MARK: &str = "◆";

/// Draws the brand row on `row` and records the toggle and reload hits.
pub(super) fn render_brand_row(
    buffer: &mut Buffer,
    row: Rect,
    update_available: Option<&str>,
    reload_armed: bool,
    palette: &Palette,
    hits: &mut ShellHitMap,
) {
    hits.nirvana_reload = Rect::default();
    if row.is_empty() {
        hits.sidebar_toggle = Rect::default();
        return;
    }
    let toggle_x = row.right().saturating_sub(1);
    let right = toggle_x;
    let mut x = row.x;
    x = super::render::put_segment(buffer, x, row.y, right, " ", Style::default());
    x = super::render::put_segment(
        buffer,
        x,
        row.y,
        right,
        MARK,
        Style::default()
            .fg(NIRVANA_TEAL)
            .add_modifier(Modifier::BOLD),
    );
    // No space after the name: the badge starts with its own. With one more, " alpha 0.10.0 " (two-digit minor)
    // needed 25 columns beside the toggle and lost its closing pad (0.10.0, seen in the 0.11.0 live check).
    x = super::render::put_segment(
        buffer,
        x,
        row.y,
        right,
        " Nirvana",
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD),
    );
    let (badge, style) = match update_available {
        Some(_) if reload_armed => (
            " restart? ".to_string(),
            Style::default()
                .fg(Color::White)
                .bg(palette.red)
                .add_modifier(Modifier::BOLD),
        ),
        Some(version) => (
            format!(" ↻ {version} "),
            Style::default()
                .fg(Color::White)
                .bg(NIRVANA_TEAL)
                .add_modifier(Modifier::BOLD),
        ),
        None => (
            format!(" {} ", crate::nirvana::label(crate::nirvana::VERSION)),
            Style::default().fg(palette.overlay1).bg(palette.surface0),
        ),
    };
    let start = x;
    let end = super::render::put_segment(buffer, x, row.y, right, &badge, style);
    if update_available.is_some() && end > start {
        hits.nirvana_reload = Rect::new(start, row.y, end - start, 1);
    }
    hits.sidebar_toggle = Rect::new(toggle_x, row.y, 1, 1);
    super::render::put_text(
        buffer,
        toggle_x,
        row.y,
        1,
        "«",
        Style::default().fg(palette.overlay0),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (buffer.area.x..buffer.area.right())
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    }

    fn draw(width: u16, update: Option<&str>, armed: bool) -> (Buffer, ShellHitMap) {
        let area = Rect::new(0, 0, width, 1);
        let mut buffer = Buffer::empty(area);
        let mut hits = ShellHitMap::default();
        render_brand_row(
            &mut buffer,
            area,
            update,
            armed,
            &Palette::catppuccin(),
            &mut hits,
        );
        (buffer, hits)
    }

    #[test]
    fn brand_row_names_nirvana_and_its_running_version() {
        // 25 columns: the default 26-column sidebar minus its divider.
        let (buffer, hits) = draw(25, None, false);
        let text = row_text(&buffer, 0);
        let expected = format!(
            " ◆ Nirvana {} ",
            crate::nirvana::label(crate::nirvana::VERSION)
        );
        assert!(text.starts_with(&expected), "{text:?}");
        assert_eq!(buffer[(1, 0)].fg, NIRVANA_TEAL);
        // The badge keeps its closing pad right up to the toggle (live run 2026-10-15: one column
        // short, "alpha 0.1.1" touched the toggle).
        assert_eq!(buffer[(23, 0)].bg, Palette::catppuccin().surface0);
        assert_eq!(hits.sidebar_toggle, Rect::new(24, 0, 1, 1));
        assert_eq!(buffer[(24, 0)].symbol(), "«");
        assert_eq!(hits.nirvana_reload, Rect::default());
    }

    #[test]
    fn a_newer_build_turns_the_badge_into_a_reload_button() {
        let (buffer, hits) = draw(25, Some("0.1.1"), false);
        let text = row_text(&buffer, 0);
        assert!(text.contains(" ↻ 0.1.1 "), "{text:?}");
        assert!(!text.contains("alpha"), "{text:?}");
        assert_eq!(hits.nirvana_reload.y, 0);
        assert!(hits.nirvana_reload.width >= 7);
        assert_eq!(buffer[(hits.nirvana_reload.x, 0)].bg, NIRVANA_TEAL);

        let (buffer, hits) = draw(25, Some("0.1.1"), true);
        assert!(row_text(&buffer, 0).contains(" restart? "));
        assert!(hits.nirvana_reload.width > 0);
    }

    #[test]
    fn a_narrow_sidebar_keeps_the_toggle_and_clips_the_badge() {
        let (buffer, hits) = draw(12, None, false);
        assert_eq!(hits.sidebar_toggle, Rect::new(11, 0, 1, 1));
        assert_eq!(buffer[(11, 0)].symbol(), "«");
        assert!(row_text(&buffer, 0).starts_with(" ◆ Nirvana"));
    }
}
