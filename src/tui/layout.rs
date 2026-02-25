use ratatui::{
    layout::{Constraint, Direction, Layout as RLayout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};

use super::theme::{Theme, ThemePalette, COLOR_PRESETS, THEME_CREATOR_ROLES};
use crate::live::LiveState;

// ── Sports 3-pane view types ─────────────────────────────────────────────────

/// One row in the Strategies pane.
#[derive(Clone)]
pub struct StrategyRow {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub is_custom: bool,
    pub selected: bool,
}

/// One row in the Signal Feed pane.
#[derive(Clone)]
pub struct SignalRow {
    /// Short team display e.g. "Arsenal"
    pub team: String,
    pub side: String,
    pub edge_pct: f64,
    pub kelly_size: f64,
    /// "pending" | "auto" | "done" | "dismissed"
    pub status: String,
    pub strategy_id: String,
    pub selected: bool,
}

/// One row in the Fixtures pane (either a date header or a fixture line).
#[derive(Clone)]
pub struct FixtureRow {
    pub date: String,
    pub home: String,
    pub away: String,
    pub has_market: bool,
    pub selected: bool,
    pub is_date_header: bool,
}

/// All data the Sports tab render needs.
#[derive(Clone, Default)]
pub struct SportsView {
    /// 0=Strategies, 1=Signals, 2=Fixtures
    pub pane: usize,
    pub strategies: Vec<StrategyRow>,
    pub signals: Vec<SignalRow>,
    pub fixtures: Vec<FixtureRow>,
    pub pending_count: usize,
    pub league_filter: Option<String>,
    pub team_filter: Option<String>,
    /// Show league picker overlay
    pub show_league_picker: bool,
    pub show_team_picker: bool,
    pub league_picker_items: Vec<String>,
    pub team_picker_items: Vec<String>,
    pub league_picker_sel: usize,
    pub team_picker_sel: usize,
}

const TABS: &[&str] = &["Crypto", "Sports", "Weather", "Copy", "Discover"];
const PADDING_H: u16 = 0;
const PADDING_V: u16 = 0;
const TITLE_MARGIN_EXTRA: u16 = 0;
const MIN_TERMINAL_WIDTH: u16 = 60;
const MIN_TERMINAL_HEIGHT: u16 = 24;

pub struct Layout;

/// Shortcut entry for the Shortcuts block: (key, action).
pub type ShortcutPair = (String, String);

/// Discover tab: filters, optional table rows for Leaderboard, scan note, loading.
#[derive(Clone)]
pub struct DiscoverView {
    pub filters_category: String,
    pub filters_period: String,
    pub filters_order: String,
    pub table: String,
    /// Header row cell texts; then row_cells: (selected, roi_positive, copied, cells).
    pub leaderboard_header: Vec<String>,
    pub leaderboard_rows: Vec<(bool, bool, bool, Vec<String>)>,
    pub scan_note: String,
    pub loading: bool,
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

fn inner_area(area: Rect) -> Rect {
    let w = area.width.saturating_sub(2 * PADDING_H);
    let h = area.height.saturating_sub(2 * PADDING_V);
    Rect {
        x: area.x + PADDING_H,
        y: area.y + PADDING_V,
        width: w,
        height: h,
    }
}

fn bordered_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", title))
        .border_style(Theme::block_border())
        .title_style(Theme::block_title())
        .style(Style::default().bg(Theme::BG()))
}

fn bordered_block_error(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} — Error (see logs) ", title))
        .border_style(Theme::danger())
        .title_style(Theme::block_title())
        .style(Style::default().bg(Theme::BG()))
}

fn dimensions_too_small_messages() -> &'static [&'static str] {
    &[
        "Resize me, I'm not a fan of tight spaces.",
        "Your terminal is shy. Give it some room.",
        "Small screen, big dreams. Resize to continue.",
        "This terminal has seen smaller days. Resize up.",
        "Need more pixels. Enlarge the window.",
        "Like a bonsai, but for terminals. Resize to grow.",
        "Dimensions: smol. Required: less smol.",
        "The UI needs legroom. Resize the window.",
        "Too cozy. Expand the terminal.",
        "Increase dimensions or decrease expectations.",
    ]
}

