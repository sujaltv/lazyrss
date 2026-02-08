use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

/// Top-level application configuration.
///
/// Loaded from `$XDG_CONFIG_HOME/lazyrss/config.yaml` (or platform equivalent).
/// If the file does not exist, sensible defaults are used.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// How often (in seconds) to automatically refresh feeds.
    #[serde(default = "default_refresh_every")]
    pub refresh_every: u64,

    /// Whether to refresh all feeds on application startup.
    #[serde(default = "default_refresh_on_start")]
    pub refresh_on_start: bool,

    /// Display-related settings (formatting, column widths).
    #[serde(default)]
    pub display: DisplayConfig,

    /// List of RSS/Atom feed sources - can be standalone feeds or groups.
    #[serde(default)]
    pub feeds: Vec<FeedConfigItem>,

    /// Keyboard keybindings.
    #[serde(default)]
    pub keybindings: KeyBindings,
}

/// Keybinding configuration for all actions.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyBindings {
    /// Global keybindings (work in all panes).
    #[serde(default)]
    pub global: GlobalKeyBindings,

    /// Keybindings specific to the Feeds pane.
    #[serde(default)]
    pub feeds: FeedsKeyBindings,

    /// Keybindings specific to the Articles pane.
    #[serde(default)]
    pub articles: ArticlesKeyBindings,

    /// Keybindings specific to the Article view pane.
    #[serde(default)]
    pub article_view: ArticleViewKeyBindings,
}

/// Global keybindings (work in all panes).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalKeyBindings {
    /// Quit the application.
    #[serde(default = "default_quit")]
    pub quit: Vec<KeyBinding>,

    /// Switch focus to the next pane.
    #[serde(default = "default_focus_next")]
    pub focus_next: Vec<KeyBinding>,

    /// Switch focus to the previous pane.
    #[serde(default = "default_focus_prev")]
    pub focus_prev: Vec<KeyBinding>,

    /// Refresh the current feed.
    #[serde(default = "default_refresh_current")]
    pub refresh_current: KeyBinding,

    /// Refresh all feeds.
    #[serde(default = "default_refresh_all")]
    pub refresh_all: KeyBinding,

    /// Open the selected article in a browser.
    #[serde(default = "default_open_browser")]
    pub open_browser: KeyBinding,

    /// Jump to the top of the list.
    #[serde(default = "default_jump_top")]
    pub jump_top: KeyBinding,

    /// Jump to the bottom of the list.
    #[serde(default = "default_jump_bottom")]
    pub jump_bottom: KeyBinding,

    /// Create a new group.
    #[serde(default = "default_create_group")]
    pub create_group: KeyBinding,

    /// Create a new feed.
    #[serde(default = "default_create_feed")]
    pub create_feed: KeyBinding,
}

/// Keybindings for the Feeds pane.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeedsKeyBindings {
    /// Move selection down.
    #[serde(default = "default_move_down")]
    pub move_down: Vec<KeyBinding>,

    /// Move selection up.
    #[serde(default = "default_move_up")]
    pub move_up: Vec<KeyBinding>,

    /// Select the current feed/group.
    #[serde(default = "default_select")]
    pub select: KeyBinding,

    /// Toggle collapse of the current group.
    #[serde(default = "default_toggle_collapse")]
    pub toggle_collapse: KeyBinding,

    /// Expand all groups.
    #[serde(default = "default_expand_all")]
    pub expand_all: Vec<KeyBinding>,

    /// Collapse all groups.
    #[serde(default = "default_collapse_all")]
    pub collapse_all: Vec<KeyBinding>,

    /// Scroll half-page down.
    #[serde(default = "default_scroll_half_page_down")]
    pub scroll_half_page_down: Vec<KeyBinding>,

    /// Scroll half-page up.
    #[serde(default = "default_scroll_half_page_up")]
    pub scroll_half_page_up: Vec<KeyBinding>,
}

/// Keybindings for the Articles pane.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArticlesKeyBindings {
    /// Move selection down.
    #[serde(default = "default_move_down")]
    pub move_down: Vec<KeyBinding>,

    /// Move selection up.
    #[serde(default = "default_move_up")]
    pub move_up: Vec<KeyBinding>,

    /// Select/open the current article.
    #[serde(default = "default_select")]
    pub select: KeyBinding,

    /// Toggle read status of the current article.
    #[serde(default = "default_toggle_read")]
    pub toggle_read: KeyBinding,

    /// Toggle star status of the current article.
    #[serde(default = "default_toggle_star")]
    pub toggle_star: KeyBinding,

    /// Mark all articles in the current feed as read.
    #[serde(default = "default_mark_all_read")]
    pub mark_all_read: KeyBinding,

    /// Scroll half-page down.
    #[serde(default = "default_scroll_half_page_down")]
    pub scroll_half_page_down: Vec<KeyBinding>,

    /// Scroll half-page up.
    #[serde(default = "default_scroll_half_page_up")]
    pub scroll_half_page_up: Vec<KeyBinding>,
}

