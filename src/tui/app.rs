use crate::config::ExecutionMode;
use crate::copy_trading::{Monitor, TraderList};
use crate::discover::DiscoverState;
use crate::live::LiveState;
use crate::tui::layout::{
    DiscoverView, FixtureRow, Layout, ShortcutPair, SignalRow, SportsView, StrategyRow,
};
use crate::tui::theme::{
    self as theme_mod, add_custom_theme, current_theme_index, export_current_theme, import_theme,
    set_theme_index, theme_count, theme_name_at, ThemePalette, COLOR_PRESETS, THEME_CREATOR_ROLES,
};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const NUM_TABS: usize = 5;

const DISCOVER_PAGE: usize = 25;

/// One option in the Add trader dialog: pasted address or a Discover profile.
#[derive(Clone)]
enum CopyAddOption {
    PasteAddress(String),
    Profile { addr: String, name: String },
}

impl CopyAddOption {
    fn address(&self) -> &str {
        match self {
            CopyAddOption::PasteAddress(a) => a.as_str(),
            CopyAddOption::Profile { addr, .. } => addr.as_str(),
        }
    }
    fn display_line(&self) -> String {
        match self {
            CopyAddOption::PasteAddress(a) => {
                let short = a
                    .get(..14)
                    .map(|s| format!("{}…", s))
                    .unwrap_or_else(|| a.clone());
                format!("Add pasted address {}", short)
            }
            CopyAddOption::Profile { addr, name } => {
                let name = if name.is_empty() || name == "—" {
                    addr.get(..14)
                        .map(|s| format!("{}…", s))
                        .unwrap_or_else(|| addr.clone())
                } else {
                    name.clone()
                };
                format!("{} ({})", name, addr.get(..10).unwrap_or(addr))
            }
        }
    }
}

fn looks_like_address(s: &str) -> bool {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("0x") {
        rest.len() == 40 && rest.chars().all(|c| c.is_ascii_hexdigit())
    } else {
        s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
    }
}

fn normalize_pasted_address(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("0x") {
        s.to_string()
    } else {
        format!("0x{}", s)
    }
}

fn build_copy_add_options(
    search: &str,
    discover_entries: &[crate::discover::LeaderboardEntry],
) -> Vec<CopyAddOption> {
    let q = search.trim();
    if q.len() >= 40 && looks_like_address(q) {
        return vec![CopyAddOption::PasteAddress(normalize_pasted_address(q))];
    }
    if q.is_empty() {
        return discover_entries
            .iter()
            .take(25)
            .map(|e| CopyAddOption::Profile {
                addr: e.proxy_wallet.clone(),
                name: e.user_name.clone(),
            })
            .collect();
    }
    let ql = q.to_lowercase();
    discover_entries
        .iter()
        .filter(|e| {
            e.user_name.to_lowercase().contains(&ql) || e.proxy_wallet.to_lowercase().contains(&ql)
        })
        .take(25)
        .map(|e| CopyAddOption::Profile {
            addr: e.proxy_wallet.clone(),
            name: e.user_name.clone(),
        })
        .collect()
}

#[derive(Clone, Copy)]
enum DiscoverFilterDialog {
    Category(usize),
    Period(usize),
    Order(usize),
}

impl DiscoverFilterDialog {
    fn options_len(self) -> usize {
        match self {
            DiscoverFilterDialog::Category(_) => 9,
            DiscoverFilterDialog::Period(_) => 4,
            DiscoverFilterDialog::Order(_) => 2,
        }
    }
    fn next(self) -> Self {
        let len = self.options_len();
        match self {
            DiscoverFilterDialog::Category(i) => DiscoverFilterDialog::Category((i + 1) % len),
            DiscoverFilterDialog::Period(i) => DiscoverFilterDialog::Period((i + 1) % len),
            DiscoverFilterDialog::Order(i) => DiscoverFilterDialog::Order((i + 1) % len),
        }
    }
    fn prev(self) -> Self {
        let len = self.options_len();
        match self {
            DiscoverFilterDialog::Category(i) => {
                DiscoverFilterDialog::Category((i + len - 1) % len)
            }
            DiscoverFilterDialog::Period(i) => DiscoverFilterDialog::Period((i + len - 1) % len),
            DiscoverFilterDialog::Order(i) => DiscoverFilterDialog::Order((i + 1) % 2),
        }
    }
}

// Fixed widths for aligned columns. Selector uses 3 chars so columns don't shift.
const W_RANK: usize = 4;
const W_USER: usize = 12;
const W_VOL: usize = 12;
const W_PNL: usize = 10;
const W_ROI: usize = 6;
const W_TRADES: usize = 6;
const W_MAINLY: usize = 10;
const W_ADDR: usize = 12;

