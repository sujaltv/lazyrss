pub mod article_pane;
pub mod articles_pane;
pub mod feeds_pane;
pub mod popup;
pub mod status_bar;
pub mod theme;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

use crate::app::App;

/// Top-level render function.
///
/// Splits the terminal frame into a main content area (fills remaining space)
/// and a 1-row status bar at the bottom.  The main area is then split
/// horizontally into three panes (feeds, articles, article view) whose widths
/// are driven by the percentages in the user's config.
pub fn render(frame: &mut Frame, app: &mut App) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(app.config.display.columns.feeds_list),
            Constraint::Percentage(app.config.display.columns.articles_list),
            Constraint::Percentage(app.config.display.columns.article_view),
        ])
        .split(vertical[0]);

    feeds_pane::render(frame, app, horizontal[0]);
    articles_pane::render(frame, app, horizontal[1]);
    article_pane::render(frame, app, horizontal[2]);
    status_bar::render(frame, app, vertical[1]);

    // Render popup if active
    if let Some(ref popup) = app.popup {
        popup::render_popup(frame, popup);
    }
}
