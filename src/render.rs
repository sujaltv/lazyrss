//! Async article content rendering.
//!
//! Converting HTML to plain text using `html2text` is CPU-intensive and can
//! block the TUI. This module provides async rendering using
//! `tokio::task::spawn_blocking`.

use tokio::sync::oneshot;

/// Request to render article content in the background.
pub struct RenderRequest {
    /// HTML content to render.
    pub html: String,
    /// Article title for the header.
    pub title: String,
    /// Optional author.
    pub author: Option<String>,
    /// Optional publication date.
    pub published: Option<String>,
    /// Date format string.
    pub date_format: String,
    /// Channel to send the rendered result.
    pub respond_to: oneshot::Sender<String>,
}

/// Spawn a blocking task to render article content.
///
/// The rendering is CPU-intensive (HTML parsing and text conversion),
/// so we use `spawn_blocking` to avoid blocking the main TUI thread.
pub fn spawn_render(req: RenderRequest) {
    tokio::task::spawn_blocking(move || {
        let content = render_content_blocking(
            &req.html,
            &req.title,
            &req.author,
            &req.published,
            &req.date_format,
        );
        // Send result; receiver may have been dropped if user navigated away.
        let _ = req.respond_to.send(content);
    });
}

/// Blocking implementation of content rendering.
///
/// This function is intentionally synchronous and should only be called
/// within a `spawn_blocking` task.
fn render_content_blocking(
    html: &str,
    title: &str,
    author: &Option<String>,
    published: &Option<String>,
    _date_format: &str,
) -> String {
    // Build a header block.
    let mut header = title.to_string();
    header.push('\n');

    if let Some(author) = author {
        header.push_str(&format!("By {author}\n"));
    }

    if let Some(published) = published {
        header.push_str(&format!("{published}\n"));
    }

    header.push_str("\n---\n\n");

    // Convert HTML to plain text using html2text.
    // The 80-column width matches the original implementation.
    let body = html2text::from_read(html.as_bytes(), 80);

    header + &body
}

/// Convenience function to create a render request and return the receiver.
///
/// Example:
/// ```ignore
/// let rx = render::render_article(
///     article.content.clone().unwrap_or_default(),
///     article.title.clone(),
///     article.author.clone(),
///     published.map(|d| d.format(&config.display.format.date).to_string()),
///     config.display.format.date.clone(),
/// );
/// // Later, in the event loop:
/// if let Ok(rendered) = rx.try_recv() {
///     app.article_content = rendered;
/// }
/// ```
pub fn render_article(
    html: String,
    title: String,
    author: Option<String>,
    published: Option<String>,
    date_format: String,
) -> oneshot::Receiver<String> {
    let (tx, rx) = oneshot::channel();
    spawn_render(RenderRequest {
        html,
        title,
        author,
        published,
        date_format,
        respond_to: tx,
    });
    rx
}
