use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Render an alacritty terminal directly into a ratatui Buffer at the given area.
/// No intermediate allocations — writes cell by cell from the grid's display iterator.
pub fn render_term<T>(term: &alacritty_terminal::term::Term<T>, buf: &mut Buffer, area: Rect) {
    let grid = term.grid();
    let display_offset = grid.display_offset();
    let num_cols = grid.columns().min(area.width as usize);
    let num_rows = grid.screen_lines().min(area.height as usize);

    // display_iter starts at Line(-(display_offset) - 1) and advances.
    // First visible viewport row corresponds to Line(-(display_offset as i32)).
    let viewport_top = -(display_offset as i32);

    for indexed in grid.display_iter() {
        let grid_line = indexed.point.line.0;
        let col = indexed.point.column.0;

        // Convert grid line to viewport row (0-based)
        let row = (grid_line - viewport_top) as usize;

        if row >= num_rows || col >= num_cols {
            continue;
        }

        let x = area.x + col as u16;
        let y = area.y + row as u16;

        if x >= area.x + area.width || y >= area.y + area.height {
            continue;
        }

        let cell = indexed.cell;
        let style = cell_style(cell);
        let buf_cell = &mut buf[(x, y)];
        buf_cell.set_char(cell.c);
        buf_cell.set_style(style);
    }

    // Draw the cursor as a reversed block on top of whatever cell it sits on.
    // Only render when SHOW_CURSOR mode is set and the cursor is in the viewport
    // (i.e. the user hasn't scrolled away into history).
    if term.mode().contains(TermMode::SHOW_CURSOR) {
        let cursor_point = grid.cursor.point;
        let cursor_row = (cursor_point.line.0 - viewport_top) as isize;
        let cursor_col = cursor_point.column.0 as isize;
        if cursor_row >= 0
            && (cursor_row as usize) < num_rows
            && cursor_col >= 0
            && (cursor_col as usize) < num_cols
        {
            let x = area.x + cursor_col as u16;
            let y = area.y + cursor_row as u16;
            if x < area.x + area.width && y < area.y + area.height {
                let buf_cell = &mut buf[(x, y)];
                // If the underlying cell is blank, show a visible block.
                if buf_cell.symbol().is_empty() || buf_cell.symbol() == " " {
                    buf_cell.set_char(' ');
                }
                let existing = buf_cell.style();
                buf_cell.set_style(existing.add_modifier(Modifier::REVERSED));
            }
        }
    }
}

/// Get the total number of lines (history + screen) for scroll calculations.
pub fn total_lines<T>(term: &alacritty_terminal::term::Term<T>) -> usize {
    let grid = term.grid();
    grid.history_size() + grid.screen_lines()
}

/// Get the current display offset (how far scrolled back into history).
pub fn display_offset<T>(term: &alacritty_terminal::term::Term<T>) -> usize {
    term.grid().display_offset()
}

fn cell_style(cell: &Cell) -> Style {
    let mut style = Style::default();

    style = style.fg(convert_color(cell.fg));
    style = style.bg(convert_color(cell.bg));

    let mut modifier = Modifier::empty();
    if cell.flags.contains(Flags::BOLD) {
        modifier |= Modifier::BOLD;
    }
    if cell.flags.contains(Flags::ITALIC) {
        modifier |= Modifier::ITALIC;
    }
    if cell.flags.intersects(Flags::ALL_UNDERLINES) {
        modifier |= Modifier::UNDERLINED;
    }
    if cell.flags.contains(Flags::INVERSE) {
        modifier |= Modifier::REVERSED;
    }
    if cell.flags.contains(Flags::DIM) {
        modifier |= Modifier::DIM;
    }
    if cell.flags.contains(Flags::HIDDEN) {
        modifier |= Modifier::HIDDEN;
    }
    if cell.flags.contains(Flags::STRIKEOUT) {
        modifier |= Modifier::CROSSED_OUT;
    }

    style.add_modifier(modifier)
}