fn discover_view(
    entries: &[crate::discover::LeaderboardEntry],
    selected: Option<usize>,
    discover: &DiscoverState,
    copy_addresses: &[String],
    max_rows: usize,
) -> DiscoverView {
    let loading = discover.is_fetching();
    let filters = (
        discover.category_label(),
        discover.time_period_label(),
        discover.order_by_label(),
    );
    let (table, header, rows, scan_note) = if entries.is_empty() && !loading {
        (
            "No data. Press r to fetch.".to_string(),
            vec![],
            vec![],
            String::new(),
        )
    } else if entries.is_empty() {
        (String::new(), vec![], vec![], String::new())
    } else {
        let total = entries.len();
        let page_size = max_rows.min(total).max(1);
        let sel = selected.unwrap_or(0).min(total.saturating_sub(1));
        let start = (sel as i32 - (page_size as i32 / 2))
            .max(0)
            .min((total.saturating_sub(page_size)) as i32) as usize;
        let end = (start + page_size).min(total);
        let header = vec![
            "".to_string(),
            "Rank".to_string(),
            "User".to_string(),
            "Vol".to_string(),
            "P&L".to_string(),
            "ROI%".to_string(),
            "Trades".to_string(),
            "Mainly".to_string(),
            "Address".to_string(),
        ];
        let mut row_data = Vec::new();
        let is_copied = |addr: &str| copy_addresses.iter().any(|a| a == addr);
        for (idx, e) in entries[start..end].iter().enumerate() {
            let global_idx = start + idx;
            let selected_row = Some(global_idx) == selected;
            let copied = is_copied(&e.proxy_wallet);
            let roi = if e.vol > 0.0 {
                e.pnl / e.vol * 100.0
            } else {
                0.0
            };
            let roi_positive = roi > 0.0;
            let stats = discover.get_stats(&e.proxy_wallet);
            let (trades, mainly) = stats
                .map(|s| (s.trade_count.to_string(), s.top_category.clone()))
                .unwrap_or_else(|| ("…".to_string(), "…".to_string()));
            let user = truncate_user(&e.user_name, W_USER).to_string();
            let mainly_short = if mainly.len() > W_MAINLY {
                format!("{}…", &mainly[..W_MAINLY.saturating_sub(1)])
            } else {
                mainly.clone()
            };
            let addr_short = e
                .proxy_wallet
                .get(..10)
                .map(|s| format!("{}…", s))
                .unwrap_or_else(|| e.proxy_wallet.clone());
            let sel_mark = if selected_row { "►" } else { " " };
            let copy_mark = if copied { " ●" } else { "" };
            let cells = vec![
                format!("{}{}", sel_mark, copy_mark),
                e.rank.clone(),
                user,
                format!("{:.2}", e.vol),
                format!("{:.2}", e.pnl),
                format!("{:.1}%", roi),
                trades,
                mainly_short,
                addr_short,
            ];
            row_data.push((selected_row, roi_positive, copied, cells));
        }
        let table_str = format!(
            "Profiles: {} (showing {}-{})   ↑↓ / jk  scroll   a / Enter  add to copy",
            total,
            start + 1,
            end
        );
        let note = "Background scan: Trades + Mainly fill in as profiles are fetched.";
        (table_str, header, row_data, note.to_string())
    };
    DiscoverView {
        filters_category: filters.0,
        filters_period: filters.1,
        filters_order: filters.2,
        table,
        leaderboard_header: header,
        leaderboard_rows: rows,
        scan_note,
        loading,
    }
}

