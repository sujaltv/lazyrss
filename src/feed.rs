use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

use crate::db::{Article, Feed};

/// The result of fetching and parsing a single feed.
///
/// Produced by the background fetch tasks and sent back to the main loop
/// via an unbounded channel.
pub struct FeedUpdateResult {
    /// The database ID of the feed that was fetched.
    pub feed_id: i64,
    /// Newly parsed articles (not yet de-duplicated against the database).
    pub articles: Vec<Article>,
    /// If the fetch or parse failed, the error description.
    pub error: Option<String>,
}

/// Spawn background tasks to refresh every feed in the provided slice.
///
/// Each feed is fetched concurrently in its own Tokio task.  Results are
/// sent back through `tx` as they complete.
pub fn refresh_all(tx: &UnboundedSender<FeedUpdateResult>, feeds: &[Feed]) {
    let client = build_client();

    for feed in feeds {
        let tx = tx.clone();
        let client = client.clone();
        let feed = feed.clone();
        tokio::spawn(async move {
            let result = fetch_feed(&client, &feed).await;
            let _ = tx.send(result);
        });
    }
}

/// Spawn a background task to refresh a single feed.
pub fn refresh_one(tx: &UnboundedSender<FeedUpdateResult>, feed: &Feed) {
    let tx = tx.clone();
    let feed = feed.clone();
    tokio::spawn(async move {
        let client = build_client();
        let result = fetch_feed(&client, &feed).await;
        let _ = tx.send(result);
    });
}

/// Build a shared HTTP client with a reasonable timeout and user-agent.
/// Uses a browser-like user agent to avoid sites returning HTML to bots.
/// Gzip/deflate decompression is enabled by default in reqwest.
fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .build()
        .expect("Failed to create HTTP client")
}

/// Fetch and parse a single feed, returning a `FeedUpdateResult`.
///
/// Errors are captured into the result rather than propagated so that a
/// single misbehaving feed cannot take down the entire refresh cycle.
async fn fetch_feed(client: &reqwest::Client, feed: &Feed) -> FeedUpdateResult {
    match fetch_feed_inner(client, feed).await {
        Ok(articles) => FeedUpdateResult {
            feed_id: feed.id,
            articles,
            error: None,
        },
        Err(e) => FeedUpdateResult {
            feed_id: feed.id,
            articles: Vec::new(),
            error: Some(e.to_string()),
        },
    }
}

/// Inner implementation that can use `?` for ergonomic error handling.
async fn fetch_feed_inner(
    client: &reqwest::Client,
    feed: &Feed,
) -> Result<Vec<Article>, Box<dyn std::error::Error + Send + Sync>> {
    let url = &feed.url;
    let response = client
        .get(url)
        .header("Accept", "application/rss+xml, application/rdf+xml, application/atom+xml, application/xml, text/xml, */*")
        .send()
        .await?;

    // Check for HTTP errors
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()).into());
    }

    // Get the final URL (after redirects) for better error messages
    let final_url = response.url().clone();

    // Get content type for better error messages
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    // Get the response bytes - reqwest automatically handles gzip/deflate decompression
    let mut bytes = response.bytes().await?;

    // Check if we got actual content (not an empty response)
    if bytes.is_empty() {
        return Err("Empty response from feed".into());
    }

    // Remove UTF-8 BOM if present (some feeds include this)
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes = bytes.slice(3..);
    }

    // Check if the response looks like HTML (error page) instead of a feed
    let text = String::from_utf8_lossy(&bytes);
    let text_start = text.chars().take(200).collect::<String>();
    let is_html = text_start.contains("<html>") || text_start.contains("<HTML>") ||
                  text_start.contains("<!DOCTYPE html>") || text_start.contains("<!DOCTYPE HTML>");

    if is_html {
        return Err(format!(
            "Server returned HTML instead of feed (type: {}, URL: {})",
            content_type, final_url
        ).into());
    }

    // Try to parse with feed-rs
    let parsed = match feed_rs::parser::parse(&bytes[..]) {
        Ok(p) => p,
        Err(e) => {
            // On parse error, try to provide useful debug info
            let preview = text.chars().take(100).collect::<String>();
            return Err(format!(
                "Parse error (type: {}, {} bytes, URL: {}, starts: \"{}...\"): {}",
                content_type,
                bytes.len(),
                final_url,
                preview.replace('\n', "\\n"),
                e
            ).into());
        }
    };

    let articles: Vec<Article> = parsed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let guid = entry.id;

            // Skip entries without a guid/id (they can't be deduplicated)
            if guid.is_empty() {
                return None;
            }

            let title = entry
                .title
                .map(|t| t.content)
                .unwrap_or_else(|| "(untitled)".to_string());

            let url = entry.links.first().map(|l| l.href.clone());

            let author = entry.authors.first().map(|a| a.name.clone());

            let summary = entry.summary.map(|s| s.content);

            let content = entry.content.and_then(|c| c.body);

            let published = entry.published.or(entry.updated);

            Some(Article {
                id: 0,
                feed_id: feed.id,
                guid,
                title,
                url,
                author,
                summary,
                content,
                published,
                is_read: false,
                is_starred: false,
            })
        })
        .collect();

    Ok(articles)
}
