use std::collections::HashSet;
use std::marker::PhantomData;

use ratatui::widgets::ListState;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::action::Action;
use crate::config::{Config, FeedConfigItem, FeedGroup, FeedSource};
use crate::db;
use crate::db_async::AsyncDb;
use crate::feed::{self, FeedUpdateResult};

/// Convert human-friendly date format to strftime format.
///
/// Public for use by UI modules.
///
/// Converts format specifiers like "D MMM YYYY" to "%d %b %Y"
/// for use with chrono::DateTime::format().
///
/// Supported conversions:
/// - ddd -> abbreviated weekday name (Mon, Tue, Wed) -> %a
/// - dddd -> full weekday name (Monday, Tuesday) -> %A
/// - D (non-zero-padded day) or DD (zero-padded day) -> %d
/// - MMM -> abbreviated month name -> %b
/// - MMMM -> full month name -> %B
/// - YY -> 2-digit year -> %y
/// - YYYY -> 4-digit year -> %Y
///
/// Returns the strftime format and whether the day should be non-zero-padded.
pub fn to_strftime_format(format: &str) -> (String, bool) {
    // Check if using "D" (single) which means non-zero-padded day
    let has_single_d = format.contains("D") && !format.contains("DD");

    let strftime_format = format
        .replace("dddd", "%A")
        .replace("ddd", "%a")
        .replace("YYYY", "%Y")
        .replace("YY", "%y")
        .replace("MMMM", "%B")
        .replace("MMM", "%b")
        .replace("DD", "%d")
        .replace("D", "%d");

    (strftime_format, has_single_d)
}

/// Remove leading zero from the day portion of a formatted date string.
///
/// This handles cases where the format uses "D" (non-zero-padded day)
/// but chrono only provides "%d" (zero-padded). We strip the leading zero
/// from patterns like " 03" or leading "03".
pub fn strip_day_leading_zero(formatted: &str) -> String {
    // Match " 0X" or leading "0X" where X is a digit, replace with " X" or "X"
    // We need to be careful to only match the day, not other parts of the string
    let mut result = String::with_capacity(formatted.len());

    let mut chars = formatted.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ' ' || c == '\n' {
            // Check if next chars are "0" followed by a digit
            if let Some(&'0') = chars.peek() {
                let mut temp = chars.clone();
                temp.next();
                if let Some(&next_char) = temp.peek() {
                    if next_char.is_ascii_digit() {
                        // Skip the zero, keep the space
                        result.push(c);
                        chars.next(); // consume '0'
                        continue;
                    }
                }
            }
            result.push(c);
        } else if c == '0' && result.is_empty() {
            // Check if this is a leading "0X" pattern
            if let Some(&next_char) = chars.peek() {
                if next_char.is_ascii_digit() {
                    // Skip the zero
                    continue;
                }
            }
            result.push(c);
        } else {
            result.push(c);
        }
    }

    result
}

/// Which pane currently has focus in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    Feeds,
    Articles,
    ArticleView,
}

/// A single row in the feeds list -- either the "All" option, a collapsible
/// group header, or an individual feed.
#[derive(Debug)]
pub enum FeedListItem {
    /// Shows all articles from all feeds.
    All { unread_count: u32 },
    /// A collapsible group header.
    GroupHeader { title: String, full_path: String, collapsed: bool, unread_count: u32, depth: u8 },
    /// An individual feed.
    Feed { feed: db::Feed, depth: u8 },
}

/// Result of an async database operation.
#[derive(Debug)]
pub enum DbResult {
    /// All feeds have been loaded.
    FeedsLoaded(Vec<db::Feed>),
    /// Articles for a feed have been loaded.
    ArticlesLoaded { feed_id: i64, articles: Vec<db::Article> },
    /// Articles for a group have been loaded.
    GroupArticlesLoaded { group_title: String, articles: Vec<db::Article> },
    /// All articles have been loaded.
    AllArticlesLoaded(Vec<db::Article>),
    /// An article's read status was toggled.
    ReadToggled { article_id: i64, new_value: bool },
    /// An article's starred status was toggled.
    StarToggled { article_id: i64, new_value: bool },
    /// Articles were marked as read.
    MarkedRead { feed_id: Option<i64> },
}

/// Result of async article content rendering.
#[derive(Debug)]
pub struct RenderResult {
    /// The rendered plain text content.
    pub content: String,
}

/// Clipboard item for cut/paste operations.
///
/// Stores a feed or group that has been cut and is waiting to be pasted.
#[derive(Debug, Clone)]
pub enum ClipboardItem {
    /// A standalone feed that was cut.
    Feed {
        /// The feed source configuration.
        feed_source: FeedSource,
        /// The original group path (for restoring if needed).
        original_group: Option<String>,
    },
    /// A group (with all its nested feeds and subgroups) that was cut.
    Group {
        /// The full path of the group.
        original_path: String,
        /// The group configuration with all its contents.
        group: FeedGroup,
    },
}

/// Top-level application state.
///
/// Holds everything the TUI needs to render and the event loop needs to
/// drive the application.
pub struct App {
    /// When `true` the main loop will exit.
    pub should_quit: bool,
    /// Which pane currently has keyboard focus.
    pub active_pane: ActivePane,
    /// Flat list of items shown in the feeds pane (groups + feeds).
    pub feed_list_items: Vec<FeedListItem>,
    /// Articles for the currently selected feed.
    pub articles: Vec<db::Article>,
    /// Pre-rendered plain-text content of the selected article.
    pub article_content: String,
    /// ID of the currently selected article (for preserving selection across refreshes).
    selected_article_id: Option<i64>,
    /// Selection state for the feeds list widget.
    pub feeds_state: ListState,
    /// Selection state for the articles list widget.
    pub articles_state: ListState,
    /// Vertical scroll offset for the article view pane.
    pub article_scroll: u16,
    /// Number of lines in the current article content.
    pub article_content_lines: u16,
    /// User configuration (column widths, refresh interval, etc.).
    pub config: Config,
    /// Whether a background feed refresh is currently in progress.
    pub is_refreshing: bool,
    /// An optional status message to display in the status bar.
    pub status_message: Option<String>,

    /// Pending count prefix for vim-style navigation (e.g., "10j" moves down 10).
    pub pending_count: Option<u32>,

    /// Optional active popup modal.
    pub popup: Option<crate::ui::popup::Popup>,

    /// Clipboard for cut items (feeds or groups).
    pub clipboard: Option<ClipboardItem>,

    // -- Private fields --
    /// Async database wrapper.
    db: AsyncDb,
    /// All feeds loaded from the database.
    feeds: Vec<db::Feed>,
    /// Group titles whose feed entries are currently hidden.
    collapsed_groups: HashSet<String>,
    /// Empty groups (groups with no feeds) that should still be displayed.
    empty_groups: Vec<String>,
    /// Sender half of the channel used by background feed-fetch tasks.
    feed_update_tx: UnboundedSender<FeedUpdateResult>,
    /// Sender half of the channel for async database results.
    db_result_tx: UnboundedSender<DbResult>,
    /// Sender half of the channel for async render results.
    render_tx: UnboundedSender<RenderResult>,
    /// Number of outstanding background feed-fetch tasks.
    pending_refreshes: usize,
    /// When true, skip reloading articles after feeds load (used for count-only updates).
    skip_articles_reload_after_feeds_load: bool,
    /// Whether to trigger refresh after initial feeds are loaded.
    refresh_on_startup_pending: bool,
    /// Phantom data to make the struct Send + Sync despite having UnboundedSender
    _phantom: PhantomData<*const ()>,
}

// ---------------------------------------------------------------------
// Group tree structure for hierarchical feed display
// ---------------------------------------------------------------------

/// A node in the group tree for hierarchical display.
#[derive(Debug, Clone)]
struct GroupNode {
    /// The display title of this group (last component of the path).
    title: String,
    /// The full path for collapse tracking (e.g., "News (World) > Domestic").
    full_path: String,
    /// Total unread count including all nested children.
    unread_count: u32,
    /// Direct feeds in this group (not in nested subgroups).
    feeds: Vec<db::Feed>,
    /// Nested child groups.
    children: Vec<GroupNode>,
}

impl GroupNode {
    /// Calculate total unread count recursively.
    fn calculate_unread_count(&self) -> u32 {
        let direct_feeds_count: u32 = self.feeds.iter().map(|f| f.unread_count).sum();
        let children_count: u32 = self.children.iter().map(|c| c.calculate_unread_count()).sum();
        direct_feeds_count + children_count
    }

    /// Update unread counts for this node and all children.
    fn update_unread_counts(&mut self) {
        self.unread_count = self.calculate_unread_count();
        for child in &mut self.children {
            child.update_unread_counts();
        }
    }
}

/// Build a hierarchical group tree from flat feed data and empty groups.
///
/// Parses group paths (e.g., "News (World) > Domestic") and builds
/// a tree structure with proper nesting and unread counts.
///
/// Includes empty groups (groups with no feeds) in the tree structure.
fn build_group_tree(feeds: &[db::Feed], empty_groups: &[String]) -> Vec<GroupNode> {
    use std::collections::HashMap;

    // Group feeds by their full group path
    let mut path_to_feeds: HashMap<String, Vec<db::Feed>> = HashMap::new();
    for feed in feeds {
        if !feed.group_title.is_empty() {
            path_to_feeds
                .entry(feed.group_title.clone())
                .or_insert_with(Vec::new)
                .push(feed.clone());
        }
    }

    // Collect all unique paths and their components
    let mut all_paths: Vec<String> = path_to_feeds.keys().cloned().collect();

    // Add empty groups to the paths
    for empty_group in empty_groups {
        if !all_paths.contains(empty_group) {
            all_paths.push(empty_group.clone());
        }
    }

    all_paths.sort();

    if all_paths.is_empty() {
        return Vec::new();
    }

    // Build the tree structure
    let mut root_nodes: Vec<GroupNode> = Vec::new();

    for path in &all_paths {
        let components: Vec<&str> = path.split(" > ").collect();
        let feeds = path_to_feeds.get(path).cloned().unwrap_or_default();

        // Build or traverse the tree
        let current_depth = 0;
        insert_into_tree(&mut root_nodes, &components, path, feeds, current_depth);
    }

    // Calculate unread counts recursively
    for node in &mut root_nodes {
        node.update_unread_counts();
    }

    root_nodes
}

/// Insert a path into the tree, creating nodes as needed.
fn insert_into_tree(
    nodes: &mut Vec<GroupNode>,
    components: &[&str],
    full_path: &str,
    feeds: Vec<db::Feed>,
    depth: usize,
) {
    if depth >= components.len() {
        return;
    }

    let current_title = components[depth].to_string();
    let is_last_component = depth == components.len() - 1;

    // Find if this level already exists
    let existing_pos = nodes.iter().position(|n| n.title == current_title);

    if let Some(pos) = existing_pos {
        // Node exists, update it
        if is_last_component {
            // This is the final component, add feeds directly
            nodes[pos].feeds.extend(feeds);
        } else {
            // Continue traversing down
            insert_into_tree(&mut nodes[pos].children, components, full_path, feeds, depth + 1);
        }
    } else {
        // Need to create a new node
        if is_last_component {
            // This is the final component - store the feeds directly
            let new_node = GroupNode {
                title: current_title,
                full_path: full_path.to_string(),
                unread_count: 0, // Will be calculated later
                feeds,
                children: Vec::new(),
            };
            nodes.push(new_node);
        } else {
            // Intermediate node - build path incrementally, no feeds at this level
            let partial_path: String = components[..=depth].join(" > ");
            let mut new_node = GroupNode {
                title: current_title,
                full_path: partial_path,
                unread_count: 0, // Will be calculated later
                feeds: Vec::new(),
                children: Vec::new(),
            };
            // Continue building the tree for children
            insert_into_tree(&mut new_node.children, components, full_path, feeds, depth + 1);
            nodes.push(new_node);
        }
    }
}

