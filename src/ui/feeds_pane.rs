use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;
use ratatui::style::{Color, Style};

use crate::app::{ActivePane, App, FeedListItem};
use crate::ClipboardItem;
use crate::ui::theme;

/// Check if an item is currently in the clipboard (was cut)
fn is_item_cut(app: &App, item: &FeedListItem) -> bool {
    let clipboard = match &app.clipboard {
        Some(cb) => cb,
        None => return false,
    };

    match item {
        FeedListItem::GroupHeader { full_path, .. } => {
            match clipboard {
                ClipboardItem::Group { original_path, .. } => original_path == full_path,
                _ => false,
            }
        }
        FeedListItem::Feed { feed, .. } => {
            match clipboard {
                ClipboardItem::Feed { feed_source, .. } => {
                    feed_source.feed.as_deref() == Some(&feed.url)
                }
                _ => false,
            }
        }
        FeedListItem::All { .. } => false,
    }
}

/// Render the left-hand feeds pane.
///
/// Displays a grouped list of feeds.  Group headers show a collapse/expand
/// indicator; individual feeds show their title and unread count.
pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_style = theme::get_border_style(
        app.active_pane == ActivePane::Feeds,
        &app.config.display.colours,
    );

    let border_type = theme::get_border_type(&app.config.display.colours);

    let block = Block::default()
        .title(" Feeds ")
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_type(border_type);

    let unread_style = theme::get_unread_indicator_style(&app.config.display.colours);
    let cut_style = Style::default().fg(Color::Red); // Red color for cut items

    let items: Vec<ListItem> = app
        .feed_list_items
        .iter()
        .map(|item| {
            let is_cut = is_item_cut(app, item);

            match item {
                FeedListItem::All { unread_count } => {
                    let line = Line::from(vec![
                        Span::styled("All", theme::HEADER_STYLE),
                        Span::raw(" "),
                        Span::styled(format!("({})", unread_count), unread_style),
                    ]);
                    ListItem::new(line)
                }
                FeedListItem::GroupHeader { title, full_path: _, collapsed, unread_count, depth } => {
                    let indent = "  ".repeat(*depth as usize);
                    let prefix = if *collapsed { "\u{25B6} " } else { "\u{25BC} " };
                    let title_style = if is_cut {
                        cut_style
                    } else {
                        theme::HEADER_STYLE
                    };
                    let cut_indicator = if is_cut { " [cut]" } else { "" };
                    let line = Line::from(vec![
                        Span::raw(format!("{}{}", indent, prefix)),
                        Span::styled(format!("{}{}", title.clone(), cut_indicator), title_style),
                        Span::raw(" "),
                        Span::styled(format!("({})", unread_count), unread_style),
                    ]);
                    ListItem::new(line)
                }
                FeedListItem::Feed { feed, depth } => {
                    let indent = "  ".repeat(*depth as usize);
                    let base_style = if is_cut {
                        cut_style
                    } else if feed.unread_count > 0 {
                        theme::UNREAD_STYLE
                    } else {
                        theme::READ_STYLE
                    };
                    let cut_indicator = if is_cut { " [cut]" } else { "" };
                    let line = Line::from(vec![
                        Span::styled(format!("{}{}{}", indent, feed.title, cut_indicator), base_style),
                        Span::raw(" "),
                        Span::styled(format!("({})", feed.unread_count), unread_style),
                    ]);
                    ListItem::new(line)
                }
            }
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::get_highlight_style(&app.config.display.colours));

    frame.render_stateful_widget(list, area, &mut app.feeds_state);
}
