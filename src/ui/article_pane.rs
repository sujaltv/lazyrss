use ratatui::layout::{Alignment, Rect};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{ActivePane, App};
use crate::ui::theme;

/// Render the right-hand article content pane.
///
/// When no article is selected the pane shows a placeholder message.
/// Otherwise it displays the pre-rendered plain-text content with vertical
/// scrolling support.
pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_style = theme::get_border_style(
        app.active_pane == ActivePane::ArticleView,
        &app.config.display.colours,
    );

    let border_type = theme::get_border_type(&app.config.display.colours);

    let block = Block::default()
        .title(" Article ")
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_type(border_type);

    if app.article_content.is_empty() {
        let placeholder = Paragraph::new("Select an article to read")
            .block(block)
            .alignment(Alignment::Center)
            .style(theme::META_STYLE);
        frame.render_widget(placeholder, area);
    } else {
        let text = Text::raw(&app.article_content);
        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((app.article_scroll, 0));
        frame.render_widget(paragraph, area);
    }
}