impl App {
    /// Create a new `App` and return it along with the receivers.
    ///
    /// This version returns the receivers separately to avoid borrow checker
    /// issues in the main event loop.
    pub fn new_with_receivers(
        config: Config,
        db: AsyncDb,
    ) -> (
        Self,
        UnboundedReceiver<FeedUpdateResult>,
        UnboundedReceiver<DbResult>,
        UnboundedReceiver<RenderResult>,
    ) {
        let (feed_update_tx, feed_update_rx) = mpsc::unbounded_channel();
        let (db_result_tx, db_result_rx) = mpsc::unbounded_channel();
        let (render_tx, render_rx) = mpsc::unbounded_channel();

        // Extract refresh_on_start before config is moved into app
        let refresh_on_startup_pending = config.refresh_on_start;

        // Initialize empty groups from config
        let empty_groups = crate::config::collect_empty_groups_from_config(&config);

        let mut app = Self {
            should_quit: false,
            active_pane: ActivePane::Articles,
            feed_list_items: Vec::new(),
            articles: Vec::new(),
            article_content: String::new(),
            selected_article_id: None,
            feeds_state: ListState::default(),
            articles_state: ListState::default(),
            article_scroll: 0,
            article_content_lines: 0,
            config,
            is_refreshing: refresh_on_startup_pending, // Show "Refreshing..." on start if configured
            status_message: None,
            pending_count: None,
            popup: None,
            clipboard: None,
            db,
            feeds: Vec::new(),
            collapsed_groups: HashSet::new(),
            empty_groups,
            feed_update_tx,
            db_result_tx,
            render_tx,
            pending_refreshes: 0,
            skip_articles_reload_after_feeds_load: false,
            refresh_on_startup_pending,
            _phantom: PhantomData,
        };

        // Trigger initial async feed load
        app.start_reload_feeds();

        // Set up initial placeholder state
        app.feed_list_items.push(FeedListItem::All { unread_count: 0 });
        app.feeds_state.select(Some(0));
        app.articles = Vec::new();
        app.articles_state.select(None);

        (app, feed_update_rx, db_result_rx, render_rx)
    }

    // ---------------------------------------------------------------------
    // Public accessors for the senders
    // ---------------------------------------------------------------------

    /// Return a reference to the feed-update sender (for cloning into tasks).
    pub fn feed_update_tx(&self) -> &UnboundedSender<FeedUpdateResult> {
        &self.feed_update_tx
    }

    // ---------------------------------------------------------------------
    // Async result handling
    // ---------------------------------------------------------------------

    /// Process a completed async database operation result.
    pub fn handle_db_result(&mut self, result: DbResult) {
        match result {
            DbResult::FeedsLoaded(feeds) => {
                self.feeds = feeds;
                self.build_feed_list_items();
                // Check if we should trigger startup refresh after feeds are loaded
                if self.refresh_on_startup_pending {
                    self.refresh_on_startup_pending = false;
                    self.start_refresh_all();
                }
                // Reload articles for the current selection if needed
                // Skip if we're just updating feed counts (e.g., after marking article as read)
                if self.skip_articles_reload_after_feeds_load {
                    self.skip_articles_reload_after_feeds_load = false;
                } else {
                    self.load_articles_for_current_selection();
                }
            }
            DbResult::ArticlesLoaded { feed_id, articles } => {
                // Only update if we're still viewing this feed
                if self.selected_feed().map(|f| f.id) == Some(feed_id) {
                    // Save the current selected article ID to restore it after refresh
                    let prev_selected_id = self.articles_state.selected()
                        .and_then(|idx| self.articles.get(idx))
                        .map(|a| a.id);

                    self.articles = articles;

                    // Try to restore the previous selection
                    let restored_idx = prev_selected_id
                        .and_then(|id| self.articles.iter().position(|a| a.id == id));

                    if self.articles.is_empty() {
                        self.articles_state.select(None);
                        self.selected_article_id = None;
                    } else if let Some(idx) = restored_idx {
                        self.articles_state.select(Some(idx));
                        self.selected_article_id = prev_selected_id;
                        // Always render when restoring selection
                        self.article_scroll = 0;
                        self.start_render_article_content();
                    } else {
                        // Previous article not found (or first load), select first
                        self.articles_state.select(Some(0));
                        let new_article_id = self.articles.first().map(|a| a.id);
                        // Render if this is a new feed (prev_selected_id was None) or different article
                        if prev_selected_id.is_none() || prev_selected_id != new_article_id {
                            if let Some(article) = self.articles.first() {
                                self.selected_article_id = new_article_id;
                                if !article.is_read {
                                    self.start_toggle_read(article.id);
                                }
                            }
                            self.article_scroll = 0;
                            self.start_render_article_content();
                        }
                    }
                }
            }
            DbResult::GroupArticlesLoaded { group_title, articles } => {
                // Only update if we're still viewing this group
                let still_viewing = self.feeds_state.selected()
                    .and_then(|idx| self.feed_list_items.get(idx))
                    .map(|item| matches!(item, FeedListItem::GroupHeader { full_path, .. } if *full_path == group_title))
                    .unwrap_or(false);

                if still_viewing {
                    // Save the current selected article ID to restore it after refresh
                    let prev_selected_id = self.articles_state.selected()
                        .and_then(|idx| self.articles.get(idx))
                        .map(|a| a.id);

                    self.articles = articles;

                    // Try to restore the previous selection
                    let restored_idx = prev_selected_id
                        .and_then(|id| self.articles.iter().position(|a| a.id == id));

                    if self.articles.is_empty() {
                        self.articles_state.select(None);
                        self.selected_article_id = None;
                    } else if let Some(idx) = restored_idx {
                        self.articles_state.select(Some(idx));
                        self.selected_article_id = prev_selected_id;
                        // Always render when restoring selection
                        self.article_scroll = 0;
                        self.start_render_article_content();
                    } else {
                        // Previous article not found (or first load), select first
                        self.articles_state.select(Some(0));
                        let new_article_id = self.articles.first().map(|a| a.id);
                        // Render if this is a new feed (prev_selected_id was None) or different article
                        if prev_selected_id.is_none() || prev_selected_id != new_article_id {
                            if let Some(article) = self.articles.first() {
                                self.selected_article_id = new_article_id;
                                if !article.is_read {
                                    self.start_toggle_read(article.id);
                                }
                            }
                            self.article_scroll = 0;
                            self.start_render_article_content();
                        }
                    }
                }
            }
            DbResult::AllArticlesLoaded(articles) => {
                // Only update if we're still viewing "All"
                let still_viewing_all = self.feeds_state.selected()
                    .and_then(|idx| self.feed_list_items.get(idx))
                    .map(|item| matches!(item, FeedListItem::All { .. }))
                    .unwrap_or(false);

                if still_viewing_all {
                    // Save the current selected article ID to restore it after refresh
                    let prev_selected_id = self.articles_state.selected()
                        .and_then(|idx| self.articles.get(idx))
                        .map(|a| a.id);

                    self.articles = articles;

                    // Try to restore the previous selection
                    let restored_idx = prev_selected_id
                        .and_then(|id| self.articles.iter().position(|a| a.id == id));

                    if self.articles.is_empty() {
                        self.articles_state.select(None);
                        self.selected_article_id = None;
                    } else if let Some(idx) = restored_idx {
                        self.articles_state.select(Some(idx));
                        self.selected_article_id = prev_selected_id;
                        // Always render when restoring selection
                        self.article_scroll = 0;
                        self.start_render_article_content();
                    } else {
                        // Previous article not found (or first load), select first
                        self.articles_state.select(Some(0));
                        let new_article_id = self.articles.first().map(|a| a.id);
                        // Render if this is a new feed (prev_selected_id was None) or different article
                        if prev_selected_id.is_none() || prev_selected_id != new_article_id {
                            if let Some(article) = self.articles.first() {
                                self.selected_article_id = new_article_id;
                                if !article.is_read {
                                    self.start_toggle_read(article.id);
                                }
                            }
                            self.article_scroll = 0;
                            self.start_render_article_content();
                        }
                    }
                }
            }
            DbResult::ReadToggled { article_id, new_value } => {
                if let Some(article) = self.articles.iter_mut().find(|a| a.id == article_id) {
                    article.is_read = new_value;
                }
                // Reload feeds to update unread counts, but don't reload articles
                self.skip_articles_reload_after_feeds_load = true;
                self.start_reload_feeds();
            }
            DbResult::StarToggled { article_id, new_value } => {
                if let Some(article) = self.articles.iter_mut().find(|a| a.id == article_id) {
                    article.is_starred = new_value;
                }
            }
            DbResult::MarkedRead { feed_id } => {
                // Reload the current article list
                match feed_id {
                    Some(id) => {
                        if self.selected_feed().map(|f| f.id) == Some(id) {
                            self.start_load_articles_for_feed(id);
                        }
                    }
                    None => {
                        // "All" was selected
                        self.start_load_all_articles();
                    }
                }
                // Reload feeds to update unread counts
                self.start_reload_feeds();
            }
        }
    }

    /// Process a completed async render result.
    pub fn handle_render_result(&mut self, result: RenderResult) {
        self.article_content = result.content;
        // Count the number of lines in the rendered content
        self.article_content_lines = self.article_content.lines().count() as u16;
        // Reset scroll position if needed (content may have changed)
        if self.article_scroll > 0 && self.article_scroll >= self.article_content_lines.saturating_sub(1) {
            self.article_scroll = self.article_content_lines.saturating_sub(1);
        }
    }

    // ---------------------------------------------------------------------
    // Action dispatch
    // ---------------------------------------------------------------------

