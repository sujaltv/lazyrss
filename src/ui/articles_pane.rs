use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

use crate::app::{strip_day_leading_zero, to_strftime_format, ActivePane, App};
use crate::ui::theme;

/// Wrap text to fit within a maximum width, returning a vector of lines.
fn wrap_text(text: &str, max_width: usize, max_lines: usize) -> Vec<String> {
    if text.is_empty() || max_width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut line_chars = 0;

    for word in text.split_whitespace() {
        let word_len = word.chars().count();

        // Check if adding this word would exceed the width
        if line_chars > 0 && line_chars + 1 + word_len > max_width {
            // Start a new line
            lines.push(current_line);
            current_line = String::new();
            line_chars = 0;

            // Stop if we've reached max lines
            if lines.len() >= max_lines {
                break;
            }
        }

        // Add the word to the current line
        if line_chars > 0 {
            current_line.push(' ');
            line_chars += 1;
        }
        current_line.push_str(word);
        line_chars += word_len;
    }

    // Add the last line if it has content, but check max_lines limit
    if !current_line.is_empty() && lines.len() < max_lines {
        lines.push(current_line);
    }

    // If we got no lines but have text, return at least one line
    if lines.is_empty() && !text.is_empty() {
        // Truncate to fit max_width
        let truncated: String = text.chars().take(max_width).collect();
        lines.push(truncated);
    }

    lines
}

/// Render the middle articles pane.
///
/// Displays a list of articles for the currently selected feed.  Each entry
/// is two lines:
/// - Line 1: read/unread dot, optional star, and article title
/// - Line 2: author (if available) and right-aligned publication date
pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_style = theme::get_border_style(
        app.active_pane == ActivePane::Articles,
        &app.config.display.colours,
    );

    let border_type = theme::get_border_type(&app.config.display.colours);

    let block = Block::default()
        .title(" Articles ")
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_type(border_type);

    // Inner width after borders (2 columns for left+right border).
    let inner_width = area.width.saturating_sub(2) as usize;

    // Get date format from config
    let (date_format, strip_day_zero) = to_strftime_format(&app.config.display.format.date);

    // Get title lines config (minimum 1, max as configured)
    let title_lines = app.config.display.format.title_lines.max(1) as usize;

    // Get the currently selected article index for relative numbering
    let selected_idx = app.articles_state.selected().unwrap_or(0);

    let items: Vec<ListItem> = app
        .articles
        .iter()
        .enumerate()
        .map(|(idx, article)| {
            let base_style = if article.is_read {
                theme::READ_STYLE
            } else {
                theme::UNREAD_STYLE
            };

            // Relative article number (vim-style: distance from selected article)
            let article_num = if idx == selected_idx {
                // Selected article - show indicator
                Span::styled("> ", theme::get_unread_indicator_style(&app.config.display.colours))
            } else {
                // Calculate relative distance
                let relative = (idx as i32 - selected_idx as i32).unsigned_abs();
                Span::styled(format!("{} ", relative), theme::META_STYLE)
            };

            // Read/unread dot.
            let unread_style = theme::get_unread_indicator_style(&app.config.display.colours);
            let dot = if article.is_read {
                Span::styled("\u{25CB} ", theme::READ_STYLE)
            } else {
                Span::styled("\u{25CF} ", unread_style)
            };

            // Star indicator.
            let star = if article.is_starred {
                Span::styled("\u{2605} ", theme::STAR_STYLE)
            } else {
                Span::raw("")
            };

            // === Title Lines (wrappable) ===
            // Budget for title: full width minus article number, dot and star
            let prefix_len = 2 + 2 + if article.is_starred { 2 } else { 0 };
            let title_budget = inner_width.saturating_sub(prefix_len);

            // Wrap title to fit within the configured number of lines
            let title_lines_result = wrap_text(&article.title, title_budget, title_lines);

            // Ensure we have at least 1 line for title
            let title_lines_vec = if title_lines_result.is_empty() {
                vec![article.title.clone()]
            } else {
                title_lines_result
            };

            // Create title line vectors with article_num and dot repeated on first line only
            let mut all_lines: Vec<Line> = Vec::new();

            for (line_idx, title_line) in title_lines_vec.into_iter().enumerate() {
                let mut spans = Vec::new();

                if line_idx == 0 {
                    // First line: article number, dot, star, and title
                    spans.push(article_num.clone());
                    spans.push(dot.clone());
                    spans.push(star.clone());
                } else {
                    // Subsequent lines: indentation to align with title
                    spans.push(Span::raw("   ")); // 2 for number, 1 for dot
                    if article.is_starred {
                        spans.push(Span::raw("  ")); // extra space for star
                    }
                }

                if !title_line.is_empty() {
                    spans.push(Span::styled(title_line, base_style));
                }

                all_lines.push(Line::from(spans));
            }

            // === Line: Author and Date ===
            // Format date using config
            let date_str = article.published.as_ref().map(|dt| {
                let formatted = dt.format(&date_format).to_string();
                if strip_day_zero {
                    strip_day_leading_zero(&formatted)
                } else {
                    formatted
                }
            }).unwrap_or_default();

            // Metadata line: right-aligned date only
            let date_len = date_str.len();
            let date_padding = inner_width.saturating_sub(date_len);

            let meta_line = if !date_str.is_empty() {
                vec![
                    Span::raw(" ".repeat(date_padding)),
                    Span::styled(date_str, theme::META_STYLE),
                ]
            } else {
                vec![Span::raw("")]
            };

            // Separator line
            let separator_line = vec![Span::styled("─".repeat(inner_width.min(80)), theme::META_STYLE)];

            // Add metadata and separator lines
            all_lines.push(Line::from(meta_line));
            all_lines.push(Line::from(separator_line));

            ListItem::new(all_lines)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::get_highlight_style(&app.config.display.colours));

    frame.render_stateful_widget(list, area, &mut app.articles_state);
}