/// Keybindings for the Article view pane.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArticleViewKeyBindings {
    /// Scroll content down.
    #[serde(default = "default_scroll_down")]
    pub scroll_down: Vec<KeyBinding>,

    /// Scroll content up.
    #[serde(default = "default_scroll_up")]
    pub scroll_up: Vec<KeyBinding>,

    /// Scroll half-page down.
    #[serde(default = "default_scroll_half_page_down")]
    pub scroll_half_page_down: Vec<KeyBinding>,

    /// Scroll half-page up.
    #[serde(default = "default_scroll_half_page_up")]
    pub scroll_half_page_up: Vec<KeyBinding>,
}

/// A single key binding.
///
/// Can be deserialized from various formats:
/// - `"a"` - a single character
/// - `"Ctrl-a"` - control+character
/// - `"Enter"`, `"Tab"`, `"BackTab"`, `"Esc"`, `"Space"`, etc. - special keys
/// - `"Up"`, `"Down"`, `"Left"`, `"Right"` - arrow keys
/// - `"PageUp"`, `"PageDown"`, `"Home"`, `"End"` - navigation keys
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

// Implement Default for all keybinding structs
impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            global: GlobalKeyBindings::default(),
            feeds: FeedsKeyBindings::default(),
            articles: ArticlesKeyBindings::default(),
            article_view: ArticleViewKeyBindings::default(),
        }
    }
}

impl Default for GlobalKeyBindings {
    fn default() -> Self {
        Self {
            quit: default_quit(),
            focus_next: default_focus_next(),
            focus_prev: default_focus_prev(),
            refresh_current: default_refresh_current(),
            refresh_all: default_refresh_all(),
            open_browser: default_open_browser(),
            jump_top: default_jump_top(),
            jump_bottom: default_jump_bottom(),
            create_group: default_create_group(),
            create_feed: default_create_feed(),
        }
    }
}

impl Default for FeedsKeyBindings {
    fn default() -> Self {
        Self {
            move_down: default_move_down(),
            move_up: default_move_up(),
            select: default_select(),
            toggle_collapse: default_toggle_collapse(),
            expand_all: default_expand_all(),
            collapse_all: default_collapse_all(),
            scroll_half_page_down: default_scroll_half_page_down(),
            scroll_half_page_up: default_scroll_half_page_up(),
        }
    }
}

impl Default for ArticlesKeyBindings {
    fn default() -> Self {
        Self {
            move_down: default_move_down(),
            move_up: default_move_up(),
            select: default_select(),
            toggle_read: default_toggle_read(),
            toggle_star: default_toggle_star(),
            mark_all_read: default_mark_all_read(),
            scroll_half_page_down: default_scroll_half_page_down(),
            scroll_half_page_up: default_scroll_half_page_up(),
        }
    }
}

impl Default for ArticleViewKeyBindings {
    fn default() -> Self {
        Self {
            scroll_down: default_scroll_down(),
            scroll_up: default_scroll_up(),
            scroll_half_page_down: default_scroll_half_page_down(),
            scroll_half_page_up: default_scroll_half_page_up(),
        }
    }
}

// KeyBinding parsing and serialization implementation
mod keybinding_serde {
    use super::*;
    use serde::de::{Error, Visitor};
    use std::fmt;

    // Helper to parse a key binding string
    pub fn parse_keybinding(s: &str) -> Result<KeyBinding, String> {
        let s = s.trim();

        // Special case: Shift-Tab (case-insensitive) should map to BackTab
        if s.to_lowercase() == "shift-tab" {
            return Ok(KeyBinding {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::NONE,
            });
        }

        let (modifiers, key) = if let Some(rest) = s.strip_prefix("Ctrl-") {
            (KeyModifiers::CONTROL, rest)
        } else if let Some(rest) = s.strip_prefix("Alt-") {
            (KeyModifiers::ALT, rest)
        } else if let Some(rest) = s.strip_prefix("Shift-") {
            (KeyModifiers::SHIFT, rest)
        } else {
            (KeyModifiers::NONE, s)
        };

        let code = match key.to_lowercase().as_str() {
            "enter" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "esc" | "escape" => KeyCode::Esc,
            "space" => KeyCode::Char(' '),
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "insert" => KeyCode::Insert,
            "delete" => KeyCode::Delete,
            "f1" => KeyCode::F(1),
            "f2" => KeyCode::F(2),
            "f3" => KeyCode::F(3),
            "f4" => KeyCode::F(4),
            "f5" => KeyCode::F(5),
            "f6" => KeyCode::F(6),
            "f7" => KeyCode::F(7),
            "f8" => KeyCode::F(8),
            "f9" => KeyCode::F(9),
            "f10" => KeyCode::F(10),
            "f11" => KeyCode::F(11),
            "f12" => KeyCode::F(12),
            "null" | "nop" => KeyCode::Null,
            s if s.len() == 1 => KeyCode::Char(key.chars().next().unwrap()),
            _ => return Err(format!("Unknown key: {}", key)),
        };

        // For uppercase letters, add SHIFT modifier
        let modifiers = if key.len() == 1 && key.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
            modifiers | KeyModifiers::SHIFT
        } else {
            modifiers
        };