    /// Process a single user action, updating all relevant application state.
    pub fn update(&mut self, action: Action) {
        // Clear any transient status message on the next user action.
        self.status_message = None;

        match action {
            Action::Quit => {
                self.should_quit = true;
            }

            Action::Digit(digit) => {
                // Accumulate digit for vim-style count prefix
                let new_count = self.pending_count.unwrap_or(0) * 10 + digit as u32;
                // Cap at a reasonable maximum (9999)
                self.pending_count = Some(new_count.min(9999));
            }

            Action::FocusNext => {
                self.active_pane = match self.active_pane {
                    ActivePane::Feeds => ActivePane::Articles,
                    ActivePane::Articles => ActivePane::ArticleView,
                    ActivePane::ArticleView => ActivePane::Feeds,
                };
            }

            Action::FocusPrev => {
                self.active_pane = match self.active_pane {
                    ActivePane::Feeds => ActivePane::ArticleView,
                    ActivePane::Articles => ActivePane::Feeds,
                    ActivePane::ArticleView => ActivePane::Articles,
                };
            }

            Action::MoveUp => {
                let count = self.pending_count.unwrap_or(1) as i32;
                self.pending_count = None;
                match self.active_pane {
                    ActivePane::Feeds => self.move_feed_selection(-count),
                    ActivePane::Articles => self.move_article_selection(-count),
                    ActivePane::ArticleView => {}
                }
            },

            Action::MoveDown => {
                let count = self.pending_count.unwrap_or(1) as i32;
                self.pending_count = None;
                match self.active_pane {
                    ActivePane::Feeds => self.move_feed_selection(count),
                    ActivePane::Articles => self.move_article_selection(count),
                    ActivePane::ArticleView => {}
                }
            },

            Action::Select => match self.active_pane {
                ActivePane::Feeds => self.select_feed_item(),
                ActivePane::Articles => self.select_article(),
                ActivePane::ArticleView => {}
            },

            Action::ToggleCollapse => {
                if self.active_pane == ActivePane::Feeds {
                    // Mark all as read (Space key) - only direct feeds
                    let Some(idx) = self.feeds_state.selected() else {
                        return;
                    };
                    let Some(item) = self.feed_list_items.get(idx) else {
                        return;
                    };

                    match item {
                        FeedListItem::All { .. } => {
                            self.start_mark_all_read_all();
                        }
                        FeedListItem::GroupHeader { full_path, .. } => {
                            let group_path = full_path.clone();
                            self.start_mark_all_read_for_group(group_path);
                        }
                        FeedListItem::Feed { feed, .. } => {
                            self.start_mark_all_read(feed.id);
                        }
                    }
                }
            }

            Action::ToggleCollapseRecursive => {
                if self.active_pane == ActivePane::Feeds {
                    // Mark all as read recursively (Shift+Space key)
                    let Some(idx) = self.feeds_state.selected() else {
                        return;
                    };
                    let Some(item) = self.feed_list_items.get(idx) else {
                        return;
                    };

                    match item {
                        FeedListItem::All { .. } => {
                            self.start_mark_all_read_all();
                        }
                        FeedListItem::GroupHeader { full_path, .. } => {
                            let group_path = full_path.clone();
                            self.start_mark_all_read_for_group_recursive(group_path);
                        }
                        FeedListItem::Feed { feed, .. } => {
                            self.start_mark_all_read(feed.id);
                        }
                    }
                }
            }

            Action::ToggleRead => {
                if let Some(article) = self.selected_article() {
                    let article_id = article.id;
                    self.start_toggle_read(article_id);
                }
            }

            Action::ToggleStar => {
                if let Some(article) = self.selected_article() {
                    let article_id = article.id;
                    self.start_toggle_star(article_id);
                }
            }

            Action::MarkAllRead => {
                // Check if "All" is selected.
                let is_all = self.feeds_state.selected()
                    .and_then(|idx| self.feed_list_items.get(idx))
                    .map(|item| matches!(item, FeedListItem::All { .. }))
                    .unwrap_or(false);

                if is_all {
                    self.start_mark_all_read_all();
                } else if let Some(feed) = self.selected_feed() {
                    let feed_id = feed.id;
                    self.start_mark_all_read(feed_id);
                }
            }

            Action::OpenInBrowser => {
                if let Some(article) = self.selected_article() {
                    if let Some(ref url) = article.url.clone() {
                        // Run browser opening in background to avoid blocking the TUI
                        let url_clone = url.clone();
                        tokio::spawn(async move {
                            let _ = open::that(&url_clone);
                        });
                    }
                }
            }

            Action::ScrollUp => match self.active_pane {
                ActivePane::ArticleView => {
                    self.article_scroll = self.article_scroll.saturating_sub(1);
                }
                _ => {}
            },

            Action::ScrollDown => match self.active_pane {
                ActivePane::ArticleView => {
                    // Don't scroll past the last line
                    let max_scroll = if self.article_content_lines > 0 {
                        self.article_content_lines.saturating_sub(1)
                    } else {
                        0
                    };
                    self.article_scroll = self.article_scroll.saturating_add(1).min(max_scroll);
                }
                _ => {}
            },

            Action::ScrollHalfPageUp => match self.active_pane {
                ActivePane::Feeds => self.move_feed_selection(-10),
                ActivePane::Articles => self.move_article_selection(-10),
                ActivePane::ArticleView => {
                    self.article_scroll = self.article_scroll.saturating_sub(10);
                }
            },

            Action::ScrollHalfPageDown => match self.active_pane {
                ActivePane::Feeds => self.move_feed_selection(10),
                ActivePane::Articles => self.move_article_selection(10),
                ActivePane::ArticleView => {
                    // Don't scroll past the last line
                    let max_scroll = if self.article_content_lines > 0 {
                        self.article_content_lines.saturating_sub(1)
                    } else {
                        0
                    };
                    self.article_scroll = self.article_scroll.saturating_add(10).min(max_scroll);
                }
            },

            Action::RefreshAll => {
                self.start_refresh_all();
            }

            Action::RefreshCurrent => {
                // If "All" is selected, refresh all feeds.
                let is_all = self.feeds_state.selected()
                    .and_then(|idx| self.feed_list_items.get(idx))
                    .map(|item| matches!(item, FeedListItem::All { .. }))
                    .unwrap_or(false);

                if is_all {
                    self.start_refresh_all();
                } else if let Some(feed) = self.selected_feed().cloned() {
                    self.pending_refreshes += 1;
                    self.is_refreshing = true;
                    feed::refresh_one(&self.feed_update_tx, &feed);
                }
            }

            Action::JumpToTop => match self.active_pane {
                ActivePane::Feeds => {
                    if !self.feed_list_items.is_empty() {
                        self.feeds_state.select(Some(0));
                        self.start_load_all_articles();
                    }
                }
                ActivePane::Articles => {
                    if !self.articles.is_empty() {
                        let current = self.articles_state.selected().unwrap_or(0);
                        if current != 0 {
                            self.articles_state.select(Some(0));
                            if let Some(article) = self.articles.first() {
                                self.selected_article_id = Some(article.id);
                                if !article.is_read {
                                    self.start_toggle_read(article.id);
                                }
                            }
                        }
                        self.start_render_article_content();
                    }
                }
                ActivePane::ArticleView => {
                    self.article_scroll = 0;
                }
            },

            Action::JumpToBottom => match self.active_pane {
                ActivePane::Feeds => {
                    if self.feed_list_items.len() > 1 {
                        let last_idx = self.feed_list_items.len() - 1;
                        self.feeds_state.select(Some(last_idx));
                        self.load_articles_for_selection_at(last_idx);
                    }
                }
                ActivePane::Articles => {
                    if !self.articles.is_empty() {
                        let current = self.articles_state.selected().unwrap_or(0);
                        let last_idx = self.articles.len() - 1;
                        if current != last_idx {
                            self.articles_state.select(Some(last_idx));
                            if let Some(article) = self.articles.last() {
                                self.selected_article_id = Some(article.id);
                                if !article.is_read {
                                    self.start_toggle_read(article.id);
                                }
                            }
                        }
                        self.start_render_article_content();
                    }
                }
                ActivePane::ArticleView => {
                    // Scroll to the bottom of the content
                    self.article_scroll = if self.article_content_lines > 0 {
                        self.article_content_lines.saturating_sub(1)
                    } else {
                        0
                    };
                }
            },

            Action::ExpandAllGroups => {
                if self.active_pane == ActivePane::Feeds {
                    self.expand_all_groups();
                }
            },

            Action::CollapseAllGroups => {
                if self.active_pane == ActivePane::Feeds {
                    self.collapse_all_groups();
                }
            },

            Action::ToggleAllGroups => {
                if self.active_pane == ActivePane::Feeds {
                    self.toggle_all_groups();
                }
            },

            Action::CreateGroup => {
                self.popup = Some(crate::ui::popup::Popup::create_group());
            },

            Action::CreateFeed => {
                self.popup = Some(crate::ui::popup::Popup::create_feed());
            },

            Action::Delete => {
                if self.active_pane == ActivePane::Feeds {
                    self.delete_selected_item();
                }
            },

            Action::Edit => {
                if self.active_pane == ActivePane::Feeds {
                    self.open_edit_popup();
                }
            },

            Action::Cut => {
                if self.active_pane == ActivePane::Feeds {
                    self.cut_selected_item();
                }
            },

            Action::Paste => {
                if self.active_pane == ActivePane::Feeds {
                    self.paste_clipboard();
                }
            },
        }
    }

    // ---------------------------------------------------------------------
    // Feed update handling
    // ---------------------------------------------------------------------

    /// Process a completed background feed-fetch result.
    ///
    /// Upserts articles into the database, updates the last-fetched timestamp,
    /// and refreshes any affected in-memory state.
    pub fn handle_feed_update(&mut self, result: FeedUpdateResult) {
        // Persist new articles asynchronously.
        let db = self.db.clone();
        let tx = self.db_result_tx.clone();
        let feed_id = result.feed_id;
        let articles = result.articles;
        let error = result.error;

        tokio::spawn(async move {
            // Upsert articles
            if let Err(_e) = db.upsert_articles(articles).await {
                let _ = tx.send(DbResult::FeedsLoaded(Vec::new())); // Dummy to wake up
                // TODO: send error
            }

            // Update last_fetched
            if let Err(_e) = db.update_last_fetched(feed_id).await {
                // TODO: send error
            }

            // Trigger feed reload to update unread counts
            match db.get_all_feeds().await {
                Ok(feeds) => {
                    let _ = tx.send(DbResult::FeedsLoaded(feeds));
                }
                Err(_) => {}
            }
        });

        // Surface fetch errors to the user.
        if let Some(ref err) = error {
            self.status_message = Some(format!("Fetch error: {err}"));
        }

        // Track outstanding refreshes.
        self.pending_refreshes = self.pending_refreshes.saturating_sub(1);
        if self.pending_refreshes == 0 {
            self.is_refreshing = false;
        }
    }

    /// Kick off a background refresh of all feeds.
    pub fn start_refresh_all(&mut self) {
        if self.feeds.is_empty() {
            return;
        }
        self.pending_refreshes = self.feeds.len();
        self.is_refreshing = true;
        feed::refresh_all(&self.feed_update_tx, &self.feeds);
    }

    // ---------------------------------------------------------------------
    // Async database operation starters
    // ---------------------------------------------------------------------

    /// Start an async reload of all feeds from the database.
    fn start_reload_feeds(&mut self) {
        let db = self.db.clone();
        let tx = self.db_result_tx.clone();
        tokio::spawn(async move {
            match db.get_all_feeds().await {
                Ok(feeds) => {
                    let _ = tx.send(DbResult::FeedsLoaded(feeds));
                }
                Err(_e) => {
                    // TODO: send error
                }
            }
        });
    }

