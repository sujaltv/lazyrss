use ratatui::layout::Alignment;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

/// Represents an active popup modal
#[derive(Debug)]
pub enum Popup {
    /// Create group popup with current input
    CreateGroup { input: String },
    /// Create feed popup with multi-field input
    CreateFeed {
        title: String,
        url: String,
        feed_url: String,
        selected_field: usize, // 0=title, 1=url, 2=feed_url
    },
    /// Edit feed popup with pre-populated fields
    EditFeed {
        original_url: String,  // Original URL to identify the feed
        title: String,
        url: String,
        feed_url: String,
        selected_field: usize, // 0=title, 1=url, 2=feed_url
    },
    /// Edit group popup with pre-populated title
    EditGroup {
        original_path: String,  // Original path to identify the group
        input: String,
    },
}

impl Popup {
    /// Create a new create_group popup
    pub fn create_group() -> Self {
        Self::CreateGroup {
            input: String::new(),
        }
    }

    /// Create a new create_feed popup
    pub fn create_feed() -> Self {
        Self::CreateFeed {
            title: String::new(),
            url: String::new(),
            feed_url: String::new(),
            selected_field: 0,
        }
    }

    /// Create a new edit_feed popup with pre-populated fields
    pub fn edit_feed(original_url: String, title: String, url: String, feed_url: Option<String>) -> Self {
        Self::EditFeed {
            original_url,
            title,
            url,
            feed_url: feed_url.unwrap_or_default(),
            selected_field: 0,
        }
    }

    /// Create a new edit_group popup with pre-populated title
    pub fn edit_group(original_path: String, title: String) -> Self {
        Self::EditGroup {
            original_path,
            input: title,
        }
    }

    /// Get the title for this popup
    pub fn title(&self) -> &str {
        match self {
            Popup::CreateGroup { .. } => "Create Group",
            Popup::CreateFeed { .. } => "Create Feed",
            Popup::EditFeed { .. } => "Edit Feed",
            Popup::EditGroup { .. } => "Edit Group",
        }
    }

    /// Check if this is an edit popup (EditFeed or EditGroup)
    pub fn is_edit(&self) -> bool {
        matches!(self, Popup::EditFeed { .. } | Popup::EditGroup { .. })
    }

    /// Handle a character input event
    pub fn handle_char(&mut self, c: char) {
        match self {
            Popup::CreateGroup { input } | Popup::EditGroup { input, .. } => {
                if c != '\n' && c != '\t' && !c.is_control() {
                    input.push(c);
                }
            }
            Popup::CreateFeed { title, url, feed_url, selected_field }
            | Popup::EditFeed { title, url, feed_url, selected_field, .. } => {
                if c != '\n' && c != '\t' && !c.is_control() {
                    match selected_field {
                        0 => title.push(c),
                        1 => url.push(c),
                        2 => feed_url.push(c),
                        _ => {}
                    }
                }
            }
        }
    }

    /// Handle backspace
    pub fn handle_backspace(&mut self) {
        match self {
            Popup::CreateGroup { input } | Popup::EditGroup { input, .. } => {
                input.pop();
            }
            Popup::CreateFeed { title, url, feed_url, selected_field }
            | Popup::EditFeed { title, url, feed_url, selected_field, .. } => {
                match selected_field {
                    0 => { title.pop(); }
                    1 => { url.pop(); }
                    2 => { feed_url.pop(); }
                    _ => {}
                }
            }
        }
    }

    /// Handle tab to switch between fields (for multi-field popups)
    pub fn handle_tab(&mut self) {
        if let Popup::CreateFeed { selected_field, .. } | Popup::EditFeed { selected_field, .. } = self {
            *selected_field = (*selected_field + 1) % 3;
        }
    }

    /// Handle shift+tab to switch between fields backwards (for multi-field popups)
    pub fn handle_backtab(&mut self) {
        if let Popup::CreateFeed { selected_field, .. } | Popup::EditFeed { selected_field, .. } = self {
            *selected_field = if *selected_field == 0 {
                2
            } else {
                *selected_field - 1
            };
        }
    }

    /// Get the current input value (for single-field popups)
    pub fn input(&self) -> &str {
        match self {
            Popup::CreateGroup { input } | Popup::EditGroup { input, .. } => input,
            Popup::CreateFeed { .. } | Popup::EditFeed { .. } => "",
        }
    }

    /// Get the original URL (for EditFeed popup)
    pub fn original_url(&self) -> Option<&str> {
        match self {
            Popup::EditFeed { original_url, .. } => Some(original_url),
            _ => None,
        }
    }

    /// Get the original path (for EditGroup popup)
    pub fn original_path(&self) -> Option<&str> {
        match self {
            Popup::EditGroup { original_path, .. } => Some(original_path),
            _ => None,
        }
    }