        Ok(KeyBinding { code, modifiers })
    }

    impl<'de> Deserialize<'de> for KeyBinding {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct KeyBindingVisitor;

            impl<'de> Visitor<'de> for KeyBindingVisitor {
                type Value = KeyBinding;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a key binding string like \"a\", \"Ctrl-a\", \"Enter\", etc.")
                }

                fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                where
                    E: Error,
                {
                    parse_keybinding(s).map_err(Error::custom)
                }
            }

            deserializer.deserialize_str(KeyBindingVisitor)
        }
    }
}

// Implement Serialize for KeyBinding
impl serde::Serialize for KeyBinding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = self.as_string();
        serializer.serialize_str(&s)
    }
}

// Implement matching for KeyBinding
impl KeyBinding {
    pub fn matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        // Special case for BackTab (Shift+Tab):
        // Different terminals report this key differently:
        // - Some: BackTab with NONE modifiers
        // - Some: BackTab with SHIFT modifiers
        // - Some: Tab with SHIFT modifiers
        // When binding expects BackTab with no modifiers, accept all variants.
        if self.code == KeyCode::BackTab && self.modifiers == KeyModifiers::NONE {
            // BackTab key event (with or without SHIFT modifier)
            if code == KeyCode::BackTab
                && (modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT) {
                return true;
            }
            // Tab with SHIFT modifier (some terminals report Shift+Tab this way)
            if code == KeyCode::Tab && modifiers == KeyModifiers::SHIFT {
                return true;
            }
            return false;
        }
        self.code == code && self.modifiers == modifiers
    }

    /// Display this keybinding as a string for hints
    pub fn display(&self) -> String {
        let modifier_str = if self.modifiers.contains(KeyModifiers::CONTROL) {
            "Ctrl+"
        } else if self.modifiers.contains(KeyModifiers::ALT) {
            "Alt+"
        } else if self.modifiers.contains(KeyModifiers::SHIFT) {
            "Shift+"
        } else {
            ""
        };

        let key_str = match self.code {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "Shift+Tab".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::F(n) => format!("F{}", n),
            KeyCode::Null => "␀".to_string(),
            _ => "?".to_string(),
        };

        format!("{}{}", modifier_str, key_str)
    }

    /// Convert this keybinding to a string for serialization
    pub fn as_string(&self) -> String {
        let modifier_str = if self.modifiers.contains(KeyModifiers::CONTROL) {
            "Ctrl-"
        } else if self.modifiers.contains(KeyModifiers::ALT) {
            "Alt-"
        } else if self.modifiers.contains(KeyModifiers::SHIFT) {
            "Shift-"
        } else {
            ""
        };

        let key_str = match self.code {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "BackTab".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::F(n) => format!("F{}", n),
            KeyCode::Null => "Null".to_string(),
            _ => "?".to_string(),
        };

        if modifier_str.is_empty() {
            key_str
        } else {
            format!("{}{}", modifier_str, key_str)
        }
    }
}