    /// Start an async load of articles for a specific feed.
    fn start_load_articles_for_feed(&mut self, feed_id: i64) {
        let db = self.db.clone();
        let tx = self.db_result_tx.clone();
        tokio::spawn(async move {
            match db.get_articles_for_feed(feed_id).await {
                Ok(articles) => {
                    let _ = tx.send(DbResult::ArticlesLoaded { feed_id, articles });
                }
                Err(_) => {}
            }
        });
        // Don't clear articles immediately - keep showing current articles until new ones arrive
    }

    /// Start an async load of articles for a group.
    fn start_load_articles_for_group(&mut self, group_title: String) {
        let db = self.db.clone();
        let tx = self.db_result_tx.clone();
        tokio::spawn(async move {
            match db.get_articles_for_group(&group_title).await {
                Ok(articles) => {
                    let _ = tx.send(DbResult::GroupArticlesLoaded { group_title, articles });
                }
                Err(_) => {}
            }
        });
        // Don't clear articles immediately - keep showing current articles until new ones arrive
    }

    /// Start an async load of all articles.
    fn start_load_all_articles(&mut self) {
        let db = self.db.clone();
        let tx = self.db_result_tx.clone();
        tokio::spawn(async move {
            match db.get_all_articles().await {
                Ok(articles) => {
                    let _ = tx.send(DbResult::AllArticlesLoaded(articles));
                }
                Err(_) => {}
            }
        });
        // Don't clear articles immediately - keep showing current articles until new ones arrive
    }

    /// Start an async toggle read operation.
    fn start_toggle_read(&mut self, article_id: i64) {
        let db = self.db.clone();
        let tx = self.db_result_tx.clone();
        tokio::spawn(async move {
            match db.toggle_read(article_id).await {
                Ok(new_value) => {
                    let _ = tx.send(DbResult::ReadToggled { article_id, new_value });
                }
                Err(_) => {}
            }
        });
    }

    /// Start an async toggle star operation.
    fn start_toggle_star(&mut self, article_id: i64) {
        let db = self.db.clone();
        let tx = self.db_result_tx.clone();
        tokio::spawn(async move {
            match db.toggle_star(article_id).await {
                Ok(new_value) => {
                    let _ = tx.send(DbResult::StarToggled { article_id, new_value });
                }
                Err(_) => {}
            }
        });
    }

    /// Start an async mark all read operation for a feed.
    fn start_mark_all_read(&mut self, feed_id: i64) {
        let db = self.db.clone();
        let tx = self.db_result_tx.clone();
        tokio::spawn(async move {
            match db.mark_all_read(feed_id).await {
                Ok(_) => {
                    let _ = tx.send(DbResult::MarkedRead { feed_id: Some(feed_id) });
                }
                Err(_) => {}
            }
        });
    }

    /// Start an async mark all read operation for all feeds.
    fn start_mark_all_read_all(&mut self) {
        let db = self.db.clone();
        let tx = self.db_result_tx.clone();
        tokio::spawn(async move {
            match db.mark_all_read_all().await {
                Ok(_) => {
                    let _ = tx.send(DbResult::MarkedRead { feed_id: None });
                }
                Err(_) => {}
            }
        });
    }

    /// Start an async mark all read operation for a group (direct feeds only).
    fn start_mark_all_read_for_group(&mut self, group_title: String) {
        // Find all feeds in this group and mark each as read
        let feed_ids: Vec<i64> = self.feeds
            .iter()
            .filter(|f| f.group_title == group_title)
            .map(|f| f.id)
            .collect();

        // User-friendly summary message
        let matched_titles: Vec<String> = self.feeds
            .iter()
            .filter(|f| feed_ids.contains(&f.id))
            .map(|f| f.title.clone())
            .collect();
        self.status_message = Some(format!("Marked {} feed(s) in '{}' as read: {}",
            matched_titles.len(), group_title, matched_titles.join(", ")));

        for feed_id in feed_ids {
            self.start_mark_all_read(feed_id);
        }
    }

    /// Start an async mark all read operation for a group and all nested groups recursively.
    fn start_mark_all_read_for_group_recursive(&mut self, group_path: String) {
        // Find all feeds in this group or any nested group and mark each as read
        let feed_ids: Vec<i64> = self.feeds
            .iter()
            .filter(|f| {
                // Exact match for this group (direct feeds)
                if f.group_title == group_path {
                    return true;
                }
                // Nested groups - check if group_title starts with "{group_path} > "
                let nested_prefix = format!("{} > ", group_path);
                f.group_title.starts_with(&nested_prefix)
            })
            .map(|f| f.id)
            .collect();

        // User-friendly summary message
        let matched_titles: Vec<String> = self.feeds
            .iter()
            .filter(|f| feed_ids.contains(&f.id))
            .map(|f| f.title.clone())
            .collect();
        self.status_message = Some(format!("Marked {} feed(s) in '{}' and subgroups as read: {}",
            matched_titles.len(), group_path, matched_titles.join(", ")));

        for feed_id in feed_ids {
            self.start_mark_all_read(feed_id);
        }
    }

    /// Start an async render of the current article's content.
    fn start_render_article_content(&mut self) {
        let idx = match self.articles_state.selected() {
            Some(i) if i < self.articles.len() => i,
            _ => {
                self.article_content.clear();
        self.article_content_lines = 0;
                return;
            }
        };

        let article = match self.articles.get(idx) {
            Some(a) => a,
            None => return,
        };

        let html = article.content
            .as_deref()
            .or(article.summary.as_deref())
            .unwrap_or("(No content available)")
            .to_string();

        let title = article.title.clone();
        let author = article.author.clone();
        let (date_format, strip_day_zero) = to_strftime_format(&self.config.display.format.date_detail);
        let published = article.published
            .as_ref()
            .map(|d| {
                let formatted = d.format(&date_format).to_string();
                if strip_day_zero {
                    strip_day_leading_zero(&formatted)
                } else {
                    formatted
                }
            });

        // Look up feed name
        let feed_name = self.feeds.iter()
            .find(|f| f.id == article.feed_id)
            .map(|f| f.title.clone());

        let tx = self.render_tx.clone();

        tokio::task::spawn_blocking(move || {
            // Build header
            let mut content = title.clone();
            content.push('\n');

            if let Some(ref feed) = feed_name {
                content.push_str(&format!("\nFrom: {feed}\n"));
            }

            if let Some(ref author) = author {
                content.push_str(&format!("By {author}\n"));
            }
            if let Some(ref published) = published {
                content.push_str(&format!("{published}\n"));
            }
            content.push_str("\n──────────\n\n");

            // Convert HTML to plain text
            let body = html2text::from_read(html.as_bytes(), 80);
            content.push_str(&body);

            let _ = tx.send(RenderResult { content });
        });

        self.article_content.clear();
        self.article_content_lines = 0;
    }

    // ---------------------------------------------------------------------
    // Feed list building
    // ---------------------------------------------------------------------

    /// Rebuild `feed_list_items` from the current `feeds` vec, respecting
    /// collapse state.
    ///
    /// Preserves the current selection index when possible.
    fn build_feed_list_items(&mut self) {
        let old_selection = self.feeds_state.selected();

        // Remember what was selected before rebuilding.
        let old_was_all = old_selection.and_then(|idx| {
            self.feed_list_items.get(idx).and_then(|item| match item {
                FeedListItem::All { .. } => Some(true),
                _ => None,
            })
        }).unwrap_or(false);

        let old_selected_feed_id = old_selection.and_then(|idx| {
            self.feed_list_items.get(idx).and_then(|item| match item {
                FeedListItem::Feed { feed, .. } => Some(feed.id),
                _ => None,
            })
        });

        let old_selected_group_path = old_selection.and_then(|idx| {
            self.feed_list_items.get(idx).and_then(|item| match item {
                // For groups, we store the full_path for proper identification
                FeedListItem::GroupHeader { full_path, .. } => Some(full_path.clone()),
                _ => None,
            })
        });

        self.feed_list_items.clear();

        // Calculate total unread count for "All"
        let total_unread: u32 = self.feeds.iter().map(|f| f.unread_count).sum();

        // Add "All" at the top.
        self.feed_list_items.push(FeedListItem::All { unread_count: total_unread });

        // Separate standalone feeds (empty group_title) from grouped feeds
        let standalone_feeds: Vec<_> = self.feeds.iter()
            .filter(|f| f.group_title.is_empty())
            .cloned()
            .collect();

        let grouped_feeds: Vec<_> = self.feeds.iter()
            .filter(|f| !f.group_title.is_empty())
            .cloned()
            .collect();

        // Add standalone feeds first (no header, no indent)
        for feed in standalone_feeds {
            self.feed_list_items.push(FeedListItem::Feed {
                feed,
                depth: 0,
            });
        }

        // Build tree from grouped feeds and empty groups
        let tree = build_group_tree(&grouped_feeds, &self.empty_groups);

        // Recursively add tree items
        for node in tree {
            self.add_tree_node(&node, 0, false);
        }

        // Attempt to restore the selection to the same item.
        let mut restored = false;

        // Restore "All" selection.
        if old_was_all {
            self.feeds_state.select(Some(0));
            restored = true;
        }

        // Restore feed selection.
        if !restored {
            if let Some(feed_id) = old_selected_feed_id {
                if let Some(pos) = self.feed_list_items.iter().position(|item| {
                    matches!(item, FeedListItem::Feed { feed, .. } if feed.id == feed_id)
                }) {
                    self.feeds_state.select(Some(pos));
                    restored = true;
                }
            }
        }

        // Restore group header selection (match by full_path)
        if !restored {
            if let Some(group_path) = old_selected_group_path {
                if let Some(pos) = self.feed_list_items.iter().position(|item| {
                    matches!(item, FeedListItem::GroupHeader { full_path, .. } if *full_path == group_path)
                }) {
                    self.feeds_state.select(Some(pos));
                    restored = true;
                }
            }
        }

        // Default to first item (which is "All").
        if !restored {
            if self.feed_list_items.is_empty() {
                self.feeds_state.select(None);
            } else {
                let idx = old_selection.unwrap_or(0).min(self.feed_list_items.len() - 1);
                self.feeds_state.select(Some(idx));
            }
        }
    }

    /// Recursively add a group node and its children to the feed list.
    fn add_tree_node(&mut self, node: &GroupNode, depth: u8, parent_collapsed: bool) {
        let is_collapsed = self.collapsed_groups.contains(&node.full_path);
        let actually_collapsed = parent_collapsed || is_collapsed;

        self.feed_list_items.push(FeedListItem::GroupHeader {
            title: node.title.clone(),
            full_path: node.full_path.clone(),
            collapsed: is_collapsed,
            unread_count: node.unread_count,
            depth,
        });

        if !actually_collapsed {
            // Add child feeds
            for feed in &node.feeds {
                self.feed_list_items.push(FeedListItem::Feed {
                    feed: feed.clone(),
                    depth: depth + 1,
                });
            }

            // Recursively add child groups
            for child in &node.children {
                self.add_tree_node(child, depth + 1, actually_collapsed);
            }
        }
    }

    // ---------------------------------------------------------------------
    // Navigation helpers
    // ---------------------------------------------------------------------

