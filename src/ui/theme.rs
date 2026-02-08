use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::BorderType;

use crate::config::ColourConfig;

/// Border style for the currently focused pane.
pub const ACTIVE_BORDER: Style = Style::new().fg(Color::Cyan);

/// Border style for unfocused panes.
pub const INACTIVE_BORDER: Style = Style::new().fg(Color::DarkGray);

/// Get border style based on whether the pane is focused and the colour config.
pub fn get_border_style(is_focused: bool, colours: &ColourConfig) -> Style {
    let color_str = if is_focused {
        &colours.active_border
    } else {
        &colours.inactive_border
    };

    let color = crate::config::parse_color(color_str).unwrap_or_else(|_| {
        if is_focused { Color::Cyan } else { Color::DarkGray }
    });

    Style::new().fg(color)
}

/// Get highlight style for the currently selected row based on the colour config.
pub fn get_highlight_style(colours: &ColourConfig) -> Style {
    let color = crate::config::parse_color(&colours.highlight_bg)
        .unwrap_or(Color::DarkGray);

    Style::new()
        .bg(color)
        .add_modifier(Modifier::BOLD)
}

/// Get the unread indicator style based on the colour config.
pub fn get_unread_indicator_style(colours: &ColourConfig) -> Style {
    let color = crate::config::parse_color(&colours.unread_indicator)
        .unwrap_or(Color::Cyan);

    Style::new().fg(color)
}

/// Get border type based on the colour config.
pub fn get_border_type(colours: &ColourConfig) -> BorderType {
    crate::config::parse_border_type(&colours.border_type)
        .unwrap_or(BorderType::Plain)
}

/// Style for group headers and section titles.
pub const HEADER_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);

/// Highlight style for the currently selected row in a list.
pub const SELECTED_STYLE: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);

/// Style for feeds/articles that have unread content.
pub const UNREAD_STYLE: Style = Style::new()
    .fg(Color::White)
    .add_modifier(Modifier::BOLD);

/// Style for feeds/articles that have been fully read.
pub const READ_STYLE: Style = Style::new().fg(Color::DarkGray);

/// Style for the star indicator on starred articles.
pub const STAR_STYLE: Style = Style::new().fg(Color::Yellow);

/// Style for unread-count badges.
pub const COUNT_STYLE: Style = Style::new().fg(Color::Cyan);

/// Style for the article title in the article view pane.
pub const TITLE_STYLE: Style = Style::new()
    .fg(Color::Green)
    .add_modifier(Modifier::BOLD);

/// Style for metadata lines (author, date, etc.) in the article view.
pub const META_STYLE: Style = Style::new().fg(Color::DarkGray);

/// Background style for the bottom status bar.
pub const STATUS_STYLE: Style = Style::new().fg(Color::White).bg(Color::DarkGray);