// Check if a key matches any in a slice
pub fn matches_any(bindings: &[KeyBinding], code: KeyCode, modifiers: KeyModifiers) -> bool {
    bindings.iter().any(|b| b.matches(code, modifiers))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayConfig {
    /// Formatting options for dates and times.
    #[serde(default)]
    pub format: FormatConfig,

    /// Column width configuration for the TUI layout.
    #[serde(default)]
    pub columns: ColumnConfig,

    /// Color configuration for the UI.
    #[serde(default)]
    pub colours: ColourConfig,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            format: FormatConfig::default(),
            columns: ColumnConfig::default(),
            colours: ColourConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormatConfig {
    /// Hour format: 12 or 24.
    #[serde(default = "default_time_format")]
    pub time: u8,

    /// Date format string for the articles list (e.g. "D MMM YYYY").
    #[serde(default = "default_date_format")]
    pub date: String,

    /// Date format string for the detailed article view (e.g. "dddd, D MMMM YYYY").
    #[serde(default = "default_date_format")]
    pub date_detail: String,

    /// Number of lines for article titles in the articles list (allows wrapping).
    #[serde(default = "default_title_lines")]
    pub title_lines: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColumnConfig {
    /// Width percentage for the feeds list pane.
    #[serde(default = "default_feeds_list_width")]
    pub feeds_list: u16,

    /// Width percentage for the articles list pane.
    #[serde(default = "default_articles_list_width")]
    pub articles_list: u16,

    /// Width percentage for the article view pane.
    #[serde(default = "default_article_view_width")]
    pub article_view: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColourConfig {
    /// Color for the focused pane border.
    #[serde(default = "default_active_border")]
    pub active_border: String,

    /// Color for unfocused pane borders.
    #[serde(default = "default_inactive_border")]
    pub inactive_border: String,

    /// Border type (plain, double, thick, rounded,_quadrant).
    #[serde(default = "default_border_type")]
    pub border_type: String,

    /// Background color for the highlighted article.
    #[serde(default = "default_highlight_bg")]
    pub highlight_bg: String,

    /// Color for the unread indicator (filled dot) and unread counts.
    #[serde(default = "default_unread_indicator")]
    pub unread_indicator: String,
}

impl Default for ColourConfig {
    fn default() -> Self {
        Self {
            active_border: default_active_border(),
            inactive_border: default_inactive_border(),
            border_type: default_border_type(),
            highlight_bg: default_highlight_bg(),
            unread_indicator: default_unread_indicator(),
        }
    }
}

/// Parse a border type string into ratatui::widgets::border::BorderType.
pub fn parse_border_type(border_str: &str) -> Result<ratatui::widgets::BorderType, String> {
    match border_str.to_lowercase().as_str() {
        "plain" => Ok(ratatui::widgets::BorderType::Plain),
        "double" => Ok(ratatui::widgets::BorderType::Double),
        "thick" => Ok(ratatui::widgets::BorderType::Thick),
        "rounded" => Ok(ratatui::widgets::BorderType::Rounded),
        _ => Err(format!(
            "Unknown border type: {}. Valid options: plain, double, thick, rounded",
            border_str
        )),
    }
}

/// Parse a color name into ratatui::Color.
pub fn parse_color(color_str: &str) -> Result<ratatui::style::Color, String> {
    match color_str.to_lowercase().as_str() {
        "black" => Ok(ratatui::style::Color::Black),
        "red" => Ok(ratatui::style::Color::Red),
        "green" => Ok(ratatui::style::Color::Green),
        "yellow" => Ok(ratatui::style::Color::Yellow),
        "blue" => Ok(ratatui::style::Color::Blue),
        "magenta" => Ok(ratatui::style::Color::Magenta),
        "cyan" => Ok(ratatui::style::Color::Cyan),
        "white" => Ok(ratatui::style::Color::White),
        "darkgray" | "dark_grey" | "dark_gray" => Ok(ratatui::style::Color::DarkGray),
        "gray" | "grey" => Ok(ratatui::style::Color::Gray),
        "lightred" | "light_red" => Ok(ratatui::style::Color::LightRed),
        "lightgreen" | "light_green" => Ok(ratatui::style::Color::LightGreen),
        "lightyellow" | "light_yellow" => Ok(ratatui::style::Color::LightYellow),
        "lightblue" | "light_blue" => Ok(ratatui::style::Color::LightBlue),
        "lightmagenta" | "light_magenta" => Ok(ratatui::style::Color::LightMagenta),
        "lightcyan" | "light_cyan" => Ok(ratatui::style::Color::LightCyan),
        "lightwhite" | "light_white" => Ok(ratatui::style::Color::White),
        // Indexed colors (0-255)
        s if s.starts_with('#') => {
            // Try to parse as RGB hex
            let hex = &s[1..];
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                Ok(ratatui::style::Color::Rgb(r, g, b))
            } else {
                Err(format!("Invalid hex color format: {}", color_str))
            }
        }
        _ => Err(format!("Unknown color: {}", color_str)),
    }
}

/// A single feed source within a group.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeedSource {
    pub title: String,
    /// URL to the website (for reference, opening in browser).
    pub url: String,
    /// URL to the RSS/Atom feed (for fetching articles).
    /// If not provided, the `url` field will be used as the feed URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed: Option<String>,
}

/// A named group of feeds (e.g. "Tech", "News (World)").
///
/// Groups can contain both standalone feeds and nested groups.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeedGroup {
    pub title: String,
    pub feeds: Vec<FeedConfigItem>,
}

/// A feed configuration item that can be either a standalone feed or a group of feeds.
///
/// This allows flexible configuration where feeds can be defined standalone or grouped,
/// with support for recursive nesting of groups.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FeedConfigItem {
    /// A standalone feed (not part of any group).
    Standalone(FeedSource),
    /// A group of related feeds (may contain nested groups).
    Group(FeedGroup),
}

/// Collect all empty group paths from the config.
///
/// Empty groups are groups with no feeds (either standalone or nested).
/// This function scans all top-level items and collects paths to empty groups.
pub fn collect_empty_groups_from_config(config: &Config) -> Vec<String> {
    let mut result = Vec::new();
    for item in &config.feeds {
        item.collect_empty_groups_recursive(None, &mut result);
    }
    result
}

impl FeedConfigItem {
    /// Recursively iterate over all feeds, collecting them with their full group path.
    ///
    /// Returns a vector of (group_path, FeedSource) pairs.
    /// - For standalone feeds at top level, group_path is None.
    /// - For feeds in groups, group_path is the full path (e.g., "News (World) > Domestic").
    pub fn collect_feeds(&self) -> Vec<(Option<String>, FeedSource)> {
        let mut result = Vec::new();
        self.collect_feeds_recursive(None, &mut result);
        result
    }

