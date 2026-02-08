use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::ActivePane;
use crate::config::{self, KeyBinding};
use crate::event::Event;

/// High-level actions that the application can perform in response to user
/// input.  The event handler maps raw key events to these actions based on
/// which pane is currently focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    FocusNext,
    FocusPrev,
    MoveUp,
    MoveDown,
    Select,
    ToggleRead,
    ToggleStar,
    OpenInBrowser,
    MarkAllRead,
    ScrollUp,
    ScrollDown,
    ScrollHalfPageUp,
    ScrollHalfPageDown,
    RefreshAll,
    RefreshCurrent,
    ToggleCollapse,
    ToggleCollapseRecursive,
    JumpToTop,
    JumpToBottom,
    ExpandAllGroups,
    CollapseAllGroups,
    ToggleAllGroups,
    CreateGroup,
    CreateFeed,
    Delete,
    Cut,
    Paste,
    Edit,
    Digit(u8),  // 0-9 for vim-style count prefix
}

/// Map a raw terminal [`Event`] to an application [`Action`], considering which
/// pane is currently active and the configured keybindings.
///
/// Returns `None` for events that have no associated action (e.g. mouse events,
/// ticks, or unmapped keys).
pub fn handle_event(
    event: &Event,
    active_pane: ActivePane,
    keybindings: &config::KeyBindings,
) -> Option<Action> {
    let Event::Key(key) = event else {
        return None;
    };

    let code = key.code;
    let mods = key.modifiers;

    // ----- Global bindings (independent of pane) -----

    // Quit
    if config::matches_any(&keybindings.global.quit, code, mods) {
        return Some(Action::Quit);
    }

    // Focus navigation
    if config::matches_any(&keybindings.global.focus_next, code, mods) {
        return Some(Action::FocusNext);
    }
    if config::matches_any(&keybindings.global.focus_prev, code, mods) {
        return Some(Action::FocusPrev);
    }

    // Refresh
    if keybindings.global.refresh_current.matches(code, mods) {
        return Some(Action::RefreshCurrent);
    }
    if keybindings.global.refresh_all.matches(code, mods) {
        return Some(Action::RefreshAll);
    }

    // Open in browser (all panes)
    if keybindings.global.open_browser.matches(code, mods) {
        return Some(Action::OpenInBrowser);
    }

    // Jump to top / bottom (all panes)
    if keybindings.global.jump_top.matches(code, mods) {
        return Some(Action::JumpToTop);
    }
    if keybindings.global.jump_bottom.matches(code, mods) {
        return Some(Action::JumpToBottom);
    }

    // Create group (all panes)
    if keybindings.global.create_group.matches(code, mods) {
        return Some(Action::CreateGroup);
    }

    // Create feed (all panes)
    if keybindings.global.create_feed.matches(code, mods) {
        return Some(Action::CreateFeed);
    }

    // Delete (Shift+d or D) - only in feeds pane
    if (code == KeyCode::Char('d') || code == KeyCode::Char('D'))
        && mods == KeyModifiers::SHIFT
        && active_pane == ActivePane::Feeds {
        return Some(Action::Delete);
    }

    // Cut (x) - only in feeds pane
    if code == KeyCode::Char('x')
        && mods == KeyModifiers::NONE
        && active_pane == ActivePane::Feeds {
        return Some(Action::Cut);
    }

    // Paste (p) - only in feeds pane
    if code == KeyCode::Char('p')
        && mods == KeyModifiers::NONE
        && active_pane == ActivePane::Feeds {
        return Some(Action::Paste);
    }

    // Edit (Ctrl+e) - only in feeds pane
    if code == KeyCode::Char('e')
        && mods == KeyModifiers::CONTROL
        && active_pane == ActivePane::Feeds {
        return Some(Action::Edit);
    }

    // Digit input for vim-style count prefix (works in any pane)
    if mods == KeyModifiers::NONE {
        match code {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                return Some(Action::Digit(c as u8 - b'0'));
            }
            _ => {}
        }
    }

    // ----- Pane-specific bindings -----

    match active_pane {
        ActivePane::Feeds => handle_feeds_key(code, mods, keybindings),
        ActivePane::Articles => handle_articles_key(code, mods, keybindings),
        ActivePane::ArticleView => handle_article_view_key(code, mods, keybindings),
    }
}

/// Key bindings when the Feeds list pane is focused.
fn handle_feeds_key(
    code: KeyCode,
    mods: KeyModifiers,
    keybindings: &config::KeyBindings,
) -> Option<Action> {
    let kb = &keybindings.feeds;

    if config::matches_any(&kb.move_down, code, mods) {
        return Some(Action::MoveDown);
    }
    if config::matches_any(&kb.move_up, code, mods) {
        return Some(Action::MoveUp);
    }
    if kb.select.matches(code, mods) {
        return Some(Action::Select);
    }

    // Shift+Space: recursive mark all read
    if mods.contains(KeyModifiers::ALT) && code == KeyCode::Char(' ') {
        return Some(Action::ToggleCollapseRecursive);
    }

     if kb.toggle_collapse.matches(code, mods) {
         return Some(Action::ToggleCollapse);
     }

    // Check if the key matches both expand_all and collapse_all
    let matches_expand = config::matches_any(&kb.expand_all, code, mods);
    let matches_collapse = config::matches_any(&kb.collapse_all, code, mods);

    if matches_expand && matches_collapse {
        // Same key used for both - toggle based on current state
        return Some(Action::ToggleAllGroups);
    }
    if matches_expand {
        return Some(Action::ExpandAllGroups);
    }
    if matches_collapse {
        return Some(Action::CollapseAllGroups);
    }

    if config::matches_any(&kb.scroll_half_page_down, code, mods) {
        return Some(Action::ScrollHalfPageDown);
    }
    if config::matches_any(&kb.scroll_half_page_up, code, mods) {
        return Some(Action::ScrollHalfPageUp);
    }

    None
}