    /// Move the feed list selection by `delta` (+1 = down, -1 = up).
    fn move_feed_selection(&mut self, delta: i32) {
        if self.feed_list_items.is_empty() {
            return;
        }
        let current = self.feeds_state.selected().unwrap_or(0);
        let len = self.feed_list_items.len();

        // Circular scrolling: wrap around using modulo
        let new_idx = if delta >= 0 {
            (current + delta as usize) % len
        } else {
            let abs_delta = (-delta) as usize;
            // Handle wrap-around for negative delta
            let offset = abs_delta % len;
            if offset <= current {
                current - offset
            } else {
                len - (offset - current)
            }
        };

        self.feeds_state.select(Some(new_idx));
        self.load_articles_for_selection_at(new_idx);
    }

    /// Load articles for the feed list item at the given index.
    fn load_articles_for_selection_at(&mut self, idx: usize) {
        match self.feed_list_items.get(idx) {
            Some(FeedListItem::All { .. }) => {
                self.start_load_all_articles();
            }
            Some(FeedListItem::GroupHeader { full_path, .. }) => {
                let group_path = full_path.clone();
                self.start_load_articles_for_group(group_path);
            }
            Some(FeedListItem::Feed { feed, .. }) => {
                self.start_load_articles_for_feed(feed.id);
            }
            None => {}
        }
    }

    /// Load articles for the currently selected feed list item.
    fn load_articles_for_current_selection(&mut self) {
        let idx = match self.feeds_state.selected() {
            Some(i) => i,
            None => return,
        };
        self.load_articles_for_selection_at(idx);
    }

    /// Move the article list selection by `delta` (+1 = down, -1 = up).
    fn move_article_selection(&mut self, delta: i32) {
        if self.articles.is_empty() {
            return;
        }
        let current = self.articles_state.selected().unwrap_or(0);
        let len = self.articles.len();

        // Circular scrolling: wrap around using modulo
        let new_idx = if delta >= 0 {
            (current + delta as usize) % len
        } else {
            let abs_delta = (-delta) as usize;
            // Handle wrap-around for negative delta
            let offset = abs_delta % len;
            if offset <= current {
                current - offset
            } else {
                len - (offset - current)
            }
        };

        // Mark the new article as read if the selection is actually changing
        let should_mark_read = new_idx != current;

        self.articles_state.select(Some(new_idx));

        // Update selected_article_id
        if let Some(article) = self.articles.get(new_idx) {
            self.selected_article_id = Some(article.id);
        }

        if should_mark_read {
            if let Some(article) = self.articles.get(new_idx) {
                if !article.is_read {
                    self.start_toggle_read(article.id);
                }
            }
        }

        self.start_render_article_content();
    }

    /// Handle `Select` in the feeds pane.
    ///
    /// If a group header is selected, toggle its collapsed state.
    /// If a feed is selected, switch focus to the articles pane.
    /// (Articles are already loaded automatically during navigation.)
    fn select_feed_item(&mut self) {
        let Some(idx) = self.feeds_state.selected() else {
            return;
        };
        let Some(item) = self.feed_list_items.get(idx) else {
            return;
        };

        match item {
            FeedListItem::All { .. } => {
                // Articles already loaded by navigation, just switch focus.
                self.active_pane = ActivePane::Articles;
            }
            FeedListItem::GroupHeader { full_path, .. } => {
                let group_path = full_path.clone();
                self.toggle_collapse(&group_path);
            }
            FeedListItem::Feed { .. } => {
                // Articles already loaded by navigation, just switch focus.
                self.active_pane = ActivePane::Articles;
            }
        }
    }

    /// Handle `Select` in the articles pane.
    ///
    /// Marks the article as read and switches focus to the article view.
    /// (Content is already loaded automatically when selection changes.)
    fn select_article(&mut self) {
        let Some(idx) = self.articles_state.selected() else {
            return;
        };
        let Some(article) = self.articles.get(idx) else {
            return;
        };

        let article_id = article.id;

        // Mark the article as read if it is not already.
        if !article.is_read {
            self.start_toggle_read(article_id);
        }

        // Content is already loaded by navigation, just switch focus.
        self.active_pane = ActivePane::ArticleView;
    }

    /// Toggle the collapsed state for the given group title and rebuild the
    /// feed list.
    fn toggle_collapse(&mut self, group_title: &str) {
        if self.collapsed_groups.contains(group_title) {
            self.collapsed_groups.remove(group_title);
        } else {
            self.collapsed_groups.insert(group_title.to_string());
        }
        self.build_feed_list_items();
    }

    /// Expand all groups by clearing the collapsed_groups set.
    fn expand_all_groups(&mut self) {
        self.collapsed_groups.clear();
        self.build_feed_list_items();
    }

    /// Collapse all groups by adding all group titles to the collapsed_groups set.
    fn collapse_all_groups(&mut self) {
        // Collect all unique group titles from feeds
        let mut all_groups: std::collections::HashSet<String> = std::collections::HashSet::new();
        for feed in &self.feeds {
            all_groups.insert(feed.group_title.clone());
        }
        self.collapsed_groups = all_groups;
        self.build_feed_list_items();
    }

    /// Toggle between all expanded and all collapsed.
    /// If any group is collapsed, expand all. If all are expanded, collapse all.
    fn toggle_all_groups(&mut self) {
        // Count unique groups
        let total_groups: std::collections::HashSet<String> = self.feeds
            .iter()
            .map(|f| f.group_title.clone())
            .collect();

        if self.collapsed_groups.is_empty() {
            // All are expanded → collapse all
            self.collapsed_groups = total_groups;
        } else {
            // Some or all are collapsed → expand all
            self.collapsed_groups.clear();
        }
        self.build_feed_list_items();
    }

    // ---------------------------------------------------------------------
    // Selection accessors
    // ---------------------------------------------------------------------

    /// Return a reference to the `Feed` at the current feeds-list cursor
    /// position, or `None` if the cursor is on a group header or no selection
    /// exists.
    pub fn selected_feed(&self) -> Option<&db::Feed> {
        let idx = self.feeds_state.selected()?;
        match self.feed_list_items.get(idx)? {
            FeedListItem::All { .. } => None,
            FeedListItem::Feed { feed, .. } => Some(feed),
            FeedListItem::GroupHeader { .. } => None,
        }
    }

    /// Return a reference to the article at the current articles-list cursor.
    pub fn selected_article(&self) -> Option<&db::Article> {
        let idx = self.articles_state.selected()?;
        self.articles.get(idx)
    }

    // ---------------------------------------------------------------------
    // Popup handling
    // ---------------------------------------------------------------------

    /// Handle character input when popup is active
    pub fn handle_popup_char(&mut self, c: char) {
        if let Some(ref mut popup) = self.popup {
            popup.handle_char(c);
        }
    }

    /// Handle backspace when popup is active
    pub fn handle_popup_backspace(&mut self) {
        if let Some(ref mut popup) = self.popup {
            popup.handle_backspace();
        }
    }

    /// Handle Enter key when popup is active
    pub fn handle_popup_enter(&mut self) {
        if let Some(popup) = self.popup.take() {
            let is_create_feed = popup.is_create_feed();
            let is_edit_feed = popup.is_edit_feed();
            let is_edit_group = popup.is_edit() && !is_edit_feed; // Edit group but not Edit feed
            let (value1, value2, value3, value4) = popup.confirm();

            if is_create_feed {
                // Create feed: value1=title, value2=url, value3=feed_url (Option<String>)
                if !value1.trim().is_empty() && !value2.trim().is_empty() {
                    self.create_feed(value1, value2, value3);
                }
            } else if is_edit_feed {
                // Edit feed: value1=title, value2=url, value3=feed_url, value4=original_url
                if !value1.trim().is_empty() && !value2.trim().is_empty() {
                    if let Some(original_url) = value4 {
                        self.edit_feed(original_url, value1, value2, value3);
                    }
                }
            } else if is_edit_group {
                // Edit group: value1=new_name, value4=original_path
                if !value1.trim().is_empty() {
                    if let Some(original_path) = value4 {
                        self.edit_group(original_path, value1);
                    }
                }
            } else {
                // Create group: value1=group_name
                if !value1.trim().is_empty() {
                    self.create_group(value1);
                }
            }
        }
    }

    /// Handle Tab key when popup is active
    pub fn handle_popup_tab(&mut self) {
        if let Some(ref mut popup) = self.popup {
            popup.handle_tab();
        }
    }

    /// Handle Shift+Tab (BackTab) when popup is active
    pub fn handle_popup_backtab(&mut self) {
        if let Some(ref mut popup) = self.popup {
            popup.handle_backtab();
        }
    }

    /// Handle Escape key when popup is active
    pub fn handle_popup_escape(&mut self) {
        self.popup = None;
    }

    /// Create a new group with the given name
    fn create_group(&mut self, group_name: String) {
        // Get currently selected group path, or use empty for root
        let parent_group = self.get_selected_group_path();

        // Build the full group path
        let full_path = if let Some(parent) = parent_group {
            if parent.is_empty() {
                group_name
            } else {
                format!("{} > {}", parent, group_name)
            }
        } else {
            group_name
        };

        // Add to config
        self.add_group_to_config(&full_path);

        // Save only the feeds section to preserve formatting
        if let Err(e) = crate::config::save_feeds_only(&self.config.feeds) {
            self.status_message = Some(format!("Failed to save config: {}", e));
            return;
        }

        // Reload feeds from updated config
        self.reload_feeds_from_config();

        self.status_message = Some(format!("Created group: {}", full_path));
    }

    /// Create a new feed with the given title, URL, and optional feed URL
    fn create_feed(&mut self, title: String, url: String, feed_url: Option<String>) {
        // Get the parent group path (if a group is selected)
        let parent_group = self.get_selected_group_path();

        // Add to config
        self.add_feed_to_config(&title, &url, feed_url.as_deref(), parent_group.as_deref());

        // Save only the feeds section to preserve formatting
        if let Err(e) = crate::config::save_feeds_only(&self.config.feeds) {
            self.status_message = Some(format!("Failed to save config: {}", e));
            return;
        }

        // Reload feeds from updated config
        self.reload_feeds_from_config();

        let location = if let Some(group) = parent_group {
            format!("'{}'", group)
        } else {
            "root".to_string()
        };
        self.status_message = Some(format!("Created feed '{}' in {}", title, location));
    }

    /// Open the appropriate edit popup based on the selected item
    fn open_edit_popup(&mut self) {
        let Some(idx) = self.feeds_state.selected() else {
            self.status_message = Some("No item selected to edit".to_string());
            return;
        };

        let Some(item) = self.feed_list_items.get(idx) else {
            return;
        };

        // Cannot edit "All"
        if matches!(item, FeedListItem::All { .. }) {
            self.status_message = Some("Cannot edit 'All'".to_string());
            return;
        }

        match item {
            FeedListItem::Feed { feed, .. } => {
                // Open edit feed popup with pre-populated values
                // original_url = feed.url (used to identify the feed)
                // title = feed.title
                // url = website URL (from site_url)
                // feed_url = feed URL (from url field, optional)
                self.popup = Some(crate::ui::popup::Popup::edit_feed(
                    feed.url.clone(),
                    feed.title.clone(),
                    feed.site_url.clone().unwrap_or_default(),
                    Some(feed.url.clone()),
                ));
            }
            FeedListItem::GroupHeader { full_path, title, .. } => {
                // Open edit group popup with pre-populated title
                self.popup = Some(crate::ui::popup::Popup::edit_group(
                    full_path.clone(),
                    title.clone(),
                ));
            }
            FeedListItem::All { .. } => {
                // Already handled above
            }
        }
    }