    fn collect_feeds_recursive(&self, current_path: Option<String>, result: &mut Vec<(Option<String>, FeedSource)>) {
        match self {
            FeedConfigItem::Standalone(feed) => {
                result.push((current_path, feed.clone()));
            }
            FeedConfigItem::Group(group) => {
                let new_path = if let Some(ref path) = current_path {
                    format!("{} > {}", path, group.title)
                } else {
                    group.title.clone()
                };

                for item in &group.feeds {
                    item.collect_feeds_recursive(Some(new_path.clone()), result);
                }
            }
        }
    }

    /// Recursively collect all empty group paths.
    ///
    /// Returns a vector of full paths to groups that have no feeds.
    /// Empty groups are those with an empty `feeds` array.
    pub fn collect_empty_groups(&self) -> Vec<String> {
        let mut result = Vec::new();
        self.collect_empty_groups_recursive(None, &mut result);
        result
    }

    fn collect_empty_groups_recursive(&self, current_path: Option<String>, result: &mut Vec<String>) {
        match self {
            FeedConfigItem::Standalone(_feed) => {
                // Standalone feeds are not groups, so they can't be empty groups
            }
            FeedConfigItem::Group(group) => {
                let new_path = if let Some(ref path) = current_path {
                    format!("{} > {}", path, group.title)
                } else {
                    group.title.clone()
                };

                // If this group has no feeds, add it to the result
                if group.feeds.is_empty() {
                    result.push(new_path.clone());
                }

                // Recursively check child groups for empty ones
                for item in &group.feeds {
                    item.collect_empty_groups_recursive(Some(new_path.clone()), result);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

fn default_refresh_every() -> u64 {
    300
}

fn default_refresh_on_start() -> bool {
    true
}

fn default_time_format() -> u8 {
    12
}

fn default_date_format() -> String {
    "D MMM YYYY".to_string()
}

fn default_title_lines() -> u8 {
    2
}

fn default_feeds_list_width() -> u16 {
    25
}

fn default_articles_list_width() -> u16 {
    35
}

fn default_article_view_width() -> u16 {
    40
}

fn default_active_border() -> String {
    "cyan".to_string()
}

fn default_inactive_border() -> String {
    "darkgray".to_string()
}

fn default_border_type() -> String {
    "plain".to_string()
}

fn default_highlight_bg() -> String {
    "darkgray".to_string()
}

fn default_unread_indicator() -> String {
    "cyan".to_string()
}

// Keybinding defaults
fn parse_kb(s: &str) -> KeyBinding {
    keybinding_serde::parse_keybinding(s).unwrap()
}

fn default_quit() -> Vec<KeyBinding> {
    vec![parse_kb("q"), parse_kb("Ctrl-c")]
}

fn default_focus_next() -> Vec<KeyBinding> {
    vec![parse_kb("Tab")]
}

fn default_focus_prev() -> Vec<KeyBinding> {
    vec![parse_kb("BackTab")]
}

fn default_move_down() -> Vec<KeyBinding> {
    vec![parse_kb("j"), parse_kb("Down")]
}

fn default_move_up() -> Vec<KeyBinding> {
    vec![parse_kb("k"), parse_kb("Up")]
}

fn default_select() -> KeyBinding {
    parse_kb("Enter")
}

fn default_toggle_collapse() -> KeyBinding {
    parse_kb("Space")
}

fn default_expand_all() -> Vec<KeyBinding> {
    vec![parse_kb("e")]
}

fn default_collapse_all() -> Vec<KeyBinding> {
    vec![parse_kb("E")]
}

fn default_toggle_read() -> KeyBinding {
    parse_kb("m")
}

fn default_toggle_star() -> KeyBinding {
    parse_kb("s")
}

fn default_mark_all_read() -> KeyBinding {
    parse_kb("M")
}

fn default_scroll_down() -> Vec<KeyBinding> {
    vec![parse_kb("j"), parse_kb("Down")]
}

fn default_scroll_up() -> Vec<KeyBinding> {
    vec![parse_kb("k"), parse_kb("Up")]
}

fn default_scroll_half_page_down() -> Vec<KeyBinding> {
    vec![parse_kb("Ctrl-d"), parse_kb("PageDown")]
}

fn default_scroll_half_page_up() -> Vec<KeyBinding> {
    vec![parse_kb("Ctrl-u"), parse_kb("PageUp")]
}

fn default_refresh_current() -> KeyBinding {
    parse_kb("r")
}

fn default_refresh_all() -> KeyBinding {
    parse_kb("R")
}

fn default_open_browser() -> KeyBinding {
    parse_kb("o")
}

fn default_jump_top() -> KeyBinding {
    parse_kb("g")
}

fn default_jump_bottom() -> KeyBinding {
    parse_kb("G")
}

fn default_create_group() -> KeyBinding {
    parse_kb("Ctrl-g")
}

fn default_create_feed() -> KeyBinding {
    parse_kb("Ctrl-n")
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_every: default_refresh_every(),
            refresh_on_start: default_refresh_on_start(),
            display: DisplayConfig::default(),
            feeds: Vec::new(),
            keybindings: KeyBindings::default(),
        }
    }
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            time: default_time_format(),
            date: default_date_format(),
            date_detail: default_date_format(),
            title_lines: default_title_lines(),
        }
    }
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            feeds_list: default_feeds_list_width(),
            articles_list: default_articles_list_width(),
            article_view: default_article_view_width(),
        }
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Returns the path to the config file:
/// `$XDG_CONFIG_HOME/lazyrss/config.yaml` (or platform equivalent).
fn config_path() -> anyhow::Result<PathBuf> {
    let dir = dirs::config_dir().context("Could not determine config directory")?;
    Ok(dir.join("lazyrss").join("config.yaml"))
}

/// Load application configuration from disk.
///
/// If the config file does not exist, returns `Config::default()`.
/// If the file exists but cannot be parsed, the parse error is propagated.
pub fn load() -> anyhow::Result<Config> {
    let path = config_path()?;

    if !path.exists() {
        return Ok(Config::default());
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let config: Config = serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

    Ok(config)
}

/// Save application configuration to disk.
///
/// Creates the config directory if it doesn't exist, and writes the config
/// to the config file as YAML. Uses atomic write (temp file + rename) to
/// prevent corruption.
///
/// Note: This function rewrites the entire config file, which may change
/// formatting. Consider using `save_feeds_only()` for feed-related changes
/// to better preserve formatting.
pub fn save(config: &Config) -> anyhow::Result<()> {
    let path = config_path()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }

    // Serialize config to YAML
    let yaml = serde_yaml::to_string(config)
        .context("Failed to serialize config to YAML")?;

    // Write to temp file first, then rename for atomic write
    let temp_path = path.with_extension("yaml.tmp");
    fs::write(&temp_path, yaml)
        .with_context(|| format!("Failed to write config file: {}", temp_path.display()))?;

    // Atomic rename
    fs::rename(&temp_path, &path)
        .with_context(|| format!("Failed to rename config file: {} -> {}", temp_path.display(), path.display()))?;

    Ok(())
}

/// Save only the feeds section while preserving the rest of the config formatting.
///
/// This function reads the existing YAML file, modifies only the feeds section,
/// and writes it back, preserving most of the original formatting.
///
/// Note: This function uses serde_yaml which may reformat the feeds section,
/// but preserves the structure and content of other sections.
pub fn save_feeds_only(feeds: &[FeedConfigItem]) -> anyhow::Result<()> {
    let path = config_path()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }

    // Read existing config if it exists
    let mut yaml_value = if path.exists() {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        serde_yaml::from_str::<Value>(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?
    } else {
        // Add default config structure if file doesn't exist
        return save(&Config { feeds: feeds.to_vec(), ..Default::default() });
    };

    // Convert feeds to YAML value
    let feeds_value = serde_yaml::to_value(feeds)
        .context("Failed to serialize feeds to YAML")?;

    // Update the feeds section
    if let Value::Mapping(ref mut map) = yaml_value {
        map.insert(Value::String("feeds".to_string()), feeds_value);
    }

    // Write to temp file first, then rename for atomic write
    let temp_path = path.with_extension("yaml.tmp");
    let yaml_string = serde_yaml::to_string(&yaml_value)
        .context("Failed to serialize config to YAML")?;
    fs::write(&temp_path, yaml_string)
        .with_context(|| format!("Failed to write config file: {}", temp_path.display()))?;

    // Atomic rename
    fs::rename(&temp_path, &path)
        .with_context(|| format!("Failed to rename config file: {} -> {}", temp_path.display(), path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let cfg = Config::default();
        assert_eq!(cfg.refresh_every, 300);
        assert_eq!(cfg.refresh_on_start, true);
        assert_eq!(cfg.display.format.time, 12);
        assert_eq!(cfg.display.format.date, "D MMM YYYY");
        assert_eq!(cfg.display.format.date_detail, "D MMM YYYY");
        assert_eq!(cfg.display.columns.feeds_list, 25);
        assert_eq!(cfg.display.columns.articles_list, 35);
        assert_eq!(cfg.display.columns.article_view, 40);
        assert!(cfg.feeds.is_empty());
        // Check keybindings have defaults
        assert!(!cfg.keybindings.global.quit.is_empty());
        assert!(!cfg.keybindings.global.focus_next.is_empty());
    }

    #[test]
    fn deserialize_partial_yaml_uses_defaults() {
        let yaml = "refresh_every: 60\n";
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.refresh_every, 60);
        assert_eq!(cfg.display.format.time, 12);
        assert!(cfg.feeds.is_empty());
    }

    #[test]
    fn deserialize_full_yaml() {
        let yaml = r#"
refresh_every: 120
display:
  format:
    time: 24
    date: "YYYY-MM-DD"
  columns:
    feeds_list: 30
    articles_list: 40
    article_view: 30
feeds:
  - title: "Tech"
    feeds:
      - title: "Rust Blog"
        url: "https://blog.rust-lang.org/"
        feed: "https://blog.rust-lang.org/feed.xml"
keybindings:
  global:
    quit: ["q"]
    open_browser: "o"
  feeds:
    select: "Enter"
    toggle_collapse: "Space"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.refresh_every, 120);
        assert_eq!(cfg.display.format.time, 24);
        assert_eq!(cfg.display.format.date, "YYYY-MM-DD");
        assert_eq!(cfg.display.columns.feeds_list, 30);
        assert_eq!(cfg.feeds.len(), 1);
        match &cfg.feeds[0] {
            FeedConfigItem::Group(group) => {
                assert_eq!(group.title, "Tech");
                assert_eq!(group.feeds.len(), 1);
                match &group.feeds[0] {
                    FeedConfigItem::Standalone(feed) => {
                        assert_eq!(feed.url, "https://blog.rust-lang.org/");
                        assert_eq!(feed.feed, Some("https://blog.rust-lang.org/feed.xml".to_string()));
                    }
                    _ => panic!("Expected FeedConfigItem::Standalone"),
                }
            }
            _ => panic!("Expected FeedConfigItem::Group"),
        }
        assert_eq!(cfg.keybindings.global.quit.len(), 1);
        assert_eq!(cfg.keybindings.feeds.select.code, KeyCode::Enter);
    }