    /// Confirm and return the input value as a tuple (title, url, feed_url, original_url)
    /// For CreateGroup/EditGroup, returns (name, "", None, None)
    /// For CreateFeed/EditFeed, feed_url is None if empty, otherwise Some(trimmed value)
    pub fn confirm(self) -> (String, String, Option<String>, Option<String>) {
        match self {
            Popup::CreateGroup { input } | Popup::EditGroup { input, .. } => {
                (input, String::new(), None, None)
            }
            Popup::CreateFeed { title, url, feed_url, .. } => {
                let feed = if feed_url.trim().is_empty() {
                    None
                } else {
                    Some(feed_url.trim().to_string())
                };
                (title.trim().to_string(), url.trim().to_string(), feed, None)
            }
            Popup::EditFeed { original_url, title, url, feed_url, .. } => {
                let feed = if feed_url.trim().is_empty() {
                    None
                } else {
                    Some(feed_url.trim().to_string())
                };
                (title.trim().to_string(), url.trim().to_string(), feed, Some(original_url))
            }
        }
    }

    /// Check if this is a create feed popup
    pub fn is_create_feed(&self) -> bool {
        matches!(self, Popup::CreateFeed { .. })
    }

    /// Check if this is an edit feed popup
    pub fn is_edit_feed(&self) -> bool {
        matches!(self, Popup::EditFeed { .. })
    }

    /// Get field names for multi-field popups
    pub fn field_names(&self) -> Option<Vec<&str>> {
        match self {
            Popup::CreateFeed { .. } | Popup::EditFeed { .. } => Some(vec!["Title", "URL", "Feed URL"]),
            _ => None,
        }
    }

    /// Get field values for multi-field popups
    pub fn field_values(&self) -> Option<Vec<&str>> {
        match self {
            Popup::CreateFeed { title, url, feed_url, .. }
            | Popup::EditFeed { title, url, feed_url, .. } => {
                Some(vec![title, url, feed_url])
            }
            _ => None,
        }
    }

    /// Get currently selected field index for multi-field popups
    pub fn selected_field(&self) -> Option<usize> {
        match self {
            Popup::CreateFeed { selected_field, .. } | Popup::EditFeed { selected_field, .. } => Some(*selected_field),
            _ => None,
        }
    }
}