    /// Edit an existing feed with new values
    fn edit_feed(&mut self, original_url: String, new_title: String, new_url: String, new_feed_url: Option<String>) {
        // Update the feed in config
        let updated = Self::update_feed_in_config(&mut self.config.feeds, &original_url, &new_title, &new_url, new_feed_url.as_deref());

        if !updated {
            self.status_message = Some(format!("Feed '{}' not found in config", original_url));
            return;
        }

        // Save only the feeds section to preserve formatting
        if let Err(e) = crate::config::save_feeds_only(&self.config.feeds) {
            self.status_message = Some(format!("Failed to save config: {}", e));
            return;
        }

        // Reload feeds from updated config
        self.reload_feeds_from_config();

        self.status_message = Some(format!("Updated feed '{}'", new_title));
    }

    /// Edit an existing group title
    fn edit_group(&mut self, original_path: String, new_name: String) {
        // Update the group title in config
        let updated = Self::update_group_in_config(&mut self.config.feeds, &original_path, &new_name);

        if !updated {
            self.status_message = Some(format!("Group '{}' not found in config", original_path));
            return;
        }

        // Save only the feeds section to preserve formatting
        if let Err(e) = crate::config::save_feeds_only(&self.config.feeds) {
            self.status_message = Some(format!("Failed to save config: {}", e));
            return;
        }

        // Reload feeds from updated config
        self.reload_feeds_from_config();

        self.status_message = Some(format!("Updated group to '{}'", new_name));
    }

    /// Get the path of the currently selected group, if any.
    ///
    /// Returns the parent group path based on the focused item:
    /// - "All" focused -> None (create at root)
    /// - Group header focused -> that group's path
    /// - Feed focused -> the feed's parent group (from feed.group_title)
    fn get_selected_group_path(&self) -> Option<String> {
        let idx = self.feeds_state.selected()?;
        match self.feed_list_items.get(idx)? {
            FeedListItem::All { .. } => None,
            FeedListItem::GroupHeader { full_path, .. } => Some(full_path.clone()),
            FeedListItem::Feed { feed, .. } => {
                // If feed has a parent group, use that; otherwise create at root
                if feed.group_title.is_empty() {
                    None
                } else {
                    Some(feed.group_title.clone())
                }
            }
        }
    }

    /// Add a group to the config at the given path
    fn add_group_to_config(&mut self, group_path: &str) {
        // Parse group path into components
        let components: Vec<&str> = group_path.split(" > ").collect();

        // Recursively find or create group structure
        Self::insert_group_recursive(&mut self.config.feeds, &components, 0);
    }

    /// Add a feed to the config under the given parent group (or standalone if None)
    fn add_feed_to_config(&mut self, title: &str, url: &str, feed_url: Option<&str>, parent_group: Option<&str>) {
        let feed_source = FeedSource {
            title: title.to_string(),
            url: url.to_string(),
            feed: feed_url.map(|s| s.to_string()),
        };

        if let Some(group_path) = parent_group {
            // Add to group - parse group path into components
            let components: Vec<&str> = group_path.split(" > ").collect();
            Self::insert_feed_into_group(&mut self.config.feeds, &components, 0, feed_source);
        } else {
            // Add as standalone feed
            self.config.feeds.push(FeedConfigItem::Standalone(feed_source));
        }
    }

    /// Recursively insert a feed into a group in the config tree
    fn insert_feed_into_group(feeds: &mut Vec<FeedConfigItem>, components: &[&str], depth: usize, feed_source: FeedSource) {
        if depth >= components.len() {
            return;
        }

        let current_title = components[depth];
        let is_last = depth == components.len() - 1;

        // Find existing group at this level
        let existing_pos = feeds.iter().position(|item| {
            if let FeedConfigItem::Group(group) = item {
                group.title == current_title
            } else {
                false
            }
        });

        if let Some(pos) = existing_pos {
            if let FeedConfigItem::Group(ref mut group) = feeds[pos] {
                if is_last {
                    // This is the target group, add the feed
                    group.feeds.push(FeedConfigItem::Standalone(feed_source));
                } else {
                    // Continue traversing
                    Self::insert_feed_into_group(&mut group.feeds, components, depth + 1, feed_source);
                }
            }
        } else {
            // Group doesn't exist - we need to create it first
            if is_last {
                // Create the group with the feed inside
                let new_group = FeedGroup {
                    title: current_title.to_string(),
                    feeds: vec![FeedConfigItem::Standalone(feed_source)],
                };
                feeds.push(FeedConfigItem::Group(new_group));
            } else {
                // Create intermediate group and continue
                let new_group = FeedGroup {
                    title: current_title.to_string(),
                    feeds: Vec::new(),
                };
                let mut new_group = new_group;
                Self::insert_feed_into_group(&mut new_group.feeds, components, depth + 1, feed_source);
                feeds.push(FeedConfigItem::Group(new_group));
            }
        }
    }

    /// Recursively insert a group into the config tree
    fn insert_group_recursive(feeds: &mut Vec<FeedConfigItem>, components: &[&str], depth: usize) {
        if depth >= components.len() {
            return;
        }

        let current_title = components[depth];
        let is_last = depth == components.len() - 1;

        // Find existing group at this level
        let existing_pos = feeds.iter().position(|item| {
            if let FeedConfigItem::Group(group) = item {
                group.title == current_title
            } else {
                false
            }
        });

        if let Some(pos) = existing_pos {
            if let FeedConfigItem::Group(ref mut group) = feeds[pos] {
                if !is_last {
                    // Continue traversing
                    Self::insert_group_recursive(&mut group.feeds, components, depth + 1);
                }
                // If it's the last component and group exists, do nothing (already exists)
            }
        } else {
            // Create new group
            if is_last {
                feeds.push(FeedConfigItem::Group(FeedGroup {
                    title: current_title.to_string(),
                    feeds: Vec::new(),
                }));
            } else {
                // Create intermediate group and continue
                let mut new_group = FeedGroup {
                    title: current_title.to_string(),
                    feeds: Vec::new(),
                };
                Self::insert_group_recursive(&mut new_group.feeds, components, depth + 1);
                feeds.push(FeedConfigItem::Group(new_group));
            }
        }
    }

    /// Reload feeds from config after making changes
    fn reload_feeds_from_config(&mut self) {
        // Update empty groups from the updated config
        self.empty_groups = crate::config::collect_empty_groups_from_config(&self.config);

        let db = self.db.clone();
        let config = self.config.clone();
        let tx = self.db_result_tx.clone();

        tokio::spawn(async move {
            match db.sync_feeds_from_config(&config).await {
                Ok(_) => {
                    // After syncing, reload feeds to update the UI
                    match db.get_all_feeds().await {
                        Ok(feeds) => {
                            let _ = tx.send(DbResult::FeedsLoaded(feeds));
                        }
                        Err(_) => {}
                    }
                }
                Err(e) => {
                    eprintln!("Failed to sync feeds from config: {}", e);
                }
            }
        });
    }

    /// Delete the currently selected feed or group
    fn delete_selected_item(&mut self) {
        let Some(idx) = self.feeds_state.selected() else {
            self.status_message = Some("No item selected to delete".to_string());
            return;
        };

        let Some(item) = self.feed_list_items.get(idx) else {
            return;
        };

        // Cannot delete "All"
        if matches!(item, FeedListItem::All { .. }) {
            self.status_message = Some("Cannot delete 'All'".to_string());
            return;
        }

        match item {
            FeedListItem::GroupHeader { full_path, .. } => {
                // Delete the group from config
                let group_path = full_path.clone();
                self.delete_group_from_config(&group_path);
            }
            FeedListItem::Feed { feed, .. } => {
                // Delete the feed from config
                let feed_url = feed.url.clone();
                self.delete_feed_from_config(&feed_url);
            }
            FeedListItem::All { .. } => {
                // Already handled above
            }
        }
    }

    /// Delete a feed from the config by URL
    fn delete_feed_from_config(&mut self, feed_url: &str) {
        // Recursively remove the feed from config
        let removed = Self::remove_feed_recursive(&mut self.config.feeds, feed_url);

        if !removed {
            self.status_message = Some(format!("Feed '{}' not found in config", feed_url));
            return;
        }

        // Save only the feeds section to preserve formatting
        if let Err(e) = crate::config::save_feeds_only(&self.config.feeds) {
            self.status_message = Some(format!("Failed to save config: {}", e));
            return;
        }

        // Reload feeds from updated config
        self.reload_feeds_from_config();

        self.status_message = Some(format!("Deleted feed: {}", feed_url));
    }

    /// Delete a group from the config by path
    fn delete_group_from_config(&mut self, group_path: &str) {
        // Recursively remove the group from config
        let removed = Self::remove_group_recursive(&mut self.config.feeds, group_path);

        if !removed {
            self.status_message = Some(format!("Group '{}' not found in config", group_path));
            return;
        }

        // Save only the feeds section to preserve formatting
        if let Err(e) = crate::config::save_feeds_only(&self.config.feeds) {
            self.status_message = Some(format!("Failed to save config: {}", e));
            return;
        }

        // Reload feeds from updated config
        self.reload_feeds_from_config();

        self.status_message = Some(format!("Deleted group: {}", group_path));
    }

