use std::time::Duration;

use clap::Parser;
use crossterm::event::{KeyCode, KeyModifiers};
use lazyrss::{action, app::App, config, db, db_async::AsyncDb, event, ui};

const LONG_HELP: &str = r#"
CONFIGURATION
    Configuration file: $XDG_CONFIG_HOME/lazyrss/config.yaml
                       (typically ~/.config/lazyrss/config.yaml)

    Article database:  $XDG_DATA_HOME/lazyrss/news.db
                       (typically ~/.local/share/lazyrss/news.db)

    Example configuration:
        refresh_every: 300           # Auto-refresh interval (seconds)
        display:
          format:
            time: 12                 # 12 or 24 hour format
            date: "D MMM YYYY"
            title_lines: 2
          columns:
            feeds_list: 25           # Width percentages
            articles_list: 35
            article_view: 40
          colours:
            active_border: "cyan"
            inactive_border: "darkgray"
            border_type: "plain"     # plain, double, thick, rounded
            highlight_bg: "darkgray"
            unread_indicator: "cyan"
        feeds:
          - title: "Tech"
            feeds:
              - title: "Rust Blog"
                url: "https://blog.rust-lang.org/"
                feed: "https://blog.rust-lang.org/feed.xml"
        keybindings:
          global:
            quit: ["q", "Ctrl-c"]
            focus_next: "Tab"
            focus_prev: "Shift-Tab"
            refresh_current: "r"
            refresh_all: "R"
            open_browser: "o"
            jump_top: "g"
            jump_bottom: "G"
            create_group: "Ctrl-g"
            create_feed: "Ctrl-n"
          feeds:
            move_down: ["j", "Down"]
            move_up: ["k", "Up"]
            select: "Enter"
            toggle_collapse: "Space"
            expand_all: "e"
            collapse_all: "E"
          articles:
            move_down: ["j", "Down"]
            move_up: ["k", "Up"]
            select: "Enter"
            toggle_read: "m"
            toggle_star: "s"
            mark_all_read: "M"
          article_view:
            scroll_down: ["j", "Down"]
            scroll_up: ["k", "Up"]

KEYBINDINGS
    Global (work in all panes):
        q, Ctrl+c      Quit
        Tab            Focus next pane
        Shift+Tab      Focus previous pane
        r              Refresh current feed
        R              Refresh all feeds
        o              Open article in browser
        g              Jump to top
        G              Jump to bottom
        Ctrl+g         Create new group
        Ctrl+n         Create new feed

    Feeds Pane:
        j, ↓           Move down
        k, ↑           Move up
        Enter          Select feed/group
        Space          Mark all read (direct feeds only)
        Shift+Space    Mark all read (including nested groups)
        e              Expand all groups
        E              Collapse all groups
        Ctrl+d, PgDn   Scroll half-page down
        Ctrl+u, PgUp   Scroll half-page up
        Ctrl+e         Edit feed/group
        x              Cut feed/group
        p              Paste feed/group
        D, Shift+d     Delete selected feed/group

    Articles Pane:
        j, ↓           Move down
        k, ↑           Move up
        Enter          Mark as read and open
        m              Toggle read status
        s              Toggle star
        M              Mark all as read
        Ctrl+d, PgDn   Scroll half-page down
        Ctrl+u, PgUp   Scroll half-page up

    Article View:
        j, ↓           Scroll down
        k, ↑           Scroll up
        Ctrl+d, PgDn   Scroll half-page down
        Ctrl+u, PgUp   Scroll half-page up

    Vim-style counts are supported (e.g., 5j, 10k).

VISUAL INDICATORS
    ●   Unread article
    ○   Read article
    ★   Starred article

For complete documentation, see 'man lazyrss'.

Project homepage: https://github.com/sujaltv/lazyrss
"#;

/// LazyRSS - A terminal-based RSS/Atom feed reader TUI
#[derive(Parser, Debug)]
#[command(name = "lazyrss")]
#[command(author = "Sujal T. V. <sujal@svijay.com>")]
#[command(version)]
#[command(about = "A terminal-based RSS/Atom feed reader inspired by lazygit", long_about = None)]
#[command(after_help = LONG_HELP)]
struct Args {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments (handles --help, --version automatically)
    let _args = Args::parse();

    // 1. Load configuration from XDG config dir.
    let config = config::load()?;

    // 2. Initialize the SQLite database (creates tables if needed).
    let conn = db::initialize()?;

    // 3. Synchronize the config's feed list into the database.
    db::sync_feeds_from_config(&conn, &config)?;

    // 4. Build the async database wrapper.
    let async_db = AsyncDb::new(conn);

    // 5. Build the application state and extract the receivers
    let refresh_secs = config.refresh_every;
    let (mut app, mut feed_update_rx, mut db_result_rx, mut render_rx) = App::new_with_receivers(config, async_db);

    // 6. Set up the terminal for TUI rendering.
    let mut terminal = ratatui::init();

    // 7. Create the async event handler (250 ms tick rate).
    let mut events = event::EventHandler::new(250);

    // 8. Set up the periodic auto-refresh timer.
    let mut refresh_interval = tokio::time::interval(Duration::from_secs(refresh_secs));
    refresh_interval.tick().await; // consume the immediate first tick

    // 9. Main event loop.
    loop {
        // Draw the current state.
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        // Wait for the next event using tokio::select! with owned receivers
        tokio::select! {
            // User input events
            event = events.next() => {
                let event = event?;
                match &event {
                    event::Event::Key(key) if app.popup.is_some() => {
                        // Handle popup input
                        match key.code {
                            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                                app.handle_popup_char(c);
                            }
                            KeyCode::Backspace => {
                                app.handle_popup_backspace();
                            }
                            KeyCode::Tab => {
                                if key.modifiers.is_empty() {
                                    app.handle_popup_tab();
                                } else {
                                    // Shift+Tab (BackTab) - some terminals report this way
                                    app.handle_popup_backtab();
                                }
                            }
                            KeyCode::BackTab => {
                                // Some terminals report Shift+Tab as BackTab
                                app.handle_popup_backtab();
                            }
                            KeyCode::Enter => {
                                app.handle_popup_enter();
                            }
                            KeyCode::Esc => {
                                app.handle_popup_escape();
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        if let Some(act) = action::handle_event(&event, app.active_pane, &app.config.keybindings) {
                            app.update(act);
                        }
                    }
                }
            }
            // Feed update results (from HTTP fetching)
            Some(result) = feed_update_rx.recv() => {
                app.handle_feed_update(result);
            }
            // Database operation results
            Some(db_result) = db_result_rx.recv() => {
                app.handle_db_result(db_result);
            }
            // Render results (HTML to text conversion)
            Some(render_result) = render_rx.recv() => {
                app.handle_render_result(render_result);
            }
            // Periodic refresh tick
            _ = refresh_interval.tick() => {
                app.start_refresh_all();
            }
        }

        if app.should_quit {
            break;
        }
    }

    // 10. Restore the terminal to its original state.
    ratatui::restore();

    Ok(())
}