impl Layout {
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        f: &mut Frame,
        selected_tab: usize,
        copy_list_content: &str,
        copy_trades_content: &str,
        copy_status_line: Option<&str>,
        copy_last_line: Option<&str>,
        discover_content: &str,
        discover_view: Option<&DiscoverView>,
        live: &LiveState,
        _shortcuts: &[(String, Vec<ShortcutPair>)],
        tab_mode: Option<&str>,
        copy_status: Option<&str>,
        copy_status_style: Option<Style>,
        section_scroll_offsets: &[usize],
        _focused_section: usize,
        pnl_currency: &str,
        sports_view: Option<&SportsView>,
    ) {
        let area = f.area();
        let bg = Block::default().style(Style::default().bg(Theme::BG()));
        f.render_widget(bg, area);

        if area.width < MIN_TERMINAL_WIDTH || area.height < MIN_TERMINAL_HEIGHT {
            let messages = dimensions_too_small_messages();
            let idx = (area.width as usize + area.height as usize) % messages.len();
            let sub = messages[idx];
            let inner = inner_area(area);
            let para = Paragraph::new(vec![
                Line::from(Span::styled(
                    "Myoro Polymarket Terminal",
                    Theme::block_title(),
                )),
                Line::from(Span::styled(sub, Theme::body())),
            ])
            .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(para, inner);
            return;
        }

        let inner = inner_area(area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let tab_index = selected_tab.min(TABS.len().saturating_sub(1));
        let is_main_tab = tab_index <= 3;
        let chunks = if is_main_tab {
            RLayout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(2),
                    Constraint::Length(1),
                ])
                .margin(0)
                .split(inner)
        } else {
            RLayout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(1),
                ])
                .margin(0)
                .split(inner)
        };

        let title_area = Rect {
            x: chunks[0].x + TITLE_MARGIN_EXTRA,
            y: chunks[0].y,
            width: chunks[0].width.saturating_sub(2 * TITLE_MARGIN_EXTRA),
            height: chunks[0].height,
        };
        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::block_border())
            .style(Style::default().bg(Theme::BG()));
        let inner = header_block.inner(title_area);
        f.render_widget(header_block, title_area);
        let title_center = " Myoro Polymarket Terminal ";
        let w = inner.width;
        let n = title_center.len() as u16;
        let left = (w.saturating_sub(n)) / 2;
        let right = w.saturating_sub(n).saturating_sub(left);
        let line_str: String =
            "─".repeat(left as usize) + title_center + &"─".repeat(right as usize);
        let header_para = Paragraph::new(Line::from(Span::styled(line_str, Theme::block_title())));
        f.render_widget(header_para, inner);

        let stats = live.global_stats.read().ok();
        let pnl_prefix = match pnl_currency.to_uppercase().as_str() {
            "USD" => "$",
            "EUR" => "€",
            "GBP" => "£",
            "BTC" => "BTC ",
            "ETH" => "ETH ",
            _ => "$",
        };
        let bankroll = stats
            .as_ref()
            .and_then(|s| s.bankroll)
            .map(|b| format!("{}{:.2}", pnl_prefix, b))
            .unwrap_or_else(|| "—".to_string());
        let pnl = stats.as_ref().map(|s| s.pnl).unwrap_or(0.0);
        let open_t = stats.as_ref().map(|s| s.open_trades).unwrap_or(0);
        let closed_t = stats.as_ref().map(|s| s.closed_trades).unwrap_or(0);
        let pnl_str = format!("{}{:.2}", pnl_prefix, pnl);
        let pnl_style = if pnl > 0.0 {
            Theme::success()
        } else if pnl < 0.0 {
            Theme::danger()
        } else {
            Theme::neutral_pnl()
        };
        let mode_str = tab_mode
            .map(|s| s.to_string())
            .unwrap_or_else(|| "—".to_string());
        let copy_str = copy_status
            .map(|s| s.to_string())
            .unwrap_or_else(|| "—".to_string());
        let copy_style = copy_status_style.unwrap_or_else(Theme::body);
        let metrics_block = bordered_block("Metrics");
        let metrics_inner = metrics_block.inner(chunks[1]);
        f.render_widget(metrics_block, chunks[1]);
        let segs = RLayout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Min(0),
                Constraint::Min(0),
                Constraint::Min(0),
                Constraint::Min(0),
                Constraint::Min(0),
            ])
            .split(metrics_inner);
        let open_s = open_t.to_string();
        let closed_s = closed_t.to_string();
        let groups = [
            ("P&L:", pnl_str.as_str(), pnl_style),
            ("Bankroll:", bankroll.as_str(), Theme::body()),
            ("Open:", open_s.as_str(), Theme::body()),
            ("Closed:", closed_s.as_str(), Theme::body()),
            ("Mode:", mode_str.as_str(), Theme::body()),
            ("Copy:", copy_str.as_str(), copy_style),
        ];
        for (rect, (label, value, value_style)) in segs.iter().zip(groups.iter()) {
            let line = Line::from(vec![
                Span::styled(format!("{} ", label), Theme::metrics_label()),
                Span::styled(*value, *value_style),
            ]);
            let para = Paragraph::new(line);
            f.render_widget(para, *rect);
        }

        let tab_index = selected_tab.min(TABS.len().saturating_sub(1));
        let tab_titles = TABS.iter().map(|t| Line::from(*t)).collect::<Vec<_>>();
        let tabs = Tabs::new(tab_titles)
            .block(bordered_block("Tabs"))
            .style(Theme::tab_default())
            .highlight_style(Theme::tab_selected())
            .select(tab_index);
        f.render_widget(tabs, chunks[2]);

        let (main_rect, trades_rect, indicator_idx) = if is_main_tab {
            (chunks[3], chunks[4], 5)
        } else {
            (chunks[3], Rect::default(), 4)
        };

        let content_chunk = main_rect;
        match tab_index {
            0 => {
                let c = live.crypto.read().ok();
                let btc_raw = c.as_ref().map(|c| c.btc_usdt.as_str()).unwrap_or("");
                let btc = if btc_raw.is_empty() || btc_raw == "—" {
                    "⏳ Loading…\n(Background fetch every 8s.)"
                } else {
                    btc_raw
                };
                let events = c.as_ref().map(|c| c.events.join("\n")).unwrap_or_default();
                let sub = RLayout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(2), Constraint::Min(4)])
                    .split(content_chunk);
                let news_block = bordered_block("News & Data");
                let news = Paragraph::new(btc)
                    .style(Theme::body())
                    .wrap(Wrap { trim: true })
                    .block(news_block);
                f.render_widget(news, sub[0]);
                let scroll = section_scroll_offsets.first().copied().unwrap_or(0) as u16;
                let events_block = Paragraph::new(events)
                    .style(Theme::body())
                    .wrap(Wrap { trim: true })
                    .scroll((scroll, 0))
                    .block(bordered_block("Active events (Gamma) — open markets"));
                f.render_widget(events_block, sub[1]);
            }
            1 => {
                if let Some(sv) = sports_view {
                    Self::render_sports_3pane(f, content_chunk, sv, section_scroll_offsets);
                } else {
                    // Fallback while state is initialising.
                    let p = Paragraph::new("⏳ Loading sports data…")
                        .style(Theme::body())
                        .wrap(Wrap { trim: true })
                        .block(bordered_block("Sports"));
                    f.render_widget(p, content_chunk);
                }
            }
            2 => {
                let w = live.weather.read().ok();
                let lines = w
                    .as_ref()
                    .map(|w| w.forecast.join("\n"))
                    .unwrap_or_default();
                let content = if lines.is_empty() {
                    "⏳ Loading…".to_string()
                } else {
                    format!("7-day forecast (NYC)\n\n{}", lines)
                };
                let scroll = section_scroll_offsets.first().copied().unwrap_or(0) as u16;
                let forecast_block = if live.last_log_is_error(2) {
                    bordered_block_error("Forecast")
                } else {
                    bordered_block("Forecast")
                };
                let forecast = Paragraph::new(content)
                    .style(Theme::body())
                    .wrap(Wrap { trim: true })
                    .scroll((scroll, 0))
                    .block(forecast_block);
                f.render_widget(forecast, content_chunk);
            }
            3 => {
                let split = RLayout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(content_chunk);
                let scroll0 = section_scroll_offsets.first().copied().unwrap_or(0) as u16;
                let scroll1 = section_scroll_offsets.get(1).copied().unwrap_or(0) as u16;
                let copy_block = Paragraph::new(copy_list_content)
                    .style(Theme::body())
                    .wrap(Wrap { trim: true })
                    .scroll((scroll0, 0))
                    .block(bordered_block("Copy List"));
                f.render_widget(copy_block, split[0]);
                let trades_block = Paragraph::new(copy_trades_content)
                    .style(Theme::body())
                    .wrap(Wrap { trim: true })
                    .scroll((scroll1, 0))
                    .block(bordered_block("Recent Trades"));
                f.render_widget(trades_block, split[1]);
            }
            4 => {
                if let Some(dv) = discover_view {
                    let sub = RLayout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(3), Constraint::Min(0)])
                        .split(content_chunk);
                    let filter_line = Line::from(vec![
                        Span::styled("[c] ", Theme::key()),
                        Span::styled("Category ", Theme::block_title()),
                        Span::raw(format!("{}  ", dv.filters_category)),
                        Span::styled("[t] ", Theme::key()),
                        Span::styled("Period ", Theme::block_title()),
                        Span::raw(format!("{}  ", dv.filters_period)),
                        Span::styled("[o] ", Theme::key()),
                        Span::styled("Order ", Theme::block_title()),
                        Span::raw(format!("{}  ", dv.filters_order)),
                        Span::styled("[r] ", Theme::key()),
                        Span::styled("Refresh", Theme::block_title()),
                    ]);
                    let filters_para = Paragraph::new(filter_line).block(bordered_block("Filters"));
                    f.render_widget(filters_para, sub[0]);
                    if dv.loading && dv.leaderboard_rows.is_empty() {
                        let p = Paragraph::new("⏳ Refreshing…")
                            .style(Theme::body())
                            .block(bordered_block("Leaderboard"));
                        f.render_widget(p, sub[1]);
                    } else if !dv.leaderboard_rows.is_empty() {
                        let scroll_off = section_scroll_offsets.first().copied().unwrap_or(0);
                        let visible_rows = sub[1].height.saturating_sub(1) as usize;
                        let end = (scroll_off + visible_rows).min(dv.leaderboard_rows.len());
                        let rows_slice = &dv.leaderboard_rows[scroll_off..end];
                        let widths = [
                            Constraint::Length(4),
                            Constraint::Length(4),
                            Constraint::Min(8),
                            Constraint::Min(8),
                            Constraint::Min(6),
                            Constraint::Length(6),
                            Constraint::Length(6),
                            Constraint::Min(8),
                            Constraint::Min(10),
                        ];
                        let header_cells = dv
                            .leaderboard_header
                            .iter()
                            .map(|s| Cell::from(s.as_str()).style(Theme::block_title()));
                        let header = Row::new(header_cells).height(1);
                        let rows: Vec<Row> = rows_slice
                            .iter()
                            .enumerate()
                            .map(|(idx, (selected, roi_pos, copied, cells))| {
                                let sel = idx == 0;
                                let cells: Vec<Cell> = cells
                                    .iter()
                                    .enumerate()
                                    .map(|(i, s)| {
                                        let style = if sel || *selected {
                                            Theme::tab_selected()
                                        } else if *copied || (i == 5 && *roi_pos) {
                                            Theme::success()
                                        } else {
                                            Theme::body()
                                        };
                                        Cell::from(s.as_str()).style(style)
                                    })
                                    .collect();
                                Row::new(cells).height(1)
                            })
                            .collect();
                        let table = Table::new(rows, widths)
                            .header(header)
                            .block(bordered_block("Leaderboard"))
                            .column_spacing(1);
                        f.render_widget(table, sub[1]);
                    } else {
                        let p = Paragraph::new(dv.table.as_str())
                            .style(Theme::body())
                            .block(bordered_block("Leaderboard"));
                        f.render_widget(p, sub[1]);
                    }
                } else {
                    let fallback = Paragraph::new(discover_content)
                        .style(Theme::body())
                        .wrap(Wrap { trim: true })
                        .block(bordered_block("Leaderboard"));
                    f.render_widget(fallback, content_chunk);
                }
            }
            _ => {}
        }

        let indicator_area = chunks[indicator_idx];
        if is_main_tab {
            let bottom_strip_y = trades_rect.y;
            let bottom_strip_h = area.height.saturating_sub(bottom_strip_y);
            if bottom_strip_h > 0 {
                let bottom_strip = Rect {
                    x: area.x,
                    y: bottom_strip_y,
                    width: area.width,
                    height: bottom_strip_h,
                };
                f.render_widget(
                    Block::default()
                        .style(Style::default().bg(Theme::BG()))
                        .borders(Borders::NONE),
                    bottom_strip,
                );
            }
        }

        if is_main_tab && trades_rect.width > 0 && trades_rect.height > 0 {
            let trades_horz = RLayout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(trades_rect);
            if tab_index == 3 {
                let status = copy_status_line.unwrap_or(
                    "Monitor: Stopped | Auto-execute: off | Sizing: proportional | Bankroll: —",
                );
                let last = copy_last_line.unwrap_or("—");
                let status_para = Paragraph::new(status)
                    .style(Theme::body())
                    .wrap(Wrap { trim: true })
                    .block(bordered_block(" Copy Status "));
                f.render_widget(status_para, trades_horz[0]);
                let last_para = Paragraph::new(last)
                    .style(Theme::body())
                    .wrap(Wrap { trim: true })
                    .block(bordered_block(" Last copy "));
                f.render_widget(last_para, trades_horz[1]);
            } else {
                let active_para = Paragraph::new("—")
                    .style(Theme::dim())
                    .block(bordered_block(" Active Trades "));
                f.render_widget(active_para, trades_horz[0]);
                let closed_para = Paragraph::new("—")
                    .style(Theme::dim())
                    .block(bordered_block(" Closed Trades "));
                f.render_widget(closed_para, trades_horz[1]);
            }
        }

        let (left_str, right_len) = match discover_view {
            Some(dv) if !dv.scan_note.is_empty() => {
                let max_left = indicator_area.width.saturating_sub(14) as usize;
                let left = &dv.scan_note;
                let left_str = if left.len() > max_left {
                    format!("{}…", &left[..max_left.saturating_sub(1)])
                } else {
                    left.clone()
                };
                (left_str, 14u16)
            }
            _ => (String::new(), 14u16),
        };
        let pad = indicator_area
            .width
            .saturating_sub(left_str.len() as u16)
            .saturating_sub(right_len);
        let hint_line = Line::from(vec![
            Span::styled(left_str.as_str(), Theme::dim()),
            Span::raw(" ".repeat(pad as usize)),
            Span::styled("[?] ", Theme::key()),
            Span::styled("Shortcuts", Theme::body()),
        ]);
        let hint_block = Block::default()
            .style(Style::default().bg(Theme::BG()))
            .borders(Borders::NONE);
        let para = Paragraph::new(hint_line).block(hint_block);
        f.render_widget(para, indicator_area);
    }

    /// Full-screen shortcuts: one bordered block per category, compact; background fills height.
    pub fn render_shortcuts_screen(f: &mut Frame, shortcuts: &[(String, Vec<ShortcutPair>)]) {
        let area = f.area();
        let bg = Block::default().style(Style::default().bg(Theme::BG()));
        f.render_widget(bg, area);
        if area.width == 0 || area.height == 0 {
            return;
        }
        let mut constraints: Vec<Constraint> = shortcuts
            .iter()
            .map(|(_, pairs)| {
                let lines = 1 + pairs.len().div_ceil(4);
                Constraint::Length((lines + 1) as u16)
            })
            .collect();
        constraints.push(Constraint::Min(0));
        let chunks = RLayout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .margin(0)
            .split(area);
        for (i, (category, pairs)) in shortcuts.iter().enumerate() {
            if i >= chunks.len().saturating_sub(1) {
                break;
            }
            let block = bordered_block(category);
            let rect = chunks[i];
            let block_inner = block.inner(rect);
            f.render_widget(block, rect);
            let w = block_inner.width as usize;
            let mut lines: Vec<Line> = Vec::new();
            let mut line_spans: Vec<Span> = Vec::new();
            let mut len = 0usize;
            for (k, a) in pairs {
                let group_len = 2 + k.len() + 2 + a.len() + 2;
                if len + group_len > w && !line_spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut line_spans)));
                    len = 0;
                }
                line_spans.push(Span::styled(format!("[{}] ", k), Theme::key()));
                line_spans.push(Span::styled(format!("{}  ", a), Theme::body()));
                len += group_len;
            }
            if !line_spans.is_empty() {
                lines.push(Line::from(line_spans));
            }
            let para = Paragraph::new(if lines.is_empty() {
                vec![Line::from("")]
            } else {
                lines
            });
            f.render_widget(para, block_inner);
        }
        let fill_rect = chunks.get(shortcuts.len());
        if let Some(&r) = fill_rect {
            if r.height > 0 {
                let fill_bg = Block::default().style(Style::default().bg(Theme::BG()));
                f.render_widget(fill_bg, r);
            }
        }
    }

    /// Floating dialog to choose P&L currency (fiat/crypto).
    pub fn render_currency_picker(
        f: &mut Frame,
        currencies: &[&str],
        selected: usize,
        filter: &str,
    ) {
        let area = f.area();
        let w = 36u16.min(area.width);
        let h = (currencies.len() + 4) as u16;
        let h = h.min(area.height).max(6);
        let x = area.x + area.width.saturating_sub(w) / 2;
        let y = area.y + area.height.saturating_sub(h) / 2;
        let rect = Rect {
            x,
            y,
            width: w,
            height: h,
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" P&L Currency ")
            .border_style(Theme::block_border())
            .title_style(Theme::block_title())
            .style(Style::default().bg(Theme::BG()));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("Filter: ", Theme::block_title()),
                Span::raw(filter),
                Span::styled("▌", Theme::body()),
            ]),
            Line::from(""),
        ];
        for (i, c) in currencies.iter().enumerate() {
            let mark = if i == selected { "► " } else { "  " };
            lines.push(Line::from(vec![
                Span::raw(mark),
                Span::styled(*c, Theme::body()),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("[Enter] ", Theme::key()),
            Span::raw("select  "),
            Span::styled("[Esc] ", Theme::key()),
            Span::raw("cancel"),
        ]));
        let para = Paragraph::new(lines).style(Theme::body());
        f.render_widget(para, inner);
    }

    /// Floating confirmation when switching to Live mode.
    pub fn render_live_confirm(f: &mut Frame, tab_index: usize) {
        let area = f.area();
        let tab_name = TABS.get(tab_index).copied().unwrap_or("?");
        let msg = format!("Switch {} to Live trading? (y/n)", tab_name);
        let bw = (msg.len() as u16 + 4).max(24).min(area.width);
        let bh = 4u16;
        let x = area.x + area.width.saturating_sub(bw) / 2;
        let y = area.y + area.height.saturating_sub(bh) / 2;
        let rect = Rect {
            x,
            y,
            width: bw,
            height: bh,
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Confirm ")
            .border_style(Theme::block_border())
            .title_style(Theme::block_title())
            .style(Style::default().bg(Theme::BG()));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let para = Paragraph::new(msg.as_str()).style(Theme::body());
        f.render_widget(para, inner);
    }

    /// Centered popup for Discover filter selection (Category / Period / Order). Enter apply, Esc cancel.
    pub fn render_discover_filter_dialog(
        f: &mut Frame,
        title: &str,
        options: &[&str],
        selected: usize,
    ) {
        let area = f.area();
        let sel = selected.min(options.len().saturating_sub(1));
        let content_w = options.iter().map(|s| s.len()).max().unwrap_or(8).max(28) as u16;
        let content_h = options.len() as u16;
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} — Enter apply, Esc cancel ", title))
            .border_style(Theme::block_border())
            .title_style(Theme::block_title())
            .style(Style::default().bg(Theme::BG()));
        let rect_w = content_w + 4;
        let rect_h = content_h + 2;
        let rect = Rect {
            x: area.x + area.width.saturating_sub(rect_w) / 2,
            y: area.y + area.height.saturating_sub(rect_h) / 2,
            width: rect_w,
            height: rect_h,
        };
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let lines: Vec<Line> = options
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let style = if i == sel {
                    Theme::tab_selected()
                } else {
                    Theme::body()
                };
                Line::from(Span::styled(*s, style))
            })
            .collect();
        let para = Paragraph::new(lines).style(Theme::body());
        f.render_widget(para, inner);
    }

    /// Full-screen theming view (replaces main UI when open).
    #[allow(clippy::too_many_arguments)]
    pub fn render_theme_screen(
        f: &mut Frame,
        theme_selection: usize,
        theme_names: &[String],
        current_theme_index: usize,
        in_creator: bool,
        creator_role: usize,
        creator_color_idx: usize,
        editor_palette: Option<&ThemePalette>,
    ) {
        Self::render_theme_overlay(
            f,
            theme_selection,
            theme_names,
            current_theme_index,
            in_creator,
            creator_role,
            creator_color_idx,
            editor_palette,
        );
    }

    /// Fullscreen overlay for theming (T to open/close).
    #[allow(clippy::too_many_arguments)]
    pub fn render_theme_overlay(
        f: &mut Frame,
        theme_selection: usize,
        theme_names: &[String],
        current_theme_index: usize,
        in_creator: bool,
        creator_role: usize,
        creator_color_idx: usize,
        editor_palette: Option<&ThemePalette>,
    ) {
        let area = f.area();
        let bg = Block::default().style(Style::default().bg(Theme::BG()));
        f.render_widget(bg, area);
        let inner = inner_area(area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Theming — T / Esc close ")
            .border_style(Theme::block_border())
            .title_style(Theme::block_title())
            .style(Style::default().bg(Theme::BG()));
        let block_inner = block.inner(inner);
        f.render_widget(block, inner);

        let mut lines: Vec<Line> = Vec::new();
        if in_creator {
            lines.push(Line::from(
                "Theme Creator — j/k role, h/l color, s save, Esc back",
            ));
            lines.push(Line::from(""));
            if let Some(p) = editor_palette {
                for (i, name) in THEME_CREATOR_ROLES.iter().enumerate() {
                    let mark = if i == creator_role { "► " } else { "  " };
                    let c = p.role_color(i);
                    let color_preview = format!(" [{:3},{:3},{:3}] ", c[0], c[1], c[2]);
                    lines.push(Line::from(format!("{}{}: {}", mark, name, color_preview)));
                }
                lines.push(Line::from(""));
                let ci = creator_color_idx.min(COLOR_PRESETS.len().saturating_sub(1));
                let cp = COLOR_PRESETS[ci];
                lines.push(Line::from(format!(
                    "Color preset {}/{}: {:?}  (h/l change)",
                    ci + 1,
                    COLOR_PRESETS.len(),
                    cp
                )));
            }
        } else {
            lines.push(Line::from(
                "↑↓/jk select  Enter apply  e Export  i Import  c Creator",
            ));
            lines.push(Line::from(""));
            for (i, name) in theme_names.iter().enumerate() {
                let mark = if i == theme_selection { "► " } else { "  " };
                let cur = if i == current_theme_index {
                    " (active)"
                } else {
                    ""
                };
                lines.push(Line::from(format!("{}{}{}", mark, name, cur)));
            }
            lines.push(Line::from(""));
            let export_sel = theme_selection == theme_names.len();
            let import_sel = theme_selection == theme_names.len() + 1;
            let creator_sel = theme_selection == theme_names.len() + 2;
            lines.push(Line::from(format!(
                "{}[e] Export to theme_export.toml",
                if export_sel { "► " } else { "  " }
            )));
            lines.push(Line::from(format!(
                "{}[i] Import from theme_import.toml",
                if import_sel { "► " } else { "  " }
            )));
            lines.push(Line::from(format!(
                "{}[c] Theme creator",
                if creator_sel { "► " } else { "  " }
            )));
        }
        let para = Paragraph::new(lines).style(Theme::body());
        f.render_widget(para, block_inner);
    }

    /// Add trader dialog: search box + list of profiles or pasted address. Enter add, Esc cancel.
    pub fn render_copy_add_dialog(
        f: &mut Frame,
        search_query: &str,
        option_rows: &[(String, bool)],
    ) {
        let area = f.area();
        let w = 56u16.min(area.width).max(40);
        let list_h = option_rows.len().min(12) as u16;
        let h = (4 + list_h).min(area.height).max(8);
        let x = area.x + area.width.saturating_sub(w) / 2;
        let y = area.y + area.height.saturating_sub(h) / 2;
        let rect = Rect {
            x,
            y,
            width: w,
            height: h,
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Add trader — ↑↓ select, Enter add, Esc cancel ")
            .border_style(Theme::block_border())
            .title_style(Theme::block_title())
            .style(Style::default().bg(Theme::BG()));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let chunks = RLayout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(2),
                Constraint::Length(1),
            ])
            .split(inner);
        let search_line = Line::from(vec![
            Span::styled("Search or paste address: ", Theme::block_title()),
            Span::raw(search_query),
            Span::styled("_", Theme::body()),
        ]);
        let search_para = Paragraph::new(search_line).style(Theme::body());
        f.render_widget(search_para, chunks[0]);
        let list_lines: Vec<Line> = option_rows
            .iter()
            .map(|(text, selected)| {
                let style = if *selected {
                    Theme::tab_selected()
                } else {
                    Theme::body()
                };
                let mark = if *selected { "► " } else { "  " };
                Line::from(Span::styled(format!("{}{}", mark, text), style))
            })
            .collect();
        let list_para = Paragraph::new(if list_lines.is_empty() {
            vec![Line::from(Span::styled(
                "Type a username to search, or paste 0x… address",
                Theme::dim(),
            ))]
        } else {
            list_lines
        });
        f.render_widget(list_para, chunks[1]);
        let hint = Line::from(vec![
            Span::styled("[Enter] ", Theme::key()),
            Span::raw("add  "),
            Span::styled("[Esc] ", Theme::key()),
            Span::raw("cancel"),
        ]);
        f.render_widget(Paragraph::new(hint).style(Theme::dim()), chunks[2]);
    }

    // ── Sports 3-pane render ─────────────────────────────────────────────────

    fn render_sports_3pane(f: &mut Frame, area: Rect, sv: &SportsView, scroll_offsets: &[usize]) {
        let scroll = scroll_offsets.first().copied().unwrap_or(0);

        // Horizontal split: 22% | 38% | 40%
        let cols = RLayout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(22),
                Constraint::Percentage(38),
                Constraint::Percentage(40),
            ])
            .split(area);

        // ── Left: Strategies pane ──────────────────────────────────────────
        let strat_title = if sv.pane == 0 {
            "► Strategies"
        } else {
            "  Strategies"
        };
        let strat_block = bordered_block(strat_title);
        let strat_inner = strat_block.inner(cols[0]);
        f.render_widget(strat_block, cols[0]);

        let strat_lines: Vec<Line> = sv
            .strategies
            .iter()
            .map(|s| {
                let prefix = if s.is_custom {
                    "[c]"
                } else if s.enabled {
                    "[x]"
                } else {
                    "[ ]"
                };
                let style = if s.selected && sv.pane == 0 {
                    Theme::tab_selected()
                } else if s.enabled {
                    Theme::success()
                } else {
                    Theme::body()
                };
                Line::from(Span::styled(format!("{} {}", prefix, s.name), style))
            })
            .collect();
        let hint = Line::from(vec![
            Span::styled("[Space]", Theme::key()),
            Span::raw(" toggle "),
            Span::styled("[e]", Theme::key()),
            Span::raw(" edit"),
        ]);
        let mut all_lines = strat_lines;
        if strat_inner.height > sv.strategies.len() as u16 + 1 {
            all_lines.push(Line::from(""));
            all_lines.push(hint);
        }
        let strat_para = Paragraph::new(all_lines).style(Theme::body());
        f.render_widget(strat_para, strat_inner);

        // ── Center: Signal Feed pane ───────────────────────────────────────
        let feed_title = if sv.pane == 1 {
            format!("► Signals ({} pending)", sv.pending_count)
        } else {
            format!("  Signals ({} pending)", sv.pending_count)
        };
        let feed_block = bordered_block(&feed_title);
        let feed_inner = feed_block.inner(cols[1]);
        f.render_widget(feed_block, cols[1]);

        let pending_count = sv.signals.iter().filter(|s| s.status == "pending").count();
        let done_idx = sv.signals.iter().position(|s| s.status != "pending");
        let mut feed_lines: Vec<Line> = Vec::new();

        for (i, sig) in sv.signals.iter().enumerate() {
            if done_idx == Some(i) {
                feed_lines.push(Line::from(Span::styled(
                    "─ executed ─────────────────",
                    Theme::dim(),
                )));
            }
            let style = if sig.selected && sv.pane == 1 {
                Theme::tab_selected()
            } else {
                match sig.status.as_str() {
                    "pending" => Theme::body(),
                    "auto" | "done" => Theme::success(),
                    "dismissed" => Theme::dim(),
                    _ => Theme::body(),
                }
            };
            let line = format!(
                "{:<12} {:3}  {:>3.0}%  {:>5.2}  [{}]",
                truncate_str(&sig.team, 12),
                sig.side,
                sig.edge_pct * 100.0,
                sig.kelly_size * 1000.0, // ×1000 bankroll for dollar display
                sig.status,
            );
            feed_lines.push(Line::from(Span::styled(line, style)));
        }

        if sv.signals.is_empty() {
            feed_lines.push(Line::from(Span::styled(
                "No signals yet. Enable a strategy.",
                Theme::dim(),
            )));
        }

        let visible_h = feed_inner.height as usize;
        let total = feed_lines.len();
        let offset = if sv.pane == 1 { scroll } else { 0 };
        let offset = offset.min(total.saturating_sub(visible_h));
        let visible: Vec<Line> = feed_lines
            .into_iter()
            .skip(offset)
            .take(visible_h)
            .collect();

        let mut hint_line = Vec::new();
        if pending_count > 0 {
            hint_line.push(Span::styled("[Enter]", Theme::key()));
            hint_line.push(Span::raw(" exec  "));
            hint_line.push(Span::styled("[d]", Theme::key()));
            hint_line.push(Span::raw(" dismiss"));
        }

        let mut all_feed = visible;
        if feed_inner.height > total as u16 + 1 && !hint_line.is_empty() {
            all_feed.push(Line::from(""));
            all_feed.push(Line::from(hint_line));
        }
        f.render_widget(Paragraph::new(all_feed).style(Theme::body()), feed_inner);

        // ── Right: Fixtures pane ───────────────────────────────────────────
        let league_label = sv.league_filter.as_deref().unwrap_or("All");
        let team_label = sv.team_filter.as_deref().unwrap_or("All");
        let fix_title = format!(
            "{}Fixtures  League: {} | Team: {}",
            if sv.pane == 2 { "► " } else { "  " },
            league_label,
            team_label
        );
        let fix_block = bordered_block(&fix_title);
        let fix_inner = fix_block.inner(cols[2]);
        f.render_widget(fix_block, cols[2]);

        let fix_visible_h = fix_inner.height as usize;
        let fix_offset = if sv.pane == 2 { scroll } else { 0 };
        let fix_offset = fix_offset.min(sv.fixtures.len().saturating_sub(fix_visible_h));

        let fix_lines: Vec<Line> = sv
            .fixtures
            .iter()
            .skip(fix_offset)
            .take(fix_visible_h)
            .map(|row| {
                if row.is_date_header {
                    return Line::from(Span::styled(row.date.clone(), Theme::block_title()));
                }
                let market = if row.has_market { " [mkt]" } else { "" };
                let style = if row.selected && sv.pane == 2 {
                    Theme::tab_selected()
                } else if row.has_market {
                    Theme::success()
                } else {
                    Theme::body()
                };
                Line::from(Span::styled(
                    format!(
                        "  {:>14} vs {:<14}{}",
                        truncate_str(&row.home, 14),
                        truncate_str(&row.away, 14),
                        market
                    ),
                    style,
                ))
            })
            .collect();

        let mut all_fix = if fix_lines.is_empty() {
            vec![Line::from(Span::styled("⏳ Loading…", Theme::dim()))]
        } else {
            fix_lines
        };
        if fix_inner.height > sv.fixtures.len() as u16 + 1 {
            all_fix.push(Line::from(""));
            all_fix.push(Line::from(vec![
                Span::styled("[L]", Theme::key()),
                Span::raw(" league  "),
                Span::styled("[T]", Theme::key()),
                Span::raw(" team  "),
                Span::styled("[r]", Theme::key()),
                Span::raw(" refresh"),
            ]));
        }
        f.render_widget(Paragraph::new(all_fix).style(Theme::body()), fix_inner);

        // ── Overlays ──────────────────────────────────────────────────────
        if sv.show_league_picker {
            Self::render_picker_overlay(
                f,
                " League Filter — Enter apply, Esc cancel ",
                &sv.league_picker_items,
                sv.league_picker_sel,
            );
        } else if sv.show_team_picker {
            Self::render_picker_overlay(
                f,
                " Team Filter — Enter apply, Esc cancel ",
                &sv.team_picker_items,
                sv.team_picker_sel,
            );
        }
    }

    fn render_picker_overlay(f: &mut Frame, title: &str, items: &[String], sel: usize) {
        let area = f.area();
        let content_w = items.iter().map(|s| s.len()).max().unwrap_or(16).max(24) as u16;
        let content_h = items.len().min(20) as u16;
        let rect_w = (content_w + 4).min(area.width);
        let rect_h = (content_h + 2).min(area.height);
        let rect = Rect {
            x: area.x + area.width.saturating_sub(rect_w) / 2,
            y: area.y + area.height.saturating_sub(rect_h) / 2,
            width: rect_w,
            height: rect_h,
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Theme::block_border())
            .title_style(Theme::block_title())
            .style(Style::default().bg(Theme::BG()));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let lines: Vec<Line> = items
            .iter()
            .enumerate()
            .take(inner.height as usize)
            .map(|(i, s)| {
                let style = if i == sel {
                    Theme::tab_selected()
                } else {
                    Theme::body()
                };
                Line::from(Span::styled(s.as_str(), style))
            })
            .collect();
        f.render_widget(Paragraph::new(lines).style(Theme::body()), inner);
    }

    pub fn render_bankroll_prompt(f: &mut Frame, input: &str) {
        let area = f.area();
        let w = 50u16.min(area.width);
        let h = 4u16;
        let x = area.x + area.width.saturating_sub(w) / 2;
        let y = area.y + area.height.saturating_sub(h) / 2;
        let rect = Rect {
            x,
            y,
            width: w,
            height: h,
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Set Paper Bankroll ")
            .border_style(Theme::block_border())
            .title_style(Theme::block_title())
            .style(Style::default().bg(Theme::BG()));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let line1 = Line::from(vec![
            Span::raw("Amount: "),
            Span::raw(input),
            Span::styled("▌", Theme::body()),
        ]);
        let line2 = Line::from(vec![
            Span::styled("[Enter] ", Theme::key()),
            Span::raw("confirm  "),
            Span::styled("[Esc] ", Theme::key()),
            Span::raw("cancel"),
        ]);
        let para = Paragraph::new(vec![line1, line2]).style(Theme::body());
        f.render_widget(para, inner);
    }
}