fn truncate_user(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

fn build_copy_list_content(
    addresses: Vec<String>,
    selected_index: Option<usize>,
    discover_entries: &[crate::discover::LeaderboardEntry],
) -> String {
    let mut out = String::new();
    for (i, addr) in addresses.iter().enumerate() {
        let mark = if Some(i) == selected_index {
            "► "
        } else {
            "  "
        };
        let short = addr
            .get(..10)
            .map(|s| format!("{}…", s))
            .unwrap_or_else(|| addr.clone());
        let username = discover_entries
            .iter()
            .find(|e| e.proxy_wallet == *addr)
            .map(|e| e.user_name.as_str())
            .filter(|s| !s.is_empty() && *s != "—");
        let line = match username {
            Some(u) => format!("{}{} ({})", mark, u, short),
            None => format!("{}{}", mark, short),
        };
        out.push_str(&line);
        out.push('\n');
    }
    if addresses.is_empty() {
        out.push_str("No profiles. Add from Discover (a/Enter) or see Shortcuts [?].");
    }
    out
}

fn build_recent_copy_trades_content(monitor: &Monitor) -> String {
    let rows = monitor.recent_trades(20);
    if rows.is_empty() {
        return "No copied trades yet. Start monitor (s).".to_string();
    }
    rows.into_iter()
        .map(|r| {
            let side = if r.side.len() > 4 {
                r.side.chars().take(4).collect::<String>()
            } else {
                r.side.clone()
            };
            format!(
                "{} {:>8.4} @ {:>5.3} | {} | {}",
                side, r.size, r.price, r.outcome, r.title
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[allow(clippy::too_many_arguments)]
fn build_sports_view(
    live: &LiveState,
    pane: usize,
    strategy_sel: usize,
    signal_sel: usize,
    fixture_sel: usize,
    league_filter: Option<&str>,
    team_filter: Option<&str>,
    show_league_picker: bool,
    show_team_picker: bool,
    league_picker_sel: usize,
    team_picker_sel: usize,
) -> SportsView {
    let s = live.sports.read().ok();

    // Build strategy rows.
    let strategies: Vec<StrategyRow> = s
        .as_ref()
        .map(|st| {
            st.strategy_configs
                .iter()
                .enumerate()
                .map(|(i, c)| StrategyRow {
                    id: c.id.to_string(),
                    name: c.name.to_string(),
                    enabled: c.enabled,
                    is_custom: c.is_custom,
                    selected: i == strategy_sel,
                })
                .collect()
        })
        .unwrap_or_default();

    // Build signal rows.
    let signals: Vec<SignalRow> = s
        .as_ref()
        .map(|st| {
            st.signals
                .iter()
                .enumerate()
                .map(|(i, sig)| SignalRow {
                    team: sig.home.clone(),
                    side: sig.side.clone(),
                    edge_pct: sig.edge_pct,
                    kelly_size: sig.kelly_size,
                    status: sig.status.clone(),
                    strategy_id: sig.strategy_id.clone(),
                    selected: i == signal_sel,
                })
                .collect()
        })
        .unwrap_or_default();
    let pending_count = signals.iter().filter(|s| s.status == "pending").count();

    // Build fixture rows (grouped by date) with optional league/team filter.
    let fixture_rows: Vec<FixtureRow> = s
        .as_ref()
        .map(|st| {
            let mut rows = Vec::new();
            let mut last_date = String::new();
            let mut sel_idx = 0usize;
            for f in &st.fixtures {
                // Apply filters.
                if let Some(lf) = league_filter {
                    // Simple check: league filter matches if team is in that league.
                    let league_match = st.leagues.iter().any(|l| {
                        l.short == lf
                            && (l.teams.iter().any(|t| t == &f.fixture.home)
                                || l.teams.iter().any(|t| t == &f.fixture.away))
                    });
                    if !league_match {
                        continue;
                    }
                }
                if let Some(tf) = team_filter {
                    if f.fixture.home != tf && f.fixture.away != tf {
                        continue;
                    }
                }

                if f.fixture.date != last_date {
                    rows.push(FixtureRow {
                        date: f.fixture.date.clone(),
                        home: String::new(),
                        away: String::new(),
                        has_market: false,
                        selected: false,
                        is_date_header: true,
                    });
                    last_date = f.fixture.date.clone();
                }
                let this_idx = rows.iter().filter(|r| !r.is_date_header).count();
                rows.push(FixtureRow {
                    date: f.fixture.date.clone(),
                    home: f.fixture.home.clone(),
                    away: f.fixture.away.clone(),
                    has_market: f.polymarket.is_some(),
                    selected: this_idx == fixture_sel,
                    is_date_header: false,
                });
                sel_idx = this_idx;
            }
            let _ = sel_idx;
            rows
        })
        .unwrap_or_default();

    // Build league picker items from embedded leagues.
    let league_picker_items: Vec<String> = s
        .as_ref()
        .map(|st| st.leagues.iter().map(|l| l.short.clone()).collect())
        .unwrap_or_default();

    // Build team picker based on current league filter.
    let team_picker_items: Vec<String> = s
        .as_ref()
        .map(|st| {
            if let Some(lf) = league_filter {
                st.leagues
                    .iter()
                    .find(|l| l.short == lf)
                    .map(|l| l.teams.clone())
                    .unwrap_or_default()
            } else {
                // All teams from all leagues.
                let mut teams: Vec<String> = st
                    .leagues
                    .iter()
                    .flat_map(|l| l.teams.iter().cloned())
                    .collect();
                teams.sort();
                teams.dedup();
                teams
            }
        })
        .unwrap_or_default();

    SportsView {
        pane,
        strategies,
        signals,
        fixtures: fixture_rows,
        pending_count,
        league_filter: league_filter.map(|s| s.to_string()),
        team_filter: team_filter.map(|s| s.to_string()),
        show_league_picker,
        show_team_picker,
        league_picker_items,
        team_picker_items,
        league_picker_sel,
        team_picker_sel,
    }
}

pub fn run(initial_config: crate::config::Config) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let config = Arc::new(std::sync::RwLock::new(initial_config));
    let live_state = Arc::new(LiveState::default());
    if let Ok(c) = config.read() {
        if let Some(b) = c.paper_bankroll {
            live_state.set_bankroll(Some(b));
        }
    }
    let trader_list = Arc::new(TraderList::new(Arc::clone(&config)));
    let copy_running = Arc::new(AtomicBool::new(false));
    let monitor = Arc::new(Monitor::new(
        Arc::clone(&trader_list),
        Some(Arc::clone(&live_state)),
        Arc::clone(&copy_running),
    ));

    let config_poll = Arc::clone(&config);
    let monitor_clone = Arc::clone(&monitor);
    std::thread::spawn(move || loop {
        monitor_clone.poll_once();
        let ms = config_poll
            .read()
            .map(|c| Monitor::poll_ms_from_config(&c))
            .unwrap_or(250);
        std::thread::sleep(Duration::from_millis(ms));
    });

    let live_clone = Arc::clone(&live_state);
    std::thread::spawn(move || loop {
        live_clone.fetch_all();
        std::thread::sleep(Duration::from_secs(8));
    });

    let discover_state = Arc::new(DiscoverState::new());
    {
        let d = Arc::clone(&discover_state);
        std::thread::spawn(move || d.fetch());
    }
    let discover_clone = Arc::clone(&discover_state);
    std::thread::spawn(move || loop {
        discover_clone.scan_next();
        std::thread::sleep(Duration::from_millis(500));
    });
    theme_mod::init_themes();
    let res = run_loop(
        &mut terminal,
        &monitor,
        &live_state,
        &trader_list,
        &discover_state,
        &copy_running,
        &config,
    );

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    monitor: &Monitor,
    live: &LiveState,
    trader_list: &Arc<TraderList>,
    discover: &Arc<DiscoverState>,
    copy_running: &AtomicBool,
    config: &Arc<std::sync::RwLock<crate::config::Config>>,
) -> Result<()> {
    let mut selected_tab = 0usize;
    let mut copy_selected: Option<usize> = None;
    let mut copy_add_dialog: Option<(String, usize)> = None; // (search_query, selected_index)
    let mut discover_selected: Option<usize> = None;
    let mut tab_execution_mode: [ExecutionMode; 5] = [ExecutionMode::Paper; 5];
    let mut show_theme_overlay = false;
    let mut show_shortcuts_screen = false;
    let mut theme_overlay_selection = 0usize;
    let mut theme_in_creator = false;
    let mut theme_creator_role = 0usize;
    let mut theme_creator_color_idx = 0usize;
    let mut theme_editor_palette: Option<ThemePalette> = None;
    let mut live_confirm_tab: Option<usize> = None;
    let mut bankroll_input: Option<String> = None;
    let mut show_currency_picker = false;
    let mut currency_filter = String::new();
    let mut currency_selected: usize = 0;
    const CURRENCIES: &[&str] = &["USD", "EUR", "GBP", "BTC", "ETH"];
    let mut discover_filter_dialog: Option<DiscoverFilterDialog> = None;

    /// Number of focusable/scrollable sections per tab: Crypto, Sports, Weather, Copy, Discover.
    const SECTIONS_PER_TAB: [usize; 5] = [1, 1, 1, 2, 1];
    const PAGE_SCROLL: usize = 10;
    let mut focused_section: usize = 0;
    let mut scroll_offsets: Vec<Vec<usize>> = vec![vec![0], vec![0], vec![0], vec![0, 0], vec![0]];

    // ── Sports tab UI state ────────────────────────────────────────────────
    let mut sports_pane: usize = 0; // 0=Strategies, 1=Signals, 2=Fixtures
    let mut sports_strategy_sel: usize = 0;
    let mut sports_signal_sel: usize = 0;
    let mut sports_fixture_sel: usize = 0;
    let mut sports_league_filter: Option<String> = None;
    let mut sports_team_filter: Option<String> = None;
    let mut sports_show_league_picker = false;
    let mut sports_show_team_picker = false;
    let mut sports_league_picker_sel: usize = 0;
    let mut sports_team_picker_sel: usize = 0;

    loop {
        let list = monitor.trader_list();
        let n_addr = list.len();
        if selected_tab == 3 {
            copy_selected = if n_addr == 0 {
                None
            } else if focused_section == 0 {
                Some(scroll_offsets[3][0].min(n_addr.saturating_sub(1)))
            } else {
                copy_selected.map(|i| i.min(n_addr.saturating_sub(1)))
            };
        }
        let discover_entries = discover.get_entries();
        if selected_tab == 4 {
            let n_d = discover_entries.len();
            discover_selected = if n_d == 0 {
                None
            } else {
                Some(scroll_offsets[4][0].min(n_d.saturating_sub(1)))
            };
        }

        let copy_list_text = build_copy_list_content(
            monitor.trader_list().get_addresses(),
            copy_selected,
            &discover_entries,
        );
        let copy_trades_text = build_recent_copy_trades_content(monitor);
        let copy_status_line = {
            let running = if copy_running.load(Ordering::SeqCst) {
                "Running"
            } else {
                "Stopped"
            };
            let (auto_exec, sizing) = config
                .read()
                .map(|c| {
                    let sizing = match c.copy_sizing {
                        crate::config::CopySizing::Proportional => "proportional",
                        crate::config::CopySizing::Fixed => "fixed",
                    };
                    (
                        if c.copy_auto_execute { "on" } else { "off" },
                        sizing.to_string(),
                    )
                })
                .unwrap_or_else(|_| ("off", "proportional".to_string()));
            let bankroll = live
                .global_stats
                .read()
                .ok()
                .and_then(|s| s.bankroll)
                .map(|b| format!("${:.2}", b))
                .unwrap_or_else(|| "—".to_string());
            format!(
                "Monitor: {} | Auto-execute: {} | Sizing: {} | Bankroll: {}",
                running, auto_exec, sizing, bankroll
            )
        };
        let copy_last_line = live
            .get_copy_logs()
            .into_iter()
            .rev()
            .find_map(|(_, msg)| msg.contains("Paper copy:").then_some(msg))
            .unwrap_or_else(|| "—".to_string());
        let copy_addresses = trader_list.get_addresses();
        type ShortcutCategory = (String, Vec<ShortcutPair>);
        let base_nav: ShortcutCategory = (
            "Navigation".into(),
            vec![
                ("E / Q".into(), "Next/prev tab".into()),
                ("1-5".into(), "Jump to tab".into()),
                ("Tab".into(), "Focus section".into()),
                ("↑/↓ j/k".into(), "Scroll line".into()),
                ("h/l ←/→".into(), "Scroll page".into()),
            ],
        );
        let base_global: ShortcutCategory = (
            "Global".into(),
            vec![
                ("Esc".into(), "Quit".into()),
                ("T".into(), "Theming".into()),
                ("b".into(), "Set bankroll".into()),
                ("C".into(), "P&L currency".into()),
            ],
        );
        let base_mode: ShortcutCategory = ("Mode".into(), vec![("m".into(), "Paper/Live".into())]);
        let shortcuts: Vec<ShortcutCategory> = match selected_tab {
            3 => vec![
                base_nav,
                base_global,
                base_mode,
                (
                    "Copy".into(),
                    vec![
                        ("s".into(), "Start/stop trading".into()),
                        ("a".into(), "Add trader".into()),
                        ("d".into(), "Remove selected".into()),
                    ],
                ),
            ],
            4 => vec![
                base_nav,
                base_global,
                base_mode,
                (
                    "Discover".into(),
                    vec![
                        ("r".into(), "Refresh".into()),
                        ("c".into(), "Category".into()),
                        ("t".into(), "Period".into()),
                        ("o".into(), "Order".into()),
                        ("a / Enter".into(), "Add to copy".into()),
                    ],
                ),
            ],
            _ => vec![base_nav, base_global, base_mode],
        };
        let mode_str = (selected_tab < 4).then(|| match tab_execution_mode[selected_tab] {
            ExecutionMode::Live => "Live",
            ExecutionMode::Paper => "Paper",
        });
        let copy_status = (selected_tab == 3).then(|| {
            if copy_running.load(Ordering::SeqCst) {
                "Running"
            } else {
                "Stopped"
            }
        });
        let copy_status_style = (selected_tab == 3).then(|| {
            if copy_running.load(Ordering::SeqCst) {
                theme_mod::Theme::success()
            } else {
                theme_mod::Theme::danger()
            }
        });
        terminal.draw(|f| {
            if show_shortcuts_screen {
                Layout::render_shortcuts_screen(f, &shortcuts);
            } else if show_theme_overlay {
                let n = theme_count();
                let theme_names: Vec<String> = (0..n).map(theme_name_at).collect();
                let cur = current_theme_index();
                let sel = if theme_in_creator {
                    theme_overlay_selection
                } else {
                    theme_overlay_selection.min(n + 2)
                };
                Layout::render_theme_screen(
                    f,
                    sel,
                    &theme_names,
                    cur,
                    theme_in_creator,
                    theme_creator_role,
                    theme_creator_color_idx,
                    theme_editor_palette.as_ref(),
                );
            } else {
                let area = f.area();
                let table_height = (area.height as usize).saturating_sub(17).clamp(1, 500);
                let discover_view = discover_view(
                    &discover_entries,
                    discover_selected,
                    discover,
                    &copy_addresses,
                    table_height,
                );
                let dv = (selected_tab == 4).then_some(&discover_view);
                let pnl_currency = config
                    .read()
                    .map(|c| c.pnl_currency.clone())
                    .unwrap_or_else(|_| "USD".to_string());
                let sports_view = if selected_tab == 1 {
                    Some(build_sports_view(
                        live,
                        sports_pane,
                        sports_strategy_sel,
                        sports_signal_sel,
                        sports_fixture_sel,
                        sports_league_filter.as_deref(),
                        sports_team_filter.as_deref(),
                        sports_show_league_picker,
                        sports_show_team_picker,
                        sports_league_picker_sel,
                        sports_team_picker_sel,
                    ))
                } else {
                    None
                };
                #[allow(clippy::needless_option_as_deref)]
                Layout::render(
                    f,
                    selected_tab,
                    &copy_list_text,
                    &copy_trades_text,
                    Some(copy_status_line.as_str()),
                    Some(copy_last_line.as_str()),
                    "",
                    dv,
                    live,
                    &shortcuts,
                    mode_str.as_deref(),
                    copy_status.as_deref(),
                    copy_status_style,
                    &scroll_offsets[selected_tab],
                    focused_section,
                    &pnl_currency,
                    sports_view.as_ref(),
                );
                if let Some(tab) = live_confirm_tab {
                    Layout::render_live_confirm(f, tab);
                }
                if let Some(dialog) = discover_filter_dialog {
                    let (title, options, selected) = match dialog {
                        DiscoverFilterDialog::Category(i) => (
                            "Category",
                            &[
                                "ALL",
                                "CRYPTO",
                                "SPORTS",
                                "POLITICS",
                                "CULTURE",
                                "WEATHER",
                                "ECONOMICS",
                                "TECH",
                                "FINANCE",
                            ][..],
                            i,
                        ),
                        DiscoverFilterDialog::Period(i) => {
                            ("Period", &["ALL", "DAY", "WEEK", "MONTH"][..], i)
                        }
                        DiscoverFilterDialog::Order(i) => ("Order", &["P&L", "VOL"][..], i),
                    };
                    Layout::render_discover_filter_dialog(f, title, options, selected);
                }
                if let Some(ref s) = bankroll_input {
                    Layout::render_bankroll_prompt(f, s);
                }
                if show_currency_picker {
                    let filtered: Vec<&str> = CURRENCIES
                        .iter()
                        .filter(|c| {
                            currency_filter.is_empty()
                                || c.to_lowercase().contains(&currency_filter.to_lowercase())
                        })
                        .copied()
                        .collect();
                    Layout::render_currency_picker(
                        f,
                        &filtered,
                        currency_selected,
                        &currency_filter,
                    );
                }
                if let Some((ref search, selected_index)) = copy_add_dialog {
                    let options = build_copy_add_options(search, &discover_entries);
                    let sel = selected_index.min(options.len().saturating_sub(1));
                    let option_rows: Vec<(String, bool)> = options
                        .iter()
                        .enumerate()
                        .map(|(i, o)| (o.display_line(), i == sel))
                        .collect();
                    Layout::render_copy_add_dialog(f, search, &option_rows);
                }
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if live_confirm_tab.is_some() {
                    match key.code {
                        KeyCode::Char('y') => {
                            if let Some(tab) = live_confirm_tab {
                                tab_execution_mode[tab] = ExecutionMode::Live;
                                live_confirm_tab = None;
                            }
                        }
                        KeyCode::Char('n') | KeyCode::Esc => live_confirm_tab = None,
                        _ => {}
                    }
                    continue;
                }
                if let Some(ref mut s) = bankroll_input {
                    match key.code {
                        KeyCode::Esc => bankroll_input = None,
                        KeyCode::Enter => {
                            if let Ok(v) = s.trim().parse::<f64>() {
                                if v >= 0.0 {
                                    live.set_bankroll(Some(v));
                                    if let Ok(mut c) = config.write() {
                                        c.paper_bankroll = Some(v);
                                        let _ = crate::config::save_config(&c);
                                    }
                                }
                            }
                            bankroll_input = None;
                        }
                        KeyCode::Backspace => {
                            s.pop();
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                            if c == '.' && s.contains('.') {
                            } else {
                                s.push(c);
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                if show_shortcuts_screen {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc => show_shortcuts_screen = false,
                        _ => {}
                    }
                    continue;
                }
                if let Some(dialog) = discover_filter_dialog {
                    match key.code {
                        KeyCode::Esc => discover_filter_dialog = None,
                        KeyCode::Enter => {
                            match dialog {
                                DiscoverFilterDialog::Category(i) => {
                                    discover.set_category_by_index(i)
                                }
                                DiscoverFilterDialog::Period(i) => {
                                    discover.set_time_period_by_index(i)
                                }
                                DiscoverFilterDialog::Order(i) => discover.set_order_by_index(i),
                            }
                            discover_filter_dialog = None;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            discover_filter_dialog = Some(dialog.prev());
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            discover_filter_dialog = Some(dialog.next());
                        }
                        _ => {}
                    }
                    continue;
                }
                if show_currency_picker {
                    match key.code {
                        KeyCode::Esc => {
                            show_currency_picker = false;
                            currency_filter.clear();
                        }
                        KeyCode::Enter => {
                            let filtered: Vec<&str> = CURRENCIES
                                .iter()
                                .filter(|c| {
                                    currency_filter.is_empty()
                                        || c.to_lowercase()
                                            .contains(&currency_filter.to_lowercase())
                                })
                                .copied()
                                .collect();
                            if let Some(&cur) = filtered.get(currency_selected) {
                                if let Ok(mut c) = config.write() {
                                    c.pnl_currency = cur.to_string();
                                    let _ = crate::config::save_config(&c);
                                }
                            }
                            show_currency_picker = false;
                            currency_filter.clear();
                        }
                        KeyCode::Backspace => {
                            currency_filter.pop();
                            currency_selected = 0;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let filtered_len = CURRENCIES
                                .iter()
                                .filter(|x| {
                                    currency_filter.is_empty()
                                        || x.to_lowercase()
                                            .contains(&currency_filter.to_lowercase())
                                })
                                .count();
                            currency_selected = currency_selected
                                .saturating_sub(1)
                                .min(filtered_len.saturating_sub(1));
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let filtered: Vec<&str> = CURRENCIES
                                .iter()
                                .filter(|x| {
                                    currency_filter.is_empty()
                                        || x.to_lowercase()
                                            .contains(&currency_filter.to_lowercase())
                                })
                                .copied()
                                .collect();
                            currency_selected =
                                (currency_selected + 1).min(filtered.len().saturating_sub(1));
                        }
                        KeyCode::Char(c) => {
                            currency_filter.push(c);
                            currency_selected = 0;
                        }
                        _ => {}
                    }
                    continue;
                }
                if show_theme_overlay {
                    let n = theme_count();
                    let total_items = n + 3; // themes + Export + Import + Creator
                    if theme_in_creator {
                        match key.code {
                            KeyCode::Esc => {
                                theme_in_creator = false;
                                theme_editor_palette = None;
                            }
                            KeyCode::Char('s') => {
                                if let Some(mut p) = theme_editor_palette.take() {
                                    p.name = format!("Custom {}", n);
                                    let idx = add_custom_theme(p);
                                    set_theme_index(idx);
                                    theme_in_creator = false;
                                }
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                theme_creator_role = (theme_creator_role + 1)
                                    .min(THEME_CREATOR_ROLES.len().saturating_sub(1));
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                theme_creator_role = theme_creator_role.saturating_sub(1);
                            }
                            KeyCode::Char('l') | KeyCode::Right => {
                                theme_creator_color_idx = (theme_creator_color_idx + 1)
                                    .min(COLOR_PRESETS.len().saturating_sub(1));
                                if let Some(ref mut p) = theme_editor_palette {
                                    p.set_role_color(
                                        theme_creator_role,
                                        COLOR_PRESETS[theme_creator_color_idx],
                                    );
                                }
                            }
                            KeyCode::Char('h') | KeyCode::Left => {
                                theme_creator_color_idx = theme_creator_color_idx.saturating_sub(1);
                                if let Some(ref mut p) = theme_editor_palette {
                                    p.set_role_color(
                                        theme_creator_role,
                                        COLOR_PRESETS[theme_creator_color_idx],
                                    );
                                }
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('T') | KeyCode::F(10) | KeyCode::Esc => {
                                show_theme_overlay = false;
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                theme_overlay_selection = (theme_overlay_selection + 1)
                                    .min(total_items.saturating_sub(1));
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                theme_overlay_selection = theme_overlay_selection.saturating_sub(1);
                            }
                            KeyCode::Enter => {
                                if theme_overlay_selection < n {
                                    set_theme_index(theme_overlay_selection);
                                } else if theme_overlay_selection == n + 2 {
                                    theme_in_creator = true;
                                    theme_editor_palette = Some(theme_mod::current_palette());
                                    theme_creator_role = 0;
                                    theme_creator_color_idx = 0;
                                }
                            }
                            KeyCode::Char('e') => {
                                let path = std::path::Path::new("theme_export.toml");
                                let _ = export_current_theme(path);
                            }
                            KeyCode::Char('i') => {
                                let path = std::path::Path::new("theme_import.toml");
                                if let Ok(idx) = import_theme(path) {
                                    set_theme_index(idx);
                                }
                            }
                            KeyCode::Char('c') => {
                                theme_in_creator = true;
                                theme_editor_palette = Some(theme_mod::current_palette());
                                theme_creator_role = 0;
                                theme_creator_color_idx = 0;
                            }
                            _ => {}
                        }
                    }
                    continue;
                }
                if let Some((ref mut search, ref mut selected_index)) = copy_add_dialog {
                    match key.code {
                        KeyCode::Esc => copy_add_dialog = None,
                        KeyCode::Backspace => {
                            search.pop();
                            *selected_index = 0;
                        }
                        KeyCode::Up => {
                            let options = build_copy_add_options(search, &discover_entries);
                            let n = options.len();
                            if n > 0 {
                                *selected_index = selected_index.saturating_sub(1).min(n - 1);
                            }
                        }
                        KeyCode::Down => {
                            let options = build_copy_add_options(search, &discover_entries);
                            let n = options.len();
                            if n > 0 {
                                *selected_index = (*selected_index + 1).min(n - 1);
                            }
                        }
                        KeyCode::Enter => {
                            let options = build_copy_add_options(search, &discover_entries);
                            let sel = (*selected_index).min(options.len().saturating_sub(1));
                            if let Some(opt) = options.get(sel) {
                                if trader_list.add(opt.address().to_string()) {
                                    live.push_copy_log(
                                        crate::live::LogLevel::Success,
                                        format!("Added {} to copy list", opt.address()),
                                    );
                                }
                                copy_add_dialog = None;
                            }
                        }
                        KeyCode::Char(c) => {
                            search.push(c);
                            *selected_index = 0;
                        }
                        _ => {}
                    }
                    continue;
                }
                match key.code {
                    KeyCode::Char('?') => show_shortcuts_screen = true,
                    KeyCode::Char('T') | KeyCode::F(10) => show_theme_overlay = true,
                    KeyCode::Char('b') => bankroll_input = Some(String::new()),
                    KeyCode::Char('C') => {
                        show_currency_picker = true;
                        currency_filter.clear();
                        currency_selected = 0;
                    }
                    KeyCode::Esc => break,
                    KeyCode::Char('q') => {
                        selected_tab = if selected_tab == 0 {
                            NUM_TABS - 1
                        } else {
                            selected_tab - 1
                        };
                        focused_section = 0;
                    }
                    KeyCode::Char('e') => {
                        selected_tab = if selected_tab + 1 >= NUM_TABS {
                            0
                        } else {
                            selected_tab + 1
                        };
                        focused_section = 0;
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        if selected_tab == 1 {
                            // Sports tab: switch to previous pane.
                            sports_pane = sports_pane.saturating_sub(1);
                        } else {
                            let num_sec = SECTIONS_PER_TAB[selected_tab];
                            if focused_section < num_sec {
                                let offs = &mut scroll_offsets[selected_tab][focused_section];
                                *offs = offs.saturating_sub(PAGE_SCROLL);
                            }
                        }
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if selected_tab == 1 {
                            // Sports tab: switch to next pane.
                            sports_pane = (sports_pane + 1).min(2);
                        } else {
                            let num_sec = SECTIONS_PER_TAB[selected_tab];
                            if focused_section < num_sec {
                                let offs = &mut scroll_offsets[selected_tab][focused_section];
                                *offs = offs.saturating_add(PAGE_SCROLL);
                            }
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if selected_tab == 1 {
                            match sports_pane {
                                0 => sports_strategy_sel = sports_strategy_sel.saturating_sub(1),
                                1 => sports_signal_sel = sports_signal_sel.saturating_sub(1),
                                2 => {
                                    scroll_offsets[1][0] = scroll_offsets[1][0].saturating_sub(1);
                                    sports_fixture_sel = sports_fixture_sel.saturating_sub(1);
                                }
                                _ => {}
                            }
                        } else {
                            let num_sec = SECTIONS_PER_TAB[selected_tab];
                            if focused_section < num_sec {
                                let offs = &mut scroll_offsets[selected_tab][focused_section];
                                *offs = offs.saturating_sub(1);
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if selected_tab == 1 {
                            match sports_pane {
                                0 => sports_strategy_sel += 1,
                                1 => sports_signal_sel += 1,
                                2 => {
                                    scroll_offsets[1][0] += 1;
                                    sports_fixture_sel += 1;
                                }
                                _ => {}
                            }
                        } else {
                            let num_sec = SECTIONS_PER_TAB[selected_tab];
                            if focused_section < num_sec {
                                let offs = &mut scroll_offsets[selected_tab][focused_section];
                                *offs = offs.saturating_add(1);
                            }
                        }
                    }
                    KeyCode::Tab => {
                        let num_sec = SECTIONS_PER_TAB[selected_tab];
                        focused_section = (focused_section + 1) % num_sec;
                    }
                    KeyCode::BackTab => {
                        let num_sec = SECTIONS_PER_TAB[selected_tab];
                        focused_section = (focused_section + num_sec - 1) % num_sec;
                    }
                    KeyCode::Char('1') => {
                        selected_tab = 0;
                        focused_section = 0;
                    }
                    KeyCode::Char('2') => {
                        selected_tab = 1;
                        focused_section = 0;
                    }
                    KeyCode::Char('3') => {
                        selected_tab = 2;
                        focused_section = 0;
                    }
                    KeyCode::Char('4') => {
                        selected_tab = 3;
                        focused_section = 0;
                    }
                    KeyCode::Char('5') => {
                        selected_tab = 4;
                        focused_section = 0;
                    }
                    KeyCode::Char('m') => {
                        let idx = selected_tab.min(4);
                        if tab_execution_mode[idx] == ExecutionMode::Paper {
                            live_confirm_tab = Some(idx);
                        } else {
                            tab_execution_mode[idx] = ExecutionMode::Paper;
                        }
                    }
                    _ => {
                        if selected_tab == 1 {
                            // Sports tab: picker overlays intercept all keys when open.
                            if sports_show_league_picker {
                                let n_leagues =
                                    live.sports.read().map(|s| s.leagues.len()).unwrap_or(0);
                                match key.code {
                                    KeyCode::Esc => sports_show_league_picker = false,
                                    KeyCode::Up | KeyCode::Char('k') => {
                                        sports_league_picker_sel =
                                            sports_league_picker_sel.saturating_sub(1);
                                    }
                                    KeyCode::Down | KeyCode::Char('j') => {
                                        sports_league_picker_sel = (sports_league_picker_sel + 1)
                                            .min(n_leagues.saturating_sub(1));
                                    }
                                    KeyCode::Enter => {
                                        let chosen = live.sports.read().ok().and_then(|s| {
                                            s.leagues
                                                .get(sports_league_picker_sel)
                                                .map(|l| l.short.clone())
                                        });
                                        sports_league_filter = chosen;
                                        sports_team_filter = None;
                                        sports_show_league_picker = false;
                                    }
                                    _ => {}
                                }
                            } else if sports_show_team_picker {
                                match key.code {
                                    KeyCode::Esc => sports_show_team_picker = false,
                                    KeyCode::Up | KeyCode::Char('k') => {
                                        sports_team_picker_sel =
                                            sports_team_picker_sel.saturating_sub(1);
                                    }
                                    KeyCode::Down | KeyCode::Char('j') => {
                                        sports_team_picker_sel += 1;
                                    }
                                    KeyCode::Enter => {
                                        // Build team list same way as build_sports_view.
                                        let chosen = live.sports.read().ok().and_then(|s| {
                                            let teams: Vec<String> =
                                                if let Some(lf) = sports_league_filter.as_deref() {
                                                    s.leagues
                                                        .iter()
                                                        .find(|l| l.short == lf)
                                                        .map(|l| l.teams.clone())
                                                        .unwrap_or_default()
                                                } else {
                                                    let mut t: Vec<String> = s
                                                        .leagues
                                                        .iter()
                                                        .flat_map(|l| l.teams.iter().cloned())
                                                        .collect();
                                                    t.sort();
                                                    t.dedup();
                                                    t
                                                };
                                            teams.into_iter().nth(sports_team_picker_sel)
                                        });
                                        sports_team_filter = chosen;
                                        sports_show_team_picker = false;
                                    }
                                    _ => {}
                                }
                            } else {
                                // Normal sports keys.
                                match key.code {
                                    KeyCode::Char(' ') if sports_pane == 0 => {
                                        // Toggle selected strategy.
                                        let id = live.sports.read().ok().and_then(|s| {
                                            s.strategy_configs
                                                .get(sports_strategy_sel)
                                                .map(|c| c.id)
                                        });
                                        if let Some(id) = id {
                                            if let Ok(mut s) = live.sports.write() {
                                                if let Some(cfg) = s
                                                    .strategy_configs
                                                    .iter_mut()
                                                    .find(|c| c.id == id)
                                                {
                                                    cfg.enabled = !cfg.enabled;
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Enter if sports_pane == 1 => {
                                        // Execute pending signal at selected index.
                                        if let Ok(mut s) = live.sports.write() {
                                            if let Some(sig) = s.signals.get_mut(sports_signal_sel)
                                            {
                                                if sig.status == "pending" {
                                                    sig.status = "done".to_string();
                                                    live.push_sports_log(
                                                        crate::live::LogLevel::Success,
                                                        format!(
                                                            "Executed {} {} on {}",
                                                            sig.side, sig.strategy_id, sig.home
                                                        ),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Char('d') if sports_pane == 1 => {
                                        // Dismiss pending signal.
                                        if let Ok(mut s) = live.sports.write() {
                                            if let Some(sig) = s.signals.get_mut(sports_signal_sel)
                                            {
                                                if sig.status == "pending" {
                                                    sig.status = "dismissed".to_string();
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Char('L') if sports_pane == 2 => {
                                        sports_show_league_picker = true;
                                        sports_league_picker_sel = 0;
                                    }
                                    KeyCode::Char('T') if sports_pane == 2 => {
                                        sports_show_team_picker = true;
                                        sports_team_picker_sel = 0;
                                    }
                                    KeyCode::Char('r') => {
                                        // Force refresh (background thread handles it; just reset scroll).
                                        scroll_offsets[1][0] = 0;
                                        sports_fixture_sel = 0;
                                        sports_signal_sel = 0;
                                        live.push_sports_log(
                                            crate::live::LogLevel::Info,
                                            "Manual refresh triggered".to_string(),
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        } else if selected_tab == 3 {
                            match key.code {
                                KeyCode::Char('s') => {
                                    let v = copy_running.load(Ordering::SeqCst);
                                    copy_running.store(!v, Ordering::SeqCst);
                                }
                                KeyCode::Char('a') | KeyCode::Enter => {
                                    copy_add_dialog = Some((String::new(), 0));
                                }
                                KeyCode::Char('d') => {
                                    let idx = copy_selected.unwrap_or(0);
                                    if idx < n_addr {
                                        trader_list.remove_at(idx);
                                        copy_selected = if n_addr <= 1 {
                                            None
                                        } else {
                                            Some(
                                                idx.saturating_sub(1).min(n_addr.saturating_sub(2)),
                                            )
                                        };
                                    }
                                }
                                _ => {}
                            }
                        } else if selected_tab == 4 {
                            match key.code {
                                KeyCode::Char('r') => {
                                    let d = Arc::clone(discover);
                                    std::thread::spawn(move || d.fetch());
                                }
                                KeyCode::Char('c') => {
                                    discover_filter_dialog = Some(DiscoverFilterDialog::Category(
                                        discover.category_index(),
                                    ));
                                }
                                KeyCode::Char('t') => {
                                    discover_filter_dialog = Some(DiscoverFilterDialog::Period(
                                        discover.time_period_index(),
                                    ));
                                }
                                KeyCode::Char('o') => {
                                    discover_filter_dialog = Some(DiscoverFilterDialog::Order(
                                        discover.order_by_index(),
                                    ));
                                }
                                KeyCode::Char('a') | KeyCode::Enter => {
                                    if let Some(i) = discover_selected {
                                        if let Some(e) = discover_entries.get(i) {
                                            let addrs = trader_list.get_addresses();
                                            if let Some(pos) =
                                                addrs.iter().position(|a| a == &e.proxy_wallet)
                                            {
                                                trader_list.remove_at(pos);
                                                live.push_copy_log(
                                                    crate::live::LogLevel::Info,
                                                    format!(
                                                        "Removed {} from copy list",
                                                        e.proxy_wallet
                                                    ),
                                                );
                                            } else if trader_list.add(e.proxy_wallet.clone()) {
                                                live.push_copy_log(
                                                    crate::live::LogLevel::Success,
                                                    format!(
                                                        "Added {} to copy list",
                                                        e.proxy_wallet
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