/// Render a popup modal centered on screen
pub fn render_popup(frame: &mut Frame, popup: &Popup) {
    let area = frame.area();

    // Calculate popup size (max 60 chars wide, 15 rows tall for multi-field)
    let is_multi_field = popup.field_names().is_some();
    let width = area.width.min(60);
    let height = area.height.min(if is_multi_field { 15 } else { 10 });

    // Center the popup
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;
    let popup_area = ratatui::layout::Rect {
        x: area.x + x,
        y: area.y + y,
        width,
        height,
    };

    // Clear the area (dim background effect)
    frame.render_widget(Clear, popup_area);

    // Create the popup content
    let title = popup.title();

    let content = if let Some(field_names) = popup.field_names() {
        // Multi-field popup
        let field_values = popup.field_values().unwrap();
        let selected = popup.selected_field().unwrap();

        let mut lines = vec![Line::from("")];

        for (i, (name, value)) in field_names.iter().zip(field_values.iter()).enumerate() {
            let marker = if i == selected { ">" } else { " " };
            let cursor = if i == selected { "█" } else { "" };
            lines.push(Line::from(format!("{} {}:", marker, name)));
            lines.push(Line::from(format!("  {} {}{}", cursor, value, cursor)));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            "Tab".into(),
            ": Next field, ".into(),
            "Shift+Tab".into(),
            ": Prev field, ".into(),
            "Enter".into(),
            ": Confirm, ".into(),
            "Esc".into(),
            ": Cancel".into(),
        ]));

        lines
    } else {
        // Single-field popup (CreateGroup or EditGroup)
        let input = popup.input();
        let label = if matches!(popup, Popup::EditGroup { .. }) {
            "New name:"
        } else {
            "Group name:"
        };

        vec![
            Line::from(""),
            Line::from(label),
            Line::from(format!("> {}", input)),
            Line::from(""),
            Line::from(vec![
                "Enter".into(),
                ": Confirm, ".into(),
                "Esc".into(),
                ": Cancel".into(),
            ]),
        ]
    };

    // Create the popup block
    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(ratatui::style::Color::Cyan))
        .border_type(ratatui::widgets::BorderType::Rounded);

    let paragraph = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_group_popup_initial_state() {
        let popup = Popup::create_group();
        assert_eq!(popup.title(), "Create Group");
        assert_eq!(popup.input(), "");
    }

    #[test]
    fn test_popup_char_input() {
        let mut popup = Popup::create_group();
        popup.handle_char('T');
        popup.handle_char('e');
        popup.handle_char('s');
        popup.handle_char('t');
        assert_eq!(popup.input(), "Test");
    }

    #[test]
    fn test_popup_backspace() {
        let mut popup = Popup::create_group();
        popup.handle_char('H');
        popup.handle_char('i');
        popup.handle_backspace();
        assert_eq!(popup.input(), "H");
    }

    #[test]
    fn test_popup_confirm() {
        let mut popup = Popup::create_group();
        popup.handle_char('N');
        popup.handle_char('e');
        popup.handle_char('w');
        let result = popup.confirm();
        assert_eq!(result, ("New".to_string(), String::new(), None, None));
    }

    #[test]
    fn test_popup_ignores_control_chars() {
        let mut popup = Popup::create_group();
        popup.handle_char('\n');
        popup.handle_char('\t');
        popup.handle_char('\x01'); // SOH control character
        assert_eq!(popup.input(), "");
    }

    // CreateFeed popup tests
    #[test]
    fn test_create_feed_popup_initial_state() {
        let popup = Popup::create_feed();
        assert_eq!(popup.title(), "Create Feed");
        assert!(popup.is_create_feed());
        assert_eq!(popup.selected_field(), Some(0));
        let names = popup.field_names().unwrap();
        assert_eq!(names, vec!["Title", "URL", "Feed URL"]);
    }

    #[test]
    fn test_create_feed_popup_field_navigation() {
        let mut popup = Popup::create_feed();
        assert_eq!(popup.selected_field(), Some(0));

        popup.handle_tab();
        assert_eq!(popup.selected_field(), Some(1));

        popup.handle_tab();
        assert_eq!(popup.selected_field(), Some(2));

        popup.handle_tab();
        assert_eq!(popup.selected_field(), Some(0)); // Wrap around

        popup.handle_backtab();
        assert_eq!(popup.selected_field(), Some(2));
    }

    #[test]
    fn test_create_feed_popup_input() {
        let mut popup = Popup::create_feed();

        // Input title
        popup.handle_char('M');
        popup.handle_char('y');
        popup.handle_char('B');
        popup.handle_char('l');
        popup.handle_char('o');
        popup.handle_char('g');

        // Switch to URL field
        popup.handle_tab();

        // Input URL
        popup.handle_char('h');
        popup.handle_char('t');
        popup.handle_char('t');
        popup.handle_char('p');
        popup.handle_char(':');
        popup.handle_char('/');

        let values = popup.field_values().unwrap();
        assert_eq!(values[0], "MyBlog");
        assert_eq!(values[1], "http:/");
        assert_eq!(values[2], "");
    }

    #[test]
    fn test_create_feed_popup_backspace() {
        let mut popup = Popup::create_feed();
        popup.handle_char('T');
        popup.handle_char('e');
        popup.handle_char('s');
        popup.handle_tab(); // Switch to URL field
        popup.handle_char('U');
        popup.handle_char('R');
        popup.handle_char('L');
        popup.handle_backspace();
        popup.handle_backspace();

        let values = popup.field_values().unwrap();
        assert_eq!(values[0], "Tes"); // Title unchanged
        assert_eq!(values[1], "U"); // URL reduced
    }

    #[test]
    fn test_create_feed_popup_confirm() {
        let mut popup = Popup::create_feed();
        popup.handle_char('T');
        popup.handle_char('e');
        popup.handle_char('s');
        popup.handle_char('t');
        popup.handle_tab();
        popup.handle_char('h');
        popup.handle_char('t');
        popup.handle_char('t');
        popup.handle_char('p');
        popup.handle_tab();
        popup.handle_char('f');
        popup.handle_char('e');
        popup.handle_char('e');
        popup.handle_char('d');

        let (title, url, feed_url, _) = popup.confirm();
        assert_eq!(title, "Test");
        assert_eq!(url, "http");
        assert_eq!(feed_url, Some("feed".to_string()));
    }

    #[test]
    fn test_create_feed_popup_empty_feed_url_returns_none() {
        let mut popup = Popup::create_feed();
        popup.handle_char('T');
        popup.handle_char('e');
        popup.handle_char('s');
        popup.handle_char('t');
        popup.handle_tab();
        popup.handle_char('h');
        popup.handle_char('t');
        popup.handle_char('t');
        popup.handle_char('p');
        // Leave feed_url empty

        let (title, url, feed_url, _) = popup.confirm();
        assert_eq!(title, "Test");
        assert_eq!(url, "http");
        assert_eq!(feed_url, None); // Returns None when feed_url is empty
    }

    // EditFeed popup tests
    #[test]
    fn test_edit_feed_popup_initial_state() {
        let popup = Popup::edit_feed(
            "https://example.com/feed".to_string(),
            "My Feed".to_string(),
            "https://example.com".to_string(),
            Some("https://example.com/feed".to_string()),
        );
        assert_eq!(popup.title(), "Edit Feed");
        assert!(popup.is_edit_feed());
        assert_eq!(popup.original_url(), Some("https://example.com/feed"));
        assert_eq!(popup.selected_field(), Some(0));
        let names = popup.field_names().unwrap();
        assert_eq!(names, vec!["Title", "URL", "Feed URL"]);
    }

    #[test]
    fn test_edit_feed_popup_pre_populated() {
        let popup = Popup::edit_feed(
            "https://example.com/feed".to_string(),
            "Original Title".to_string(),
            "https://example.com".to_string(),
            Some("https://example.com/feed".to_string()),
        );

        let values = popup.field_values().unwrap();
        assert_eq!(values[0], "Original Title");
        assert_eq!(values[1], "https://example.com");
        assert_eq!(values[2], "https://example.com/feed");
    }

    #[test]
    fn test_edit_feed_popup_field_navigation() {
        let popup = Popup::edit_feed(
            "https://example.com/feed".to_string(),
            "Title".to_string(),
            "https://example.com".to_string(),
            None,
        );

        let mut p = popup;
        assert_eq!(p.selected_field(), Some(0));

        p.handle_tab();
        assert_eq!(p.selected_field(), Some(1));

        p.handle_tab();
        assert_eq!(p.selected_field(), Some(2));

        p.handle_tab();
        assert_eq!(p.selected_field(), Some(0)); // Wrap around
    }

    #[test]
    fn test_edit_feed_popup_confirm() {
        let mut popup = Popup::edit_feed(
            "https://example.com/feed".to_string(),
            "Original Title".to_string(),
            "https://example.com".to_string(),
            Some("https://example.com/feed".to_string()),
        );

        // Modify title
        popup.handle_tab(); // move to URL
        popup.handle_tab(); // move to Feed URL
        popup.handle_backtab(); // move back to URL
        popup.handle_backtab(); // move back to title

        // Change title
        while popup.field_values().unwrap()[0].len() > 0 {
            popup.handle_backspace();
        }
        popup.handle_char('N');
        popup.handle_char('e');
        popup.handle_char('w');
        popup.handle_char(' ');
        popup.handle_char('T');
        popup.handle_char('i');
        popup.handle_char('t');
        popup.handle_char('l');
        popup.handle_char('e');

        let (title, url, feed_url, original_url) = popup.confirm();
        assert_eq!(title, "New Title");
        assert_eq!(url, "https://example.com");
        assert_eq!(feed_url, Some("https://example.com/feed".to_string()));
        assert_eq!(original_url, Some("https://example.com/feed".to_string()));
    }

    // EditGroup popup tests
    #[test]
    fn test_edit_group_popup_initial_state() {
        let popup = Popup::edit_group("News > World".to_string(), "World".to_string());
        assert_eq!(popup.title(), "Edit Group");
        assert!(popup.is_edit());
        assert!(!popup.is_edit_feed());
        assert_eq!(popup.original_path(), Some("News > World"));
        assert_eq!(popup.input(), "World");
    }

    #[test]
    fn test_edit_group_popup_input() {
        let mut popup = Popup::edit_group("Tech".to_string(), "Technology".to_string());

        // Clear and type new name
        while popup.input().len() > 0 {
            popup.handle_backspace();
        }
        popup.handle_char('T');
        popup.handle_char('e');
        popup.handle_char('c');
        popup.handle_char('h');

        let (name, _, _, _) = popup.confirm();
        assert_eq!(name, "Tech");
    }

    #[test]
    fn test_edit_group_popup_confirm() {
        let mut popup = Popup::edit_group("News > Sports".to_string(), "Sports".to_string());

        popup.handle_char(' ');
        popup.handle_char('>');
        popup.handle_char(' ');
        for c in "Football".chars() {
            popup.handle_char(c);
        }

        // Get original_path before confirm() since confirm() takes ownership
        assert_eq!(popup.original_path(), Some("News > Sports"));

        let (name, _, _, _) = popup.confirm();
        assert_eq!(name, "Sports > Football");
    }

    #[test]
    fn test_is_edit_returns_true_for_edit_popups() {
        let edit_feed = Popup::edit_feed(
            "https://example.com/feed".to_string(),
            "Title".to_string(),
            "https://example.com".to_string(),
            None,
        );
        let edit_group = Popup::edit_group("News".to_string(), "News".to_string());
        let create_feed = Popup::create_feed();
        let create_group = Popup::create_group();

        assert!(edit_feed.is_edit());
        assert!(edit_group.is_edit());
        assert!(!create_feed.is_edit());
        assert!(!create_group.is_edit());
    }
}