/// Key bindings when the Articles list pane is focused.
fn handle_articles_key(
    code: KeyCode,
    mods: KeyModifiers,
    keybindings: &config::KeyBindings,
) -> Option<Action> {
    let kb = &keybindings.articles;

    if config::matches_any(&kb.move_down, code, mods) {
        return Some(Action::MoveDown);
    }
    if config::matches_any(&kb.move_up, code, mods) {
        return Some(Action::MoveUp);
    }
    if kb.select.matches(code, mods) {
        return Some(Action::Select);
    }
    if kb.toggle_read.matches(code, mods) {
        return Some(Action::ToggleRead);
    }
    if kb.toggle_star.matches(code, mods) {
        return Some(Action::ToggleStar);
    }
    if kb.mark_all_read.matches(code, mods) {
        return Some(Action::MarkAllRead);
    }
    if config::matches_any(&kb.scroll_half_page_down, code, mods) {
        return Some(Action::ScrollHalfPageDown);
    }
    if config::matches_any(&kb.scroll_half_page_up, code, mods) {
        return Some(Action::ScrollHalfPageUp);
    }

    None
}

/// Key bindings when the Article view (reading) pane is focused.
fn handle_article_view_key(
    code: KeyCode,
    mods: KeyModifiers,
    keybindings: &config::KeyBindings,
) -> Option<Action> {
    let kb = &keybindings.article_view;

    // In the reading pane j/k and arrows scroll the content rather than
    // moving the selection cursor.
    if config::matches_any(&kb.scroll_down, code, mods) {
        return Some(Action::ScrollDown);
    }
    if config::matches_any(&kb.scroll_up, code, mods) {
        return Some(Action::ScrollUp);
    }
    if config::matches_any(&kb.scroll_half_page_down, code, mods) {
        return Some(Action::ScrollHalfPageDown);
    }
    if config::matches_any(&kb.scroll_half_page_up, code, mods) {
        return Some(Action::ScrollHalfPageUp);
    }

    None
}

/// Build a display string for a list of keybindings.
/// If there are multiple bindings, join them with "/".
pub fn format_bindings(bindings: &[KeyBinding]) -> String {
    bindings
        .iter()
        .map(|kb| kb.display())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeyBindings;

    #[test]
    fn default_keybindings_quit_on_q() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::Quit));
    }

    #[test]
    fn default_keybindings_quit_on_ctrl_c() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::Quit));
    }

    #[test]
    fn feeds_pane_move_down_on_j() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::MoveDown));
    }

    #[test]
    fn feeds_pane_toggle_collapse_on_space() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::ToggleCollapse));
    }

    #[test]
    fn articles_pane_toggle_read_on_m() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('m'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Articles, &kb);
        assert_eq!(action, Some(Action::ToggleRead));
    }

    #[test]
    fn article_view_scroll_down_on_j() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::ArticleView, &kb);
        assert_eq!(action, Some(Action::ScrollDown));
    }

    #[test]
    fn format_single_binding() {
        let kb = KeyBindings::default();
        assert_eq!(kb.feeds.select.display(), "Enter");
    }

    #[test]
    fn format_multiple_bindings() {
        let kb = KeyBindings::default();
        assert_eq!(format_bindings(&kb.feeds.move_down), "j/↓");
    }

    #[test]
    fn jump_to_top_on_g() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::JumpToTop));
    }

    #[test]
    fn jump_to_bottom_on_g() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('G'),
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::JumpToBottom));
    }

    #[test]
    fn jump_works_in_articles_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Articles, &kb);
        assert_eq!(action, Some(Action::JumpToTop));
    }

    #[test]
    fn jump_works_in_article_view_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('G'),
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::ArticleView, &kb);
        assert_eq!(action, Some(Action::JumpToBottom));
    }

    #[test]
    fn backtab_triggers_focus_prev() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Articles, &kb);
        assert_eq!(action, Some(Action::FocusPrev));
    }

    #[test]
    fn create_group_on_ctrl_g() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::CreateGroup));
    }

    #[test]
    fn create_feed_on_ctrl_n() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::CreateFeed));
    }

    #[test]
    fn delete_on_shift_d_in_feeds_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('D'),
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::Delete));
    }

    #[test]
    fn delete_on_lowercase_shift_d_in_feeds_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::Delete));
    }

    #[test]
    fn delete_not_triggered_in_articles_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('D'),
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Articles, &kb);
        assert_ne!(action, Some(Action::Delete));
    }

    #[test]
    fn delete_not_triggered_without_shift() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_ne!(action, Some(Action::Delete));
    }

    #[test]
    fn cut_on_x_in_feeds_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::Cut));
    }

    #[test]
    fn cut_not_triggered_in_articles_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Articles, &kb);
        assert_ne!(action, Some(Action::Cut));
    }

    #[test]
    fn paste_on_p_in_feeds_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::Paste));
    }

    #[test]
    fn paste_not_triggered_in_articles_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Articles, &kb);
        assert_ne!(action, Some(Action::Paste));
    }

    #[test]
    fn edit_on_ctrl_e_in_feeds_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_eq!(action, Some(Action::Edit));
    }

    #[test]
    fn edit_not_triggered_in_articles_pane() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::CONTROL,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Articles, &kb);
        assert_ne!(action, Some(Action::Edit));
    }

    #[test]
    fn edit_not_triggered_without_control() {
        let kb = KeyBindings::default();
        let event = Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        });
        let action = handle_event(&event, ActivePane::Feeds, &kb);
        assert_ne!(action, Some(Action::Edit));
    }
}