fn convert_color(color: AnsiColor) -> Color {
    match color {
        AnsiColor::Named(named) => convert_named_color(named),
        AnsiColor::Spec(rgb) => Color::Rgb(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(i) => Color::Indexed(i),
    }
}

fn convert_named_color(color: NamedColor) -> Color {
    match color {
        NamedColor::Black => Color::Black,
        NamedColor::Red => Color::Red,
        NamedColor::Green => Color::Green,
        NamedColor::Yellow => Color::Yellow,
        NamedColor::Blue => Color::Blue,
        NamedColor::Magenta => Color::Magenta,
        NamedColor::Cyan => Color::Cyan,
        NamedColor::White => Color::White,
        NamedColor::BrightBlack => Color::DarkGray,
        NamedColor::BrightRed => Color::LightRed,
        NamedColor::BrightGreen => Color::LightGreen,
        NamedColor::BrightYellow => Color::LightYellow,
        NamedColor::BrightBlue => Color::LightBlue,
        NamedColor::BrightMagenta => Color::LightMagenta,
        NamedColor::BrightCyan => Color::LightCyan,
        NamedColor::BrightWhite => Color::White,
        _ => Color::Reset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_named_colors() {
        assert_eq!(convert_named_color(NamedColor::Black), Color::Black);
        assert_eq!(convert_named_color(NamedColor::Red), Color::Red);
        assert_eq!(convert_named_color(NamedColor::Green), Color::Green);
        assert_eq!(convert_named_color(NamedColor::Yellow), Color::Yellow);
        assert_eq!(convert_named_color(NamedColor::Blue), Color::Blue);
        assert_eq!(convert_named_color(NamedColor::Magenta), Color::Magenta);
        assert_eq!(convert_named_color(NamedColor::Cyan), Color::Cyan);
        assert_eq!(convert_named_color(NamedColor::White), Color::White);
    }

    #[test]
    fn convert_bright_colors() {
        assert_eq!(
            convert_named_color(NamedColor::BrightBlack),
            Color::DarkGray
        );
        assert_eq!(convert_named_color(NamedColor::BrightRed), Color::LightRed);
        assert_eq!(
            convert_named_color(NamedColor::BrightGreen),
            Color::LightGreen
        );
        assert_eq!(
            convert_named_color(NamedColor::BrightYellow),
            Color::LightYellow
        );
        assert_eq!(
            convert_named_color(NamedColor::BrightBlue),
            Color::LightBlue
        );
        assert_eq!(
            convert_named_color(NamedColor::BrightMagenta),
            Color::LightMagenta
        );
        assert_eq!(
            convert_named_color(NamedColor::BrightCyan),
            Color::LightCyan
        );
        assert_eq!(convert_named_color(NamedColor::BrightWhite), Color::White);
    }

    #[test]
    fn convert_fallback_color() {
        assert_eq!(convert_named_color(NamedColor::Foreground), Color::Reset);
        assert_eq!(convert_named_color(NamedColor::Background), Color::Reset);
    }

    #[test]
    fn convert_spec_color() {
        let rgb = alacritty_terminal::vte::ansi::Rgb {
            r: 255,
            g: 128,
            b: 0,
        };
        assert_eq!(convert_color(AnsiColor::Spec(rgb)), Color::Rgb(255, 128, 0));
    }

    #[test]
    fn convert_indexed_color() {
        assert_eq!(convert_color(AnsiColor::Indexed(42)), Color::Indexed(42));
        assert_eq!(convert_color(AnsiColor::Indexed(0)), Color::Indexed(0));
        assert_eq!(convert_color(AnsiColor::Indexed(255)), Color::Indexed(255));
    }

    #[test]
    fn convert_named_via_convert_color() {
        assert_eq!(convert_color(AnsiColor::Named(NamedColor::Red)), Color::Red);
    }

    #[test]
    fn cell_style_plain() {
        let cell = Cell::default();
        let style = cell_style(&cell);
        assert_eq!(style.add_modifier, Modifier::empty());
    }

    #[test]
    fn cell_style_with_flags() {
        let mut cell = Cell::default();
        cell.flags = Flags::BOLD | Flags::ITALIC;
        let style = cell_style(&cell);
        assert!(style.add_modifier.contains(Modifier::BOLD));
        assert!(style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn cell_style_underline() {
        let mut cell = Cell::default();
        cell.flags = Flags::UNDERLINE;
        let style = cell_style(&cell);
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn cell_style_inverse() {
        let mut cell = Cell::default();
        cell.flags = Flags::INVERSE;
        let style = cell_style(&cell);
        assert!(style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn cell_style_dim_hidden_strikeout() {
        let mut cell = Cell::default();
        cell.flags = Flags::DIM | Flags::HIDDEN | Flags::STRIKEOUT;
        let style = cell_style(&cell);
        assert!(style.add_modifier.contains(Modifier::DIM));
        assert!(style.add_modifier.contains(Modifier::HIDDEN));
        assert!(style.add_modifier.contains(Modifier::CROSSED_OUT));
    }
}