    /// Recursively remove a feed from the config tree by URL
    /// Returns true if the feed was found and removed
    fn remove_feed_recursive(feeds: &mut Vec<FeedConfigItem>, feed_url: &str) -> bool {
        for i in 0..feeds.len() {
            match &mut feeds[i] {
                FeedConfigItem::Standalone(feed_source) => {
                    if feed_source.feed.as_deref() == Some(feed_url) {
                        feeds.remove(i);
                        return true;
                    }
                }
                FeedConfigItem::Group(group) => {
                    // Check if the feed is in this group's feeds
                    for j in 0..group.feeds.len() {
                        if let FeedConfigItem::Standalone(feed_source) = &group.feeds[j] {
                            if feed_source.feed.as_deref() == Some(feed_url) {
                                group.feeds.remove(j);
                                return true;
                            }
                        }
                    }
                    // Recursively check nested groups
                    if Self::remove_feed_recursive(&mut group.feeds, feed_url) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Recursively remove a group from the config tree by path
    /// Returns true if the group was found and removed
    fn remove_group_recursive(feeds: &mut Vec<FeedConfigItem>, group_path: &str) -> bool {
        let components: Vec<&str> = group_path.split(" > ").collect();

        if components.is_empty() {
            return false;
        }

        Self::remove_group_recursive_helper(feeds, &components, 0)
    }

    fn remove_group_recursive_helper(feeds: &mut Vec<FeedConfigItem>, components: &[&str], depth: usize) -> bool {
        if depth >= components.len() {
            return false;
        }

        let current_title = components[depth];
        let is_last = depth == components.len() - 1;

        // Find the group at this level
        let mut found_idx = None;
        for (i, item) in feeds.iter().enumerate() {
            if let FeedConfigItem::Group(group) = item {
                if group.title == current_title {
                    found_idx = Some(i);
                    break;
                }
            }
        }

        let idx = match found_idx {
            Some(i) => i,
            None => return false,
        };

        if is_last {
            // This is the group to remove
            feeds.remove(idx);
            true
        } else {
            // Need to go deeper into nested groups
            // Modify the group in place instead of removing it
            if let FeedConfigItem::Group(ref mut group) = feeds[idx] {
                Self::remove_group_recursive_helper(&mut group.feeds, components, depth + 1)
            } else {
                false
            }
        }
    }

    /// Recursively update a feed in the config tree by URL
    /// Returns true if the feed was found and updated
    fn update_feed_in_config(feeds: &mut Vec<FeedConfigItem>, original_url: &str, new_title: &str, new_url: &str, new_feed_url: Option<&str>) -> bool {
        for item in feeds.iter_mut() {
            match item {
                FeedConfigItem::Standalone(feed_source) => {
                    // Check by URL (not feed URL, since URL can change)
                    if feed_source.url == original_url || feed_source.feed.as_deref() == Some(original_url) {
                        feed_source.title = new_title.to_string();
                        feed_source.url = new_url.to_string();
                        feed_source.feed = new_feed_url.map(|s| s.to_string());
                        return true;
                    }
                }
                FeedConfigItem::Group(group) => {
                    // Check feeds in this group
                    for feed_item in group.feeds.iter_mut() {
                        if let FeedConfigItem::Standalone(feed_source) = feed_item {
                            if feed_source.url == original_url || feed_source.feed.as_deref() == Some(original_url) {
                                feed_source.title = new_title.to_string();
                                feed_source.url = new_url.to_string();
                                feed_source.feed = new_feed_url.map(|s| s.to_string());
                                return true;
                            }
                        }
                    }
                    // Recursively check nested groups
                    if Self::update_feed_in_config(&mut group.feeds, original_url, new_title, new_url, new_feed_url) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Recursively update a group title in the config tree by path
    /// Returns true if the group was found and updated
    fn update_group_in_config(feeds: &mut Vec<FeedConfigItem>, group_path: &str, new_name: &str) -> bool {
        let components: Vec<&str> = group_path.split(" > ").collect();

        if components.is_empty() {
            return false;
        }

        Self::update_group_recursive_helper(feeds, &components, 0, new_name)
    }

    fn update_group_recursive_helper(feeds: &mut Vec<FeedConfigItem>, components: &[&str], depth: usize, new_name: &str) -> bool {
        if depth >= components.len() {
            return false;
        }

        let current_title = components[depth];
        let is_last = depth == components.len() - 1;

        // Find the group at this level
        for item in feeds.iter_mut() {
            if let FeedConfigItem::Group(group) = item {
                if group.title == current_title {
                    if is_last {
                        // This is the group to update
                        group.title = new_name.to_string();
                        return true;
                    } else {
                        // Need to go deeper into nested groups
                        if Self::update_group_recursive_helper(&mut group.feeds, components, depth + 1, new_name) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Cut the currently selected feed or group to the clipboard
    fn cut_selected_item(&mut self) {
        let Some(idx) = self.feeds_state.selected() else {
            self.status_message = Some("No item selected to cut".to_string());
            return;
        };

        let Some(item) = self.feed_list_items.get(idx) else {
            return;
        };

        // Cannot cut "All"
        if matches!(item, FeedListItem::All { .. }) {
            self.status_message = Some("Cannot cut 'All'".to_string());
            return;
        }

        match item {
            FeedListItem::GroupHeader { full_path, .. } => {
                // Cut the group from config
                let group_path = full_path.clone();
                if let Some(group) = self.extract_group_from_config(&group_path) {
                    self.clipboard = Some(ClipboardItem::Group {
                        original_path: group_path.clone(),
                        group,
                    });
                    self.status_message = Some(format!("Cut group: {}", group_path));
                } else {
                    self.status_message = Some(format!("Group '{}' not found in config", group_path));
                    return;
                }
            }
            FeedListItem::Feed { feed, .. } => {
                // Cut the feed from config
                let feed_url = feed.url.clone();
                let feed_title = feed.title.clone();
                let group_title = if feed.group_title.is_empty() {
                    None
                } else {
                    Some(feed.group_title.clone())
                };

                if let Some(feed_source) = self.extract_feed_from_config(&feed_url) {
                    self.clipboard = Some(ClipboardItem::Feed {
                        feed_source,
                        original_group: group_title,
                    });
                    self.status_message = Some(format!("Cut feed: {}", feed_title));
                } else {
                    self.status_message = Some(format!("Feed '{}' not found in config", feed_url));
                    return;
                }
            }
            FeedListItem::All { .. } => {
                // Already handled above
            }
        }

        // Save the updated config (after cutting/removing)
        if let Err(e) = crate::config::save_feeds_only(&self.config.feeds) {
            self.status_message = Some(format!("Failed to save config: {}", e));
            return;
        }

        // Reload feeds from updated config
        self.reload_feeds_from_config();
    }

    /// Paste the clipboard item to the currently focused location
    fn paste_clipboard(&mut self) {
        let clipboard_item = match self.clipboard.take() {
            Some(item) => item,
            None => {
                self.status_message = Some("Nothing to paste (clipboard is empty)".to_string());
                return;
            }
        };

        // Determine the target location
        let Some(idx) = self.feeds_state.selected() else {
            self.status_message = Some("No location selected to paste".to_string());
            // Restore clipboard since paste failed
            self.clipboard = Some(clipboard_item);
            return;
        };

        let Some(item) = self.feed_list_items.get(idx) else {
            // Restore clipboard since paste failed
            self.clipboard = Some(clipboard_item);
            return;
        };

        // Determine target group path
        let target_group = match item {
            FeedListItem::All { .. } => None, // Paste at root level
            FeedListItem::GroupHeader { full_path, .. } => Some(full_path.clone()),
            FeedListItem::Feed { feed, .. } => {
                // Paste into the feed's parent group
                if feed.group_title.is_empty() {
                    None // Root level
                } else {
                    Some(feed.group_title.clone())
                }
            }
        };

        // Perform the paste
        match clipboard_item {
            ClipboardItem::Feed { feed_source, .. } => {
                self.paste_feed_to_config(feed_source, target_group.as_deref());
            }
            ClipboardItem::Group { group, .. } => {
                self.paste_group_to_config(&group, target_group.as_deref());
            }
        }

        // Note: clipboard is already taken (cleared) above
        // Don't restore it after successful paste
    }

    /// Extract a group from the config by path (without removing it)
    /// This is used to get the group data for cutting
    fn extract_group_from_config(&mut self, group_path: &str) -> Option<FeedGroup> {
        let components: Vec<&str> = group_path.split(" > ").collect();
        if components.is_empty() {
            return None;
        }

        // Find and remove the group, returning it
        let removed = Self::remove_and_return_group(&mut self.config.feeds, &components, 0);
        removed
    }

    /// Recursively find, remove, and return a group from the config tree
    fn remove_and_return_group(feeds: &mut Vec<FeedConfigItem>, components: &[&str], depth: usize) -> Option<FeedGroup> {
        if depth >= components.len() {
            return None;
        }

        let current_title = components[depth];
        let is_last = depth == components.len() - 1;

        // Find the group at this level
        for (i, item) in feeds.iter().enumerate() {
            if let FeedConfigItem::Group(group) = item {
                if group.title == current_title {
                    if is_last {
                        // This is the group to remove and return
                        if let FeedConfigItem::Group(removed_group) = feeds.remove(i) {
                            return Some(removed_group);
                        }
                    } else {
                        // Need to go deeper
                        if let FeedConfigItem::Group(ref mut group) = feeds[i] {
                            return Self::remove_and_return_group(&mut group.feeds, components, depth + 1);
                        }
                    }
                    break;
                }
            }
        }

        None
    }

    /// Extract a feed from the config by URL (without removing it)
    /// This is used to get the feed data for cutting
    fn extract_feed_from_config(&mut self, feed_url: &str) -> Option<FeedSource> {
        Self::remove_and_return_feed(&mut self.config.feeds, feed_url)
    }

    /// Recursively find, remove, and return a feed from the config tree
    fn remove_and_return_feed(feeds: &mut Vec<FeedConfigItem>, feed_url: &str) -> Option<FeedSource> {
        for i in 0..feeds.len() {
            match &mut feeds[i] {
                FeedConfigItem::Standalone(feed_source) => {
                    if feed_source.feed.as_deref() == Some(feed_url) {
                        if let FeedConfigItem::Standalone(removed) = feeds.remove(i) {
                            return Some(removed);
                        }
                    }
                }
                FeedConfigItem::Group(group) => {
                    // Check if the feed is in this group's feeds
                    for j in 0..group.feeds.len() {
                        if let FeedConfigItem::Standalone(feed_source) = &group.feeds[j] {
                            if feed_source.feed.as_deref() == Some(feed_url) {
                                if let FeedConfigItem::Standalone(removed) = group.feeds.remove(j) {
                                    return Some(removed);
                                }
                            }
                        }
                    }
                    // Recursively check nested groups
                    if let Some(feed) = Self::remove_and_return_feed(&mut group.feeds, feed_url) {
                        return Some(feed);
                    }
                }
            }
        }
        None
    }

    /// Add a feed to the config at the specified group path (or root if None)
    fn paste_feed_to_config(&mut self, feed_source: FeedSource, target_group: Option<&str>) {
        let feed_item = FeedConfigItem::Standalone(feed_source);

        if let Some(group_path) = target_group {
            // Add to the specified group
            Self::paste_feed_into_group(&mut self.config.feeds, &feed_item, group_path);
        } else {
            // Add to root level
            self.config.feeds.push(feed_item);
        }

        // Save the config
        if let Err(e) = crate::config::save_feeds_only(&self.config.feeds) {
            self.status_message = Some(format!("Failed to save config: {}", e));
            return;
        }

        // Reload feeds from updated config
        self.reload_feeds_from_config();

        self.status_message = Some("Pasted feed".to_string());
    }

    /// Add a group to the config at the specified parent group path (or root if None)
    fn paste_group_to_config(&mut self, group: &FeedGroup, target_parent: Option<&str>) {
        let group_item = FeedConfigItem::Group(group.clone());

        if let Some(parent_path) = target_parent {
            // Add as a child of the specified parent group
            Self::paste_group_into_group(&mut self.config.feeds, &group_item, parent_path);
        } else {
            // Add to root level
            self.config.feeds.push(group_item);
        }

        // Save the config
        if let Err(e) = crate::config::save_feeds_only(&self.config.feeds) {
            self.status_message = Some(format!("Failed to save config: {}", e));
            return;
        }

        // Reload feeds from updated config
        self.reload_feeds_from_config();

        self.status_message = Some(format!("Pasted group: {}", group.title));
    }

    /// Insert a feed item into a group at the specified path (for paste)
    fn paste_feed_into_group(feeds: &mut Vec<FeedConfigItem>, feed_item: &FeedConfigItem, group_path: &str) {
        let components: Vec<&str> = group_path.split(" > ").collect();
        if components.is_empty() {
            return;
        }

        Self::paste_feed_into_group_recursive(feeds, feed_item, &components, 0);
    }

    fn paste_feed_into_group_recursive(feeds: &mut Vec<FeedConfigItem>, feed_item: &FeedConfigItem, components: &[&str], depth: usize) {
        if depth >= components.len() {
            return;
        }

        let current_title = components[depth];
        let is_last = depth == components.len() - 1;

        // Find the group at this level
        for item in feeds.iter_mut() {
            if let FeedConfigItem::Group(group) = item {
                if group.title == current_title {
                    if is_last {
                        // This is the target group, add the feed
                        group.feeds.push(feed_item.clone());
                    } else {
                        // Need to go deeper
                        Self::paste_feed_into_group_recursive(&mut group.feeds, feed_item, components, depth + 1);
                    }
                    return;
                }
            }
        }

        // Group not found - this shouldn't happen if the UI is consistent
        // But we'll create the group hierarchy if needed
        if is_last {
            let new_group = FeedGroup {
                title: current_title.to_string(),
                feeds: vec![feed_item.clone()],
            };
            feeds.push(FeedConfigItem::Group(new_group));
        } else {
            // Need to create intermediate groups
            let mut new_group = FeedGroup {
                title: current_title.to_string(),
                feeds: Vec::new(),
            };
            Self::paste_feed_into_group_recursive(&mut new_group.feeds, feed_item, components, depth + 1);
            feeds.push(FeedConfigItem::Group(new_group));
        }
    }

    /// Insert a group item into a parent group at the specified path (for paste)
    fn paste_group_into_group(feeds: &mut Vec<FeedConfigItem>, group_item: &FeedConfigItem, parent_path: &str) {
        let components: Vec<&str> = parent_path.split(" > ").collect();
        if components.is_empty() {
            return;
        }

        Self::paste_group_into_group_recursive(feeds, group_item, &components, 0);
    }

    fn paste_group_into_group_recursive(feeds: &mut Vec<FeedConfigItem>, group_item: &FeedConfigItem, components: &[&str], depth: usize) {
        if depth >= components.len() {
            return;
        }

        let current_title = components[depth];
        let is_last = depth == components.len() - 1;

        // Find the group at this level
        for item in feeds.iter_mut() {
            if let FeedConfigItem::Group(group) = item {
                if group.title == current_title {
                    if is_last {
                        // This is the target parent group, add the group
                        group.feeds.push(group_item.clone());
                    } else {
                        // Need to go deeper
                        Self::paste_group_into_group_recursive(&mut group.feeds, group_item, components, depth + 1);
                    }
                    return;
                }
            }
        }

        // Parent group not found - create the hierarchy
        if is_last {
            let new_group = FeedGroup {
                title: current_title.to_string(),
                feeds: vec![group_item.clone()],
            };
            feeds.push(FeedConfigItem::Group(new_group));
        } else {
            let mut new_group = FeedGroup {
                title: current_title.to_string(),
                feeds: Vec::new(),
            };
            Self::paste_group_into_group_recursive(&mut new_group.feeds, group_item, components, depth + 1);
            feeds.push(FeedConfigItem::Group(new_group));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FeedSource;

    #[test]
    fn test_to_strftime_format_default() {
        assert_eq!(to_strftime_format("D MMM YYYY"), ("%d %b %Y".to_string(), true));
    }

    #[test]
    fn test_to_strftime_format_double_digit_day() {
        assert_eq!(to_strftime_format("DD MMM YYYY"), ("%d %b %Y".to_string(), false));
    }

    #[test]
    fn test_to_strftime_format_full_month() {
        assert_eq!(to_strftime_format("D MMMM YYYY"), ("%d %B %Y".to_string(), true));
    }

    #[test]
    fn test_to_strftime_format_two_digit_year() {
        assert_eq!(to_strftime_format("D MMM YY"), ("%d %b %y".to_string(), true));
    }

    #[test]
    fn test_to_strftime_format_combined() {
        assert_eq!(to_strftime_format("DD MMMM YY"), ("%d %B %y".to_string(), false));
    }

    #[test]
    fn test_strip_day_leading_zero_single_digit_day() {
        assert_eq!(strip_day_leading_zero("02 Nov 2025"), "2 Nov 2025");
    }

    #[test]
    fn test_strip_day_leading_zero_double_digit_day() {
        assert_eq!(strip_day_leading_zero("12 Nov 2025"), "12 Nov 2025");
    }

    #[test]
    fn test_strip_day_leading_zero_first_of_month() {
        assert_eq!(strip_day_leading_zero("01 Nov 2025"), "1 Nov 2025");
    }

    #[test]
    fn test_strip_day_leading_zero_ninth_of_month() {
        assert_eq!(strip_day_leading_zero("09 Nov 2025"), "9 Nov 2025");
    }

    #[test]
    fn test_strip_day_leading_zero_tenth_of_month() {
        assert_eq!(strip_day_leading_zero("10 Nov 2025"), "10 Nov 2025");
    }

    #[test]
    fn test_to_strftime_format_abbreviated_weekday() {
        assert_eq!(to_strftime_format("ddd, D MMM YYYY"), ("%a, %d %b %Y".to_string(), true));
    }

    #[test]
    fn test_to_strftime_format_full_weekday() {
        assert_eq!(to_strftime_format("dddd, D MMMM YYYY"), ("%A, %d %B %Y".to_string(), true));
    }

    #[test]
    fn test_build_group_tree_with_empty_groups() {
        // Create some feeds
        let feeds = vec![
            db::Feed {
                id: 1,
                group_title: "Tech".to_string(),
                title: "Rust Blog".to_string(),
                url: "https://blog.rust-lang.org/feed.xml".to_string(),
                site_url: Some("https://blog.rust-lang.org/".to_string()),
                last_fetched: None,
                unread_count: 5,
            },
        ];

        // Create empty groups
        let empty_groups = vec![
            "News".to_string(),
            "Tech > Programming".to_string(),
        ];

        // Build the tree
        let tree = build_group_tree(&feeds, &empty_groups);

        // We should have 3 root nodes: "News", "Tech", and potentially nested groups
        assert_eq!(tree.len(), 2);

        // First node should be "News" (empty group)
        assert_eq!(tree[0].title, "News");
        assert_eq!(tree[0].feeds.len(), 0);
        assert_eq!(tree[0].unread_count, 0);

        // Second node should be "Tech" (with feeds)
        assert_eq!(tree[1].title, "Tech");
        assert_eq!(tree[1].feeds.len(), 1);
        assert_eq!(tree[1].unread_count, 5);

        // Tech should have a child "Programming" (empty group)
        assert_eq!(tree[1].children.len(), 1);
        assert_eq!(tree[1].children[0].title, "Programming");
        assert_eq!(tree[1].children[0].feeds.len(), 0);
        assert_eq!(tree[1].children[0].unread_count, 0);
    }

    #[test]
    fn test_build_group_tree_without_empty_groups() {
        // Create some feeds
        let feeds = vec![
            db::Feed {
                id: 1,
                group_title: "Tech".to_string(),
                title: "Rust Blog".to_string(),
                url: "https://blog.rust-lang.org/feed.xml".to_string(),
                site_url: Some("https://blog.rust-lang.org/".to_string()),
                last_fetched: None,
                unread_count: 5,
            },
        ];

        // Build the tree without empty groups
        let tree = build_group_tree(&feeds, &[]);

        // We should have 1 root node: "Tech"
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].title, "Tech");
        assert_eq!(tree[0].feeds.len(), 1);
        assert_eq!(tree[0].unread_count, 5);
    }

    #[test]
    fn test_remove_feed_recursive_standalone() {
        let mut feeds = vec![
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
        ];

        let removed = App::remove_feed_recursive(&mut feeds, "https://bair.berkeley.edu/blog/feed.xml");
        assert!(removed);
        assert_eq!(feeds.len(), 1);
        match &feeds[0] {
            FeedConfigItem::Group(g) => {
                assert_eq!(g.title, "Tech");
                assert_eq!(g.feeds.len(), 1);
            }
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn test_remove_feed_recursive_in_group() {
        let mut feeds = vec![
            FeedConfigItem::Group(FeedGroup {
                title: "Tech".to_string(),
                feeds: vec![
                    FeedConfigItem::Standalone(FeedSource {
                        title: "Rust Blog".to_string(),
                        url: "https://blog.rust-lang.org/".to_string(),
                        feed: Some("https://blog.rust-lang.org/feed.xml".to_string()),
                    }),
                    FeedConfigItem::Standalone(FeedSource {
                        title: "Go Blog".to_string(),
                        url: "https://go.dev/blog/".to_string(),
                        feed: Some("https://go.dev/blog/feed.xml".to_string()),
                    }),
                ],
            }),
        ];

        let removed = App::remove_feed_recursive(&mut feeds, "https://blog.rust-lang.org/feed.xml");
        assert!(removed);
        assert_eq!(feeds.len(), 1);
        match &feeds[0] {
            FeedConfigItem::Group(g) => {
                assert_eq!(g.title, "Tech");
                assert_eq!(g.feeds.len(), 1);
                match &g.feeds[0] {
                    FeedConfigItem::Standalone(f) => {
                        assert_eq!(f.title, "Go Blog");
                    }
                    _ => panic!("Expected standalone feed"),
                }
            }
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn test_remove_group_recursive_simple() {
        let mut feeds = vec![
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
        ];

        let removed = App::remove_group_recursive(&mut feeds, "Tech");
        assert!(removed);
        assert_eq!(feeds.len(), 1);
        match &feeds[0] {
            FeedConfigItem::Standalone(f) => {
                assert_eq!(f.title, "BAIR");
            }
            _ => panic!("Expected standalone feed"),
        }
    }

    #[test]
    fn test_remove_group_recursive_nested() {
        let mut feeds = vec![
            FeedConfigItem::Group(FeedGroup {
                title: "News".to_string(),
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
            }),
        ];

        // Remove nested group "News > Domestic"
        let removed = App::remove_group_recursive(&mut feeds, "News > Domestic");
        assert!(removed);
        assert_eq!(feeds.len(), 1);
        match &feeds[0] {
            FeedConfigItem::Group(g) => {
                assert_eq!(g.title, "News");
                assert_eq!(g.feeds.len(), 1);
                match &g.feeds[0] {
                    FeedConfigItem::Standalone(f) => {
                        assert_eq!(f.title, "Foreign Policy");
                    }
                    _ => panic!("Expected standalone feed"),
                }
            }
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn test_remove_group_recursive_removes_parent_if_empty() {
        let mut feeds = vec![
            FeedConfigItem::Group(FeedGroup {
                title: "News".to_string(),
                feeds: vec![
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
            }),
        ];

        // Remove nested group "News > Domestic"
        let removed = App::remove_group_recursive(&mut feeds, "News > Domestic");
        assert!(removed);
        // After removing "Domestic", the parent "News" should still exist (now empty)
        // Empty groups are preserved and will be shown in the UI
        assert_eq!(feeds.len(), 1);
        match &feeds[0] {
            FeedConfigItem::Group(g) => {
                assert_eq!(g.title, "News");
                assert_eq!(g.feeds.len(), 0); // Empty after removing Domestic
            }
            _ => panic!("Expected group"),
        }
    }
}
