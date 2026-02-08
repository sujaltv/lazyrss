use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::action;
use crate::app::{ActivePane, App};
use crate::ui::theme;

/// Render the single-row status bar at the bottom of the terminal.
///
/// Shows either a status message (if set), or contextual key-binding hints
/// for the currently active pane.  A "Refreshing..." prefix is prepended
/// while a background refresh is in progress.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let content = if let Some(ref msg) = app.status_message {
        format!(" {msg}")
    } else {
        let hints = build_hints(app);
        if app.is_refreshing {
            format!(" Refreshing... \u{2502}{hints}")
        } else {
            hints
        }
    };

    let bar = Paragraph::new(content).style(theme::STATUS_STYLE);
    frame.render_widget(bar, area);
}

/// Build keybinding hints for the currently active pane.
fn build_hints(app: &App) -> String {
    let kb = &app.config.keybindings;
    match app.active_pane {
        ActivePane::Feeds => build_feeds_hints(kb),
        ActivePane::Articles => build_articles_hints(kb),
        ActivePane::ArticleView => build_article_view_hints(kb),
    }
}

/// Build keybinding hints for the feeds pane.
fn build_feeds_hints(kb: &crate::config::KeyBindings) -> String {
    let parts = vec![
        format!("[{}] Navigate", action::format_bindings(&kb.feeds.move_down)),
        format!("[{}] Select", kb.feeds.select.display()),
        format!("[{}] Collapse", kb.feeds.toggle_collapse.display()),
        format!("[{}] Cut", "x"),
        format!("[{}] Paste", "p"),
        format!("[{}] Delete", "D"),
        format!("[{}] Jump", action::format_bindings(&[kb.global.jump_top.clone(), kb.global.jump_bottom.clone()])),
        format!("[{}] Page", action::format_bindings(&kb.feeds.scroll_half_page_down)),
        format!("[{}]/[{}] Pane", action::format_bindings(&kb.global.focus_prev), action::format_bindings(&kb.global.focus_next)),
        format!("[{}] Refresh", kb.global.refresh_all.display()),
        format!("[{}] Quit", action::format_bindings(&kb.global.quit)),
    ];
    parts.join(" \u{2502} ")
}

/// Build keybinding hints for the articles pane.
fn build_articles_hints(kb: &crate::config::KeyBindings) -> String {
    let parts = vec![
        format!("[{}] Navigate", action::format_bindings(&kb.articles.move_down)),
        format!("[{}] Read", kb.articles.select.display()),
        format!("[{}] Read/Unread", kb.articles.toggle_read.display()),
        format!("[{}] Star", kb.articles.toggle_star.display()),
        format!("[{}] Jump", action::format_bindings(&[kb.global.jump_top.clone(), kb.global.jump_bottom.clone()])),
        format!("[{}] Page", action::format_bindings(&kb.articles.scroll_half_page_down)),
        format!("[{}] Open", kb.global.open_browser.display()),
        format!("[{}]/[{}] Pane", action::format_bindings(&kb.global.focus_prev), action::format_bindings(&kb.global.focus_next)),
        format!("[{}] Quit", action::format_bindings(&kb.global.quit)),
    ];
    parts.join(" \u{2502} ")
}

/// Build keybinding hints for the article view pane.
fn build_article_view_hints(kb: &crate::config::KeyBindings) -> String {
    let parts = vec![
        format!("[{}] Scroll", action::format_bindings(&kb.article_view.scroll_down)),
        format!("[{}] Page", action::format_bindings(&kb.article_view.scroll_half_page_down)),
        format!("[{}] Jump", action::format_bindings(&[kb.global.jump_top.clone(), kb.global.jump_bottom.clone()])),
        format!("[{}] Open", kb.global.open_browser.display()),
        format!("[{}]/[{}] Pane", action::format_bindings(&kb.global.focus_prev), action::format_bindings(&kb.global.focus_next)),
        format!("[{}] Quit", action::format_bindings(&kb.global.quit)),
    ];
    parts.join(" \u{2502} ")
}

#[cfg(test)]
mod tests {
    use crate::config::KeyBinding;

    #[test]
    fn format_keybinding_single() {
        let kb = KeyBinding {
            code: crossterm::event::KeyCode::Char('q'),
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        assert_eq!(kb.display(), "q");
    }

    #[test]
    fn format_keybinding_ctrl() {
        let kb = KeyBinding {
            code: crossterm::event::KeyCode::Char('d'),
            modifiers: crossterm::event::KeyModifiers::CONTROL,
        };
        assert_eq!(kb.display(), "Ctrl+d");
    }
}