    #[test]
    fn deserialize_standalone_feed() {
        let yaml = r#"
feeds:
  - title: "BAIR"
    url: "http://bair.berkeley.edu/blog/"
    feed: "https://bair.berkeley.edu/blog/feed.xml"
  - title: "News (World)"
    feeds:
      - title: "BBC World News"
        url: "https://www.bbc.co.uk/news/"
        feed: "http://feeds.bbci.co.uk/news/world/rss.xml"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.feeds.len(), 2);

        // First item is standalone
        match &cfg.feeds[0] {
            FeedConfigItem::Standalone(feed) => {
                assert_eq!(feed.title, "BAIR");
                assert_eq!(feed.url, "http://bair.berkeley.edu/blog/");
                assert_eq!(feed.feed, Some("https://bair.berkeley.edu/blog/feed.xml".to_string()));
            }
            _ => panic!("Expected FeedConfigItem::Standalone"),
        }

        // Second item is a group
        match &cfg.feeds[1] {
            FeedConfigItem::Group(group) => {
                assert_eq!(group.title, "News (World)");
                assert_eq!(group.feeds.len(), 1);
                match &group.feeds[0] {
                    FeedConfigItem::Standalone(feed) => {
                        assert_eq!(feed.title, "BBC World News");
                    }
                    _ => panic!("Expected FeedConfigItem::Standalone"),
                }
            }
            _ => panic!("Expected FeedConfigItem::Group"),
        }
    }

    #[test]
    fn feed_config_item_iter_feeds() {
        let standalone = FeedConfigItem::Standalone(FeedSource {
            title: "BAIR".to_string(),
            url: "http://bair.berkeley.edu/blog/".to_string(),
            feed: Some("https://bair.berkeley.edu/blog/feed.xml".to_string()),
        });

        let feeds = standalone.collect_feeds();
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].0, None);
        assert_eq!(feeds[0].1.title, "BAIR");

        let group = FeedConfigItem::Group(FeedGroup {
            title: "Tech".to_string(),
            feeds: vec![
                FeedConfigItem::Standalone(FeedSource {
                    title: "Rust Blog".to_string(),
                    url: "https://blog.rust-lang.org/".to_string(),
                    feed: Some("https://blog.rust-lang.org/feed.xml".to_string()),
                }),
            ],
        });

        let feeds = group.collect_feeds();
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].0, Some("Tech".to_string()));
        assert_eq!(feeds[0].1.title, "Rust Blog");
    }

    #[test]
    fn feed_config_item_nested_groups() {
        // News (World) > Domestic > BBC World News
        let nested = FeedConfigItem::Group(FeedGroup {
            title: "News (World)".to_string(),
            feeds: vec![
                FeedConfigItem::Standalone(FeedSource {
                    title: "Foreign Policy".to_string(),
                    url: "https://foreignpolicy.com".to_string(),
                    feed: Some("http://foreignpolicy.com/feed".to_string()),
                }),
                FeedConfigItem::Group(FeedGroup {
                    title: "Domestic".to_string(),
                    feeds: vec![
                        FeedConfigItem::Standalone(FeedSource {
                            title: "BBC World News".to_string(),
                            url: "https://www.bbc.co.uk/news/".to_string(),
                            feed: Some("http://feeds.bbci.co.uk/news/world/rss.xml".to_string()),
                        }),
                    ],
                }),
            ],
        });

        let feeds = nested.collect_feeds();
        assert_eq!(feeds.len(), 2);

        // Foreign Policy is directly under "News (World)"
        assert_eq!(feeds[0].0, Some("News (World)".to_string()));
        assert_eq!(feeds[0].1.title, "Foreign Policy");

        // BBC World News is under "News (World) > Domestic"
        assert_eq!(feeds[1].0, Some("News (World) > Domestic".to_string()));
        assert_eq!(feeds[1].1.title, "BBC World News");
    }

    #[test]
    fn collect_empty_groups_from_single_group() {
        let empty_group = FeedConfigItem::Group(FeedGroup {
            title: "Empty Group".to_string(),
            feeds: vec![],
        });

        let empty = empty_group.collect_empty_groups();
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0], "Empty Group");
    }

    #[test]
    fn collect_empty_groups_from_nested_groups() {
        let nested = FeedConfigItem::Group(FeedGroup {
            title: "News".to_string(),
            feeds: vec![
                FeedConfigItem::Group(FeedGroup {
                    title: "Tech".to_string(),
                    feeds: vec![],
                }),
                FeedConfigItem::Group(FeedGroup {
                    title: "Sports".to_string(),
                    feeds: vec![
                        FeedConfigItem::Group(FeedGroup {
                            title: "Football".to_string(),
                            feeds: vec![],
                        }),
                    ],
                }),
            ],
        });

        let empty = nested.collect_empty_groups();
        assert_eq!(empty.len(), 2);
        assert_eq!(empty[0], "News > Tech");
        assert_eq!(empty[1], "News > Sports > Football");
    }

    #[test]
    fn test_collect_empty_groups_from_config() {
        let config = Config {
            feeds: vec![
                FeedConfigItem::Standalone(FeedSource {
                    title: "BAIR".to_string(),
                    url: "http://bair.berkeley.edu/blog/".to_string(),
                    feed: Some("https://bair.berkeley.edu/blog/feed.xml".to_string()),
                }),
                FeedConfigItem::Group(FeedGroup {
                    title: "Tech".to_string(),
                    feeds: vec![
                        FeedConfigItem::Standalone(FeedSource {
                            title: "Rust Blog".to_string(),
                            url: "https://blog.rust-lang.org/".to_string(),
                            feed: Some("https://blog.rust-lang.org/feed.xml".to_string()),
                        }),
                    ],
                }),
                FeedConfigItem::Group(FeedGroup {
                    title: "Empty Group".to_string(),
                    feeds: vec![],
                }),
            ],
            ..Default::default()
        };

        let empty = collect_empty_groups_from_config(&config);
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0], "Empty Group");
    }

    #[test]
    fn collect_empty_groups_does_not_include_non_empty_groups() {
        let group = FeedConfigItem::Group(FeedGroup {
            title: "Tech".to_string(),
            feeds: vec![
                FeedConfigItem::Standalone(FeedSource {
                    title: "Rust Blog".to_string(),
                    url: "https://blog.rust-lang.org/".to_string(),
                    feed: Some("https://blog.rust-lang.org/feed.xml".to_string()),
                }),
            ],
        });

        let empty = group.collect_empty_groups();
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn parse_keybinding_single_char() {
        let kb = parse_kb("a");
        assert_eq!(kb.code, KeyCode::Char('a'));
        assert_eq!(kb.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn parse_keybinding_ctrl_char() {
        let kb = parse_kb("Ctrl-a");
        assert_eq!(kb.code, KeyCode::Char('a'));
        assert!(kb.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn parse_keybinding_special() {
        let kb = parse_kb("Enter");
        assert_eq!(kb.code, KeyCode::Enter);
        assert_eq!(kb.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn keybinding_matches() {
        let kb = parse_kb("Ctrl-a");
        assert!(kb.matches(KeyCode::Char('a'), KeyModifiers::CONTROL));
        assert!(!kb.matches(KeyCode::Char('a'), KeyModifiers::NONE));
    }

    #[test]
    fn keybinding_display() {
        assert_eq!(parse_kb("a").display(), "a");
        assert_eq!(parse_kb("Ctrl-a").display(), "Ctrl+a");
        assert_eq!(parse_kb("Enter").display(), "Enter");
        assert_eq!(parse_kb("Up").display(), "↑");
    }

    #[test]
    fn parse_keybinding_uppercase_adds_shift() {
        let kb = parse_kb("G");
        assert_eq!(kb.code, KeyCode::Char('G'));
        assert!(kb.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn uppercase_keybinding_matches_shift_key() {
        let kb = parse_kb("G");
        assert!(kb.matches(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert!(!kb.matches(KeyCode::Char('G'), KeyModifiers::NONE));
    }

    #[test]
    fn parse_shift_tab_as_backtab() {
        let kb = parse_kb("Shift-Tab");
        assert_eq!(kb.code, KeyCode::BackTab);
        assert_eq!(kb.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn parse_backtab_directly() {
        let kb = parse_kb("BackTab");
        assert_eq!(kb.code, KeyCode::BackTab);
        assert_eq!(kb.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn backtab_matches_actual_key_event() {
        let kb = parse_kb("BackTab");
        assert!(kb.matches(KeyCode::BackTab, KeyModifiers::NONE));
    }

    #[test]
    fn backtab_matches_with_shift_modifier() {
        // Some terminals report BackTab with SHIFT modifier
        let kb = parse_kb("BackTab");
        assert!(kb.matches(KeyCode::BackTab, KeyModifiers::SHIFT));
    }

    #[test]
    fn tab_with_shift_matches_backtab_binding() {
        // Some terminals report Shift+Tab as Tab with SHIFT modifier
        let kb = parse_kb("BackTab");
        assert!(kb.matches(KeyCode::Tab, KeyModifiers::SHIFT));
    }
}
