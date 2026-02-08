//! Async database operations using tokio::spawn_blocking.
//!
//! rusqlite is a synchronous library, so all database operations would otherwise
//! block the main TUI thread. This module wraps each operation in a blocking task
//! and returns results via channels.

use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::db;

/// Commands that can be sent to the async database worker.
pub enum DbCommand {
    /// Get all feeds with unread counts.
    GetAllFeeds {
        respond_to: oneshot::Sender<anyhow::Result<Vec<db::Feed>>>,
    },

    /// Get articles for a specific feed.
    GetArticlesForFeed {
        feed_id: i64,
        respond_to: oneshot::Sender<anyhow::Result<Vec<db::Article>>>,
    },

    /// Get articles for all feeds in a group.
    GetArticlesForGroup {
        group_title: String,
        respond_to: oneshot::Sender<anyhow::Result<Vec<db::Article>>>,
    },

    /// Get all articles from all feeds.
    GetAllArticles {
        respond_to: oneshot::Sender<anyhow::Result<Vec<db::Article>>>,
    },

    /// Upsert articles (insert new ones, ignore existing by guid).
    UpsertArticles {
        articles: Vec<db::Article>,
        respond_to: oneshot::Sender<anyhow::Result<usize>>,
    },

    /// Toggle the read status of an article.
    ToggleRead {
        article_id: i64,
        respond_to: oneshot::Sender<anyhow::Result<bool>>,
    },

    /// Toggle the starred status of an article.
    ToggleStar {
        article_id: i64,
        respond_to: oneshot::Sender<anyhow::Result<bool>>,
    },

    /// Mark all articles in a feed as read.
    MarkAllRead {
        feed_id: i64,
        respond_to: oneshot::Sender<anyhow::Result<()>>,
    },

    /// Mark all articles across all feeds as read.
    MarkAllReadAll {
        respond_to: oneshot::Sender<anyhow::Result<()>>,
    },

    /// Update the last_fetched timestamp for a feed.
    UpdateLastFetched {
        feed_id: i64,
        respond_to: oneshot::Sender<anyhow::Result<()>>,
    },

    /// Sync feeds from config (add new feeds, update existing, delete removed).
    SyncFeedsFromConfig {
        config: crate::config::Config,
        respond_to: oneshot::Sender<anyhow::Result<()>>,
    },
}

/// An async wrapper around a synchronous SQLite database connection.
///
/// This type wraps a `rusqlite::Connection` and processes database commands
/// via an unbounded channel. Each command is executed in a blocking task
/// (using `tokio::task::spawn_blocking`) to avoid blocking the main TUI thread.
#[derive(Clone)]
pub struct AsyncDb {
    tx: mpsc::UnboundedSender<DbCommand>,
    /// Count of in-flight operations for monitoring purposes.
    in_flight: Arc<Mutex<usize>>,
}

impl AsyncDb {
    /// Create a new async database wrapper.
    ///
    /// Spawns a background task that processes database commands.
    /// The connection is moved into this task and all operations are
    /// executed via `spawn_blocking`.
    pub fn new(conn: rusqlite::Connection) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let conn = Arc::new(Mutex::new(conn));
        let in_flight = Arc::new(Mutex::new(0));

        // Spawn the command processor task.
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    DbCommand::GetAllFeeds { respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::get_all_feeds(&conn);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::GetArticlesForFeed { feed_id, respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::get_articles_for_feed(&conn, feed_id);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::GetArticlesForGroup { group_title, respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::get_articles_for_group(&conn, &group_title);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::GetAllArticles { respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::get_all_articles(&conn);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::UpsertArticles { articles, respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::upsert_articles(&conn, &articles);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::ToggleRead { article_id, respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::toggle_read(&conn, article_id);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::ToggleStar { article_id, respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::toggle_star(&conn, article_id);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::MarkAllRead { feed_id, respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::mark_all_read(&conn, feed_id);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::MarkAllReadAll { respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::mark_all_read_all(&conn);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::UpdateLastFetched { feed_id, respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::update_last_fetched(&conn, feed_id);
                            let _ = respond_to.send(result);
                        });
                    }
                    DbCommand::SyncFeedsFromConfig { config, respond_to } => {
                        let conn = Arc::clone(&conn);
                        tokio::task::spawn_blocking(move || {
                            let conn = conn.blocking_lock();
                            let result = db::sync_feeds_from_config(&conn, &config);
                            let _ = respond_to.send(result);
                        });
                    }
                }
            }
        });

        Self { tx, in_flight }
    }

    /// Get all feeds with unread counts.
    pub async fn get_all_feeds(&self) -> anyhow::Result<Vec<db::Feed>> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::GetAllFeeds { respond_to: tx })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Get articles for a specific feed.
    pub async fn get_articles_for_feed(&self, feed_id: i64) -> anyhow::Result<Vec<db::Article>> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::GetArticlesForFeed { feed_id, respond_to: tx })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Get articles for all feeds in a group.
    pub async fn get_articles_for_group(&self, group_title: &str) -> anyhow::Result<Vec<db::Article>> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::GetArticlesForGroup {
            group_title: group_title.to_string(),
            respond_to: tx,
        })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Get all articles from all feeds.
    pub async fn get_all_articles(&self) -> anyhow::Result<Vec<db::Article>> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::GetAllArticles { respond_to: tx })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Upsert articles (insert new ones, ignore existing by guid).
    pub async fn upsert_articles(&self, articles: Vec<db::Article>) -> anyhow::Result<usize> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::UpsertArticles { articles, respond_to: tx })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Toggle the read status of an article.
    pub async fn toggle_read(&self, article_id: i64) -> anyhow::Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::ToggleRead { article_id, respond_to: tx })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Toggle the starred status of an article.
    pub async fn toggle_star(&self, article_id: i64) -> anyhow::Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::ToggleStar { article_id, respond_to: tx })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Mark all articles in a feed as read.
    pub async fn mark_all_read(&self, feed_id: i64) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::MarkAllRead { feed_id, respond_to: tx })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Mark all articles across all feeds as read.
    pub async fn mark_all_read_all(&self) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::MarkAllReadAll { respond_to: tx })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Update the last_fetched timestamp for a feed.
    pub async fn update_last_fetched(&self, feed_id: i64) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::UpdateLastFetched { feed_id, respond_to: tx })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }

    /// Get the number of in-flight database operations.
    pub async fn in_flight_count(&self) -> usize {
        *self.in_flight.lock().await
    }

    /// Sync feeds from config (add new feeds, update existing, delete removed).
    pub async fn sync_feeds_from_config(&self, config: &crate::config::Config) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(DbCommand::SyncFeedsFromConfig {
            config: config.clone(),
            respond_to: tx,
        })
            .map_err(|_| anyhow::anyhow!("Database channel closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("Response channel closed"))?
    }
}
