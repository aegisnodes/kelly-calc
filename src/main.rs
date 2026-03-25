use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

// ── Color palette ──────────────────────────────────────────────────────────────
const C_BG: Color = Color::Rgb(8, 8, 16);
const C_PANEL: Color = Color::Rgb(14, 14, 28);
const C_PANEL2: Color = Color::Rgb(20, 18, 38);
const C_ACCENT: Color = Color::Rgb(100, 220, 255);
const C_ACCENT2: Color = Color::Rgb(180, 100, 255);
const C_GREEN: Color = Color::Rgb(80, 255, 160);
const C_YELLOW: Color = Color::Rgb(255, 210, 80);
const C_ORANGE: Color = Color::Rgb(255, 150, 60);
const C_RED: Color = Color::Rgb(255, 70, 90);
const C_DIM: Color = Color::Rgb(70, 70, 110);
const C_TEXT: Color = Color::Rgb(200, 200, 235);
const C_HIGHLIGHT: Color = Color::Rgb(30, 28, 58);
const C_SEPARATOR: Color = Color::Rgb(40, 40, 70);

// ── Sections ───────────────────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Debug)]
enum Section {
    Core,
    Crypto,
}

// ── Fields ─────────────────────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Debug)]
enum Field {
    // Core
    Balance,
    WinRate,
    WinLossRatio,
    FractionKelly,
    MaxRisk,
    // Crypto
    Leverage,
    TradeFees,
    MakerFee,
}

const FIELDS_CORE: [Field; 5] = [
    Field::Balance,
    Field::WinRate,
    Field::WinLossRatio,
    Field::FractionKelly,
    Field::MaxRisk,
];

const FIELDS_CRYPTO: [Field; 3] = [
    Field::Leverage,
    Field::TradeFees,
    Field::MakerFee,
];

// ── App state ──────────────────────────────────────────────────────────────────
struct App {
    balance: String,
    win_rate: String,
    win_loss_ratio: String,
    fraction_kelly: String,
    max_risk: String,
    leverage: String,
    trade_fees: String,
    maker_fees: String,
    // UI state
    active_section: Section,
    selected: usize,
    show_help: bool,
    show_scenario: bool,
}

impl App {
    fn new() -> Self {
        Self {
            balance: String::from("10000"),
            win_rate: String::from("55"),
            win_loss_ratio: String::from("1.5"),
            fraction_kelly: String::from("50"),
            max_risk: String::from("5"),
            leverage: String::from("20"),
            trade_fees: String::from("0.06"),
            maker_fees: String::from("0.02"),
            active_section: Section::Core,
            selected: 0,
            show_help: false,
            show_scenario: false,
        }
    }

    fn current_fields(&self) -> &[Field] {
        match self.active_section {
            Section::Core => &FIELDS_CORE,
            Section::Crypto => &FIELDS_CRYPTO,
        }
    }

    fn field_buf_mut(&mut self, f: Field) -> &mut String {
        match f {
            Field::Balance => &mut self.balance,
            Field::WinRate => &mut self.win_rate,
            Field::WinLossRatio => &mut self.win_loss_ratio,
            Field::FractionKelly => &mut self.fraction_kelly,
            Field::MaxRisk => &mut self.max_risk,
            Field::Leverage => &mut self.leverage,
            Field::TradeFees => &mut self.trade_fees,
            Field::MakerFee => &mut self.maker_fees,
        }
    }

    fn field_buf(&self, f: Field) -> &str {
        match f {
            Field::Balance => &self.balance,
            Field::WinRate => &self.win_rate,
            Field::WinLossRatio => &self.win_loss_ratio,
            Field::FractionKelly => &self.fraction_kelly,
            Field::MaxRisk => &self.max_risk,
            Field::Leverage => &self.leverage,
            Field::TradeFees => &self.trade_fees,
            Field::MakerFee => &self.maker_fees,
        }
    }

    fn current_field(&self) -> Field {
        self.current_fields()[self.selected]
    }

    fn parse_f64(&self, f: Field) -> Option<f64> {
        self.field_buf(f).parse::<f64>().ok()
    }

    fn calc(&self) -> Option<CalcResult> {
        let balance = self.parse_f64(Field::Balance)?;
        let win_rate_pct = self.parse_f64(Field::WinRate)?;
        let b = self.parse_f64(Field::WinLossRatio)?;
        let fraction_pct = self.parse_f64(Field::FractionKelly)?;
        let max_risk_pct = self.parse_f64(Field::MaxRisk)?;
        let leverage = self.parse_f64(Field::Leverage).unwrap_or(1.0);
        let trade_fees_pct = self.parse_f64(Field::TradeFees).unwrap_or(0.04);
        let maker_fees_pct = self.parse_f64(Field::MakerFee).unwrap_or(0.02);

        if !(0.0..=100.0).contains(&win_rate_pct) { return None; }
        if b <= 0.0 || balance <= 0.0 { return None; }
        if !(0.0..=200.0).contains(&fraction_pct) { return None; }
        if leverage < 1.0 { return None; }

        let p = win_rate_pct / 100.0;
        let q = 1.0 - p;

        let full_kelly = (b * p - q) / b;
        let frac = fraction_pct / 100.0;
        let kelly_frac = full_kelly * frac;
        let max_risk_f = max_risk_pct / 100.0;
        let kelly_applied = kelly_frac.min(max_risk_f).max(0.0);

        let position_size = balance * kelly_applied;
        let edge = b * p - q;

        let notional_value = position_size * leverage;
        let margin_used = position_size;
        let effective_risk_pct = kelly_applied * leverage;

        // Removed inputs (Entry Price, Stop Loss %, Funding Rate) → hardcoded defaults
        let entry_price = 65000.0;
        let stop_loss_pct = 2.0;
        let funding_rate = 0.01;

        // Use Maker Fees for both open and close (limit orders) — this is what you asked for
        let fees_open = notional_value * (maker_fees_pct / 100.0);
        let fees_close = notional_value * (maker_fees_pct / 100.0);
        let funding_cost = 0.0; // funding rate input was removed
        let total_fees = fees_open + fees_close + funding_cost;

        let gross_win = notional_value * (b * (stop_loss_pct / 100.0));
        let gross_loss = notional_value * (stop_loss_pct / 100.0);
        let net_win = gross_win - total_fees;
        let net_loss = gross_loss + total_fees;
        let expected_pnl = p * net_win - q * net_loss;
        let expected_pnl_pct = (expected_pnl / balance) * 100.0;

        let maintenance_margin = 0.005;
        let liq_price_long = if entry_price > 0.0 {
            entry_price * (1.0 - 1.0 / leverage + maintenance_margin)
        } else { 0.0 };
        let liq_price_short = if entry_price > 0.0 {
            entry_price * (1.0 + 1.0 / leverage - maintenance_margin)
        } else { 0.0 };
        let liq_distance_pct = if entry_price > 0.0 {
            ((entry_price - liq_price_long).abs() / entry_price) * 100.0
        } else { 0.0 };

        let stop_price_long = if entry_price > 0.0 {
            entry_price * (1.0 - stop_loss_pct / 100.0)
        } else { 0.0 };
        let stop_price_short = if entry_price > 0.0 {
            entry_price * (1.0 + stop_loss_pct / 100.0)
        } else { 0.0 };

        let consecutive_losses_5pct = (0.05_f64.ln() / (1.0 - kelly_applied).max(0.001).ln()).ceil() as u32;
        let max_drawdown_5pct = 1.0 - (1.0 - kelly_applied).powi(consecutive_losses_5pct as i32);
        let variance_per_trade = p * (1.0 - p) * (b + 1.0).powi(2) * kelly_applied.powi(2);
        let sharpe_approx = if variance_per_trade > 0.0 {
            expected_pnl_pct / (variance_per_trade.sqrt() * 100.0 * leverage)
        } else { 0.0 };

        let ruin_approx = if edge > 0.0 {
            let ruin_per_unit = ((1.0 - edge) / (1.0 + edge)).powf(1.0 / kelly_applied.max(0.001));
            ruin_per_unit.min(1.0)
        } else { 1.0 };

        let scenarios = [5u32, 10, 20, 50].map(|n| {
            let wins = (n as f64 * p).round() as u32;
            let losses = n - wins;
            let result = balance * (1.0 + kelly_applied * b).powi(wins as i32)
                * (1.0 - kelly_applied).powi(losses as i32);
            (n, result)
        });

        Some(CalcResult {
            balance,
            full_kelly_pct: full_kelly * 100.0,
            frac_kelly_pct: kelly_frac * 100.0,
            applied_pct: kelly_applied * 100.0,
            position_size,
            notional_value,
            margin_used,
            effective_risk_pct: effective_risk_pct * 100.0,
            net_win,
            net_loss,
            expected_pnl,
            expected_pnl_pct,
            total_fees,
            fees_open,
            fees_close,
            funding_cost,
            liq_price_long,
            liq_price_short,
            liq_distance_pct,
            stop_price_long,
            stop_price_short,
            stop_loss_pct,
            leverage,
            edge,
            sharpe_approx,
            ruin_approx,
            max_drawdown_5pct: max_drawdown_5pct * 100.0,
            consecutive_losses_5pct,
            scenarios,
            is_negative_ev: edge <= 0.0,
            fees_eat_edge: total_fees > net_win.max(0.0),
            trade_fees_pct,
            maker_fees_pct,
        })
    }
}

struct CalcResult {
    balance: f64,
    full_kelly_pct: f64,
    frac_kelly_pct: f64,
    applied_pct: f64,
    position_size: f64,
    notional_value: f64,
    margin_used: f64,
    effective_risk_pct: f64,
    net_win: f64,
    net_loss: f64,
    expected_pnl: f64,
    expected_pnl_pct: f64,
    total_fees: f64,
    fees_open: f64,
    fees_close: f64,
    funding_cost: f64,
    liq_price_long: f64,
    liq_price_short: f64,
    liq_distance_pct: f64,
    stop_price_long: f64,
    stop_price_short: f64,
    stop_loss_pct: f64,
    leverage: f64,
    edge: f64,
    sharpe_approx: f64,
    ruin_approx: f64,
    max_drawdown_5pct: f64,
    consecutive_losses_5pct: u32,
    scenarios: [(u32, f64); 4],
    is_negative_ev: bool,
    fees_eat_edge: bool,
    trade_fees_pct: f64,
    maker_fees_pct: f64,
}

// ── Main ───────────────────────────────────────────────────────────────────────
fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('q')
                || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                break;
            }

            if app.show_help || app.show_scenario {
                app.show_help = false;
                app.show_scenario = false;
                continue;
            }

            if key.code == KeyCode::Char('?') {
                app.show_help = true;
                continue;
            }
            if key.code == KeyCode::Char('s') {
                app.show_scenario = true;
                continue;
            }

            if key.code == KeyCode::F(1) {
                app.active_section = Section::Core;
                app.selected = 0;
                continue;
            }
            if key.code == KeyCode::F(2) {
                app.active_section = Section::Crypto;
                app.selected = 0;
                continue;
            }

            match key.code {
                KeyCode::Tab | KeyCode::Down => {
                    let len = app.current_fields().len();
                    app.selected = (app.selected + 1) % len;
                }
                KeyCode::BackTab | KeyCode::Up => {
                    let len = app.current_fields().len();
                    app.selected = (app.selected + len - 1) % len;
                }
                KeyCode::Backspace => {
                    let f = app.current_field();
                    app.field_buf_mut(f).pop();
                }
                KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                    let f = app.current_field();
                    let buf = app.field_buf_mut(f);
                    if c == '.' && buf.contains('.') { continue; }
                    if c == '-' && !buf.is_empty() { continue; }
                    buf.push(c);
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

// ── UI ─────────────────────────────────────────────────────────────────────────
fn ui(f: &mut Frame, app: &App) {
    let size = f.area();
    f.render_widget(Block::default().style(Style::default().bg(C_BG)), size);

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(size);

    render_title(f, outer[0]);
    render_section_bar(f, app, outer[1]);
    render_body(f, app, outer[2]);
    render_footer(f, outer[3]);

    if app.show_help { render_help(f, size); }
    if app.show_scenario { render_scenario(f, app, size); }
}

fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled("◈ ", Style::default().fg(C_ACCENT2)),
        Span::styled("KELLY CRITERION", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("  ×  ", Style::default().fg(C_DIM)),
        Span::styled("BTCUSDT Position Sizing", Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD)),
        Span::styled("  |  ", Style::default().fg(C_DIM)),
        Span::styled("Leverage · Fees · Drawdown", Style::default().fg(C_DIM)),
    ]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(C_SEPARATOR))
            .style(Style::default().bg(C_BG)),
    );
    f.render_widget(title, area);
}

fn render_section_bar(f: &mut Frame, app: &App, area: Rect) {
    let (core_style, crypto_style) = match app.active_section {
        Section::Core => (
            Style::default().fg(C_ACCENT).bg(C_PANEL2).add_modifier(Modifier::BOLD),
            Style::default().fg(C_DIM).bg(C_BG),
        ),
        Section::Crypto => (
            Style::default().fg(C_DIM).bg(C_BG),
            Style::default().fg(C_ACCENT2).bg(C_PANEL2).add_modifier(Modifier::BOLD),
        ),
    };

    let line = Line::from(vec![
        Span::styled("  [F1] Core Kelly  ", core_style),
        Span::styled("│", Style::default().fg(C_SEPARATOR)),
        Span::styled("  [F2] Crypto / Futures (BTCUSDT)  ", crypto_style),
    ]);

    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(C_BG)),
        area,
    );
}

fn render_body(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);
    render_inputs(f, app, cols[0]);
    render_results(f, app, cols[1]);
}

fn render_inputs(f: &mut Frame, app: &App, area: Rect) {
    let title_color = match app.active_section { Section::Core => C_ACCENT, Section::Crypto => C_ACCENT2 };
    let block = Block::default()
        .title(Span::styled(" Inputs ", Style::default().fg(title_color).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_DIM))
        .style(Style::default().bg(C_PANEL));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let fields_info: &[(Field, &str, &str, &str)] = match app.active_section {
        Section::Core => &[
            (Field::Balance,       "Balance",        "$", "Total available capital"),
            (Field::WinRate,       "Win Rate",       "%", "% winning trades (0-100)"),
            (Field::WinLossRatio,  "Win/Loss Ratio", "x", "Avg gain / avg loss ratio"),
            (Field::FractionKelly, "Kelly Fraction", "%", "100=Full · 50=Half (recommended)"),
            (Field::MaxRisk,       "Max Risk/Trade", "%", "Hard capital cap per trade"),
        ],
        Section::Crypto => &[
            (Field::Leverage,  "Leverage",   "x", "Contract leverage (1=spot, 10=10x)"),
            (Field::TradeFees, "Taker Fees", "%", "Market order fees (for reference)"),
            (Field::MakerFee,  "Maker Fees", "%", "Limit order fees — used in Kelly EV calc"),
        ],
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), Constraint::Length(3), Constraint::Length(3),
            Constraint::Length(3), Constraint::Length(3), Constraint::Min(0),
        ])
        .split(inner);

    for (i, (field, label, unit, hint)) in fields_info.iter().enumerate() {
        if i >= rows.len() { break; }
        let is_sel = app.selected == i;
        let buf = app.field_buf(*field);
        let valid = app.parse_f64(*field).is_some();

        let value_color = if !valid && !buf.is_empty() { C_RED }
            else if is_sel { C_GREEN }
            else { C_TEXT };

        let bg = if is_sel { C_HIGHLIGHT } else { C_PANEL };
        f.render_widget(Block::default().style(Style::default().bg(bg)), rows[i]);

        let label_area = Rect { x: rows[i].x+1, y: rows[i].y,   width: rows[i].width.saturating_sub(2), height: 1 };
        let value_area = Rect { x: rows[i].x+1, y: rows[i].y+1, width: rows[i].width.saturating_sub(2), height: 1 };

        let prefix = if is_sel { "▶ " } else { "  " };
        let label_color = if is_sel {
            match app.active_section { Section::Core => C_ACCENT, Section::Crypto => C_ACCENT2 }
        } else { C_TEXT };

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(label_color)),
                Span::styled(*label, Style::default().fg(label_color).add_modifier(if is_sel { Modifier::BOLD } else { Modifier::empty() })),
                Span::styled(format!(" ({})", hint), Style::default().fg(C_DIM)),
            ])),
            label_area,
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(*unit, Style::default().fg(C_DIM)),
                Span::styled(" ", Style::default()),
                Span::styled(buf, Style::default().fg(value_color).add_modifier(if is_sel { Modifier::BOLD } else { Modifier::empty() })),
                Span::styled(if is_sel { "█" } else { "" }, Style::default().fg(C_ACCENT)),
            ])),
            value_area,
        );
    }
}

fn render_results(f: &mut Frame, app: &App, area: Rect) {
    match app.calc() {
        None => {
            let block = Block::default()
                .title(Span::styled(" Results ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL).border_style(Style::default().fg(C_DIM))
                .style(Style::default().bg(C_PANEL));
            let inner = block.inner(area);
            f.render_widget(block, area);
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled("  ⚠  Invalid value in one or more fields.", Style::default().fg(C_YELLOW))),
                ]).style(Style::default().bg(C_PANEL)),
                inner,
            );
        }
        Some(r) => {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);
            let top_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(rows[0]);
            let bot_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(rows[1]);

            render_panel_kelly(f, &r, top_cols[0]);
            render_panel_position(f, &r, top_cols[1]);
            render_panel_risk(f, &r, bot_cols[0]);
            render_panel_fees_liq(f, &r, bot_cols[1]);
        }
    }
}

fn render_panel_kelly(f: &mut Frame, r: &CalcResult, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Kelly ", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL).border_style(Style::default().fg(C_DIM))
        .style(Style::default().bg(C_PANEL));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let bar_w = inner.width.saturating_sub(4) as usize;
    let filled = ((r.applied_pct / 25.0).min(1.0) * bar_w as f64) as usize;
    let bar_color = if r.applied_pct < 3.0 { C_GREEN } else if r.applied_pct < 8.0 { C_YELLOW } else { C_RED };
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_w.saturating_sub(filled));

    let neg_ev_line = if r.is_negative_ev {
        Line::from(Span::styled("  ✖ NEGATIVE EV — NO TRADE", Style::default().fg(C_RED).add_modifier(Modifier::BOLD)))
    } else if r.fees_eat_edge {
        Line::from(Span::styled("  ⚠ Fees > Edge!", Style::default().fg(C_ORANGE).add_modifier(Modifier::BOLD)))
    } else {
        Line::from(Span::styled("  ✔ Positive EV", Style::default().fg(C_GREEN)))
    };

    let lines = vec![
        Line::from(""),
        neg_ev_line,
        Line::from(""),
        rrow("  Full Kelly".to_string(),  fmt_pct(r.full_kelly_pct), C_DIM),
        rrow("  Frac Kelly".to_string(),  fmt_pct(r.frac_kelly_pct), C_ACCENT),
        rrow("  Applied".to_string(),     fmt_pct(r.applied_pct),    bar_color),
        rrow("  Edge".to_string(),        fmt_pct(r.edge * 100.0),   if r.edge > 0.0 { C_GREEN } else { C_RED }),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(&bar, Style::default().fg(bar_color)),
        ]),
        Line::from(vec![
            Span::styled("  Risk: ", Style::default().fg(C_DIM)),
            Span::styled(fmt_pct(r.applied_pct), Style::default().fg(bar_color).add_modifier(Modifier::BOLD)),
            Span::styled(" of capital", Style::default().fg(C_DIM)),
        ]),
    ];

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(C_PANEL)), inner);
}

fn render_panel_position(f: &mut Frame, r: &CalcResult, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Position Sizing ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL).border_style(Style::default().fg(C_DIM))
        .style(Style::default().bg(C_PANEL));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lev_color = if r.leverage <= 3.0 { C_GREEN } else if r.leverage <= 10.0 { C_YELLOW } else { C_RED };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("💰 KELLY RECOMMENDED POSITION", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
        ]),
        rrow_money("  Margin to Allocate", r.margin_used, C_GREEN),
        rrow_money("  Notional Exposure", r.notional_value, C_ACCENT),
        Line::from(""),
        rrow_money("  Balance",     r.balance,        C_TEXT),
        Line::from(""),
        rrow("  Leverage".to_string(),   format!("{:.1}x", r.leverage),         lev_color),
        rrow("  Eff. Risk".to_string(),  fmt_pct(r.effective_risk_pct),          lev_color),
        Line::from(""),
        rrow_money("  Net Win",    r.net_win,        C_GREEN),
        rrow_money("  Net Loss",   -r.net_loss,      C_RED),
        rrow_money("  Exp. PnL",   r.expected_pnl,   if r.expected_pnl >= 0.0 { C_GREEN } else { C_RED }),
        rrow("  Exp. PnL%".to_string(),  fmt_pct(r.expected_pnl_pct), if r.expected_pnl_pct >= 0.0 { C_GREEN } else { C_RED }),
    ];

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(C_PANEL)), inner);
}

fn render_panel_risk(f: &mut Frame, r: &CalcResult, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Risk / Stats ", Style::default().fg(C_YELLOW).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL).border_style(Style::default().fg(C_DIM))
        .style(Style::default().bg(C_PANEL));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let sharpe_color = if r.sharpe_approx > 1.0 { C_GREEN } else if r.sharpe_approx > 0.0 { C_YELLOW } else { C_RED };
    let ruin_color = if r.ruin_approx < 0.05 { C_GREEN } else if r.ruin_approx < 0.20 { C_YELLOW } else { C_RED };

    let mut lines = vec![
        Line::from(""),
        rrow("  Sharpe~".to_string(),   format!("{:.3}", r.sharpe_approx),                sharpe_color),
        rrow("  Ruin Risk".to_string(), fmt_pct(r.ruin_approx * 100.0),                   ruin_color),
        rrow("  Max DD~".to_string(),   fmt_pct(r.max_drawdown_5pct),                      C_ORANGE),
        rrow("  Losses→5%".to_string(), format!("{} trades", r.consecutive_losses_5pct),  C_DIM),
        Line::from(""),
        Line::from(Span::styled("  ─ Scenarios (expected trades) ─", Style::default().fg(C_DIM))),
        Line::from(""),
    ];

    for (n, result) in &r.scenarios {
        let color = if *result >= r.balance { C_GREEN } else { C_RED };
        lines.push(rrow(format!("  {} trades", n), format!("${:.2}", result), color));
    }

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(C_PANEL)), inner);
}

fn render_panel_fees_liq(f: &mut Frame, r: &CalcResult, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Fees · Liquidation ", Style::default().fg(C_ACCENT2).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL).border_style(Style::default().fg(C_DIM))
        .style(Style::default().bg(C_PANEL));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let liq_color = if r.liq_distance_pct > 20.0 { C_GREEN } else if r.liq_distance_pct > 8.0 { C_YELLOW } else { C_RED };
    let sl_safe = r.stop_loss_pct < r.liq_distance_pct;

    let sl_line = if r.liq_price_long > 0.0 {
        if sl_safe {
            Line::from(Span::styled("  ✔ SL before liquidation", Style::default().fg(C_GREEN)))
        } else {
            Line::from(Span::styled("  ✖ SL > Liq. Distance!", Style::default().fg(C_RED).add_modifier(Modifier::BOLD)))
        }
    } else {
        Line::from(Span::styled("  (default entry $65k)", Style::default().fg(C_DIM)))
    };

    let lines = vec![
        Line::from(""),
        rrow_money("  Fees Open",  r.fees_open,    C_DIM),
        rrow_money("  Fees Close", r.fees_close,   C_DIM),
        rrow_money("  Funding",    r.funding_cost, C_DIM),
        rrow_money("  Total Fees", r.total_fees,   C_ORANGE),
        Line::from(""),
        rrow("  Maker Fee % (used)".to_string(), fmt_pct(r.maker_fees_pct), C_GREEN),
        rrow("  Taker Fee %".to_string(),        fmt_pct(r.trade_fees_pct), C_DIM),
        Line::from(""),
        rrow("  Liq Dist.".to_string(),  fmt_pct(r.liq_distance_pct),   liq_color),
        rrow_price("  Liq Long",         r.liq_price_long,               C_RED),
        rrow_price("  Liq Short",        r.liq_price_short,              C_RED),
        rrow_price("  SL Long",          r.stop_price_long,              C_YELLOW),
        rrow_price("  SL Short",         r.stop_price_short,             C_YELLOW),
        Line::from(""),
        sl_line,
    ];

    f.render_widget(Paragraph::new(lines).style(Style::default().bg(C_PANEL)), inner);
}

// ── Helper row renderers ───────────────────────────────────────────────────────

fn rrow(label: String, value: String, color: Color) -> Line<'static> {
    let pad = 22usize.saturating_sub(label.len());
    let padded = format!("{:>width$}", value, width = pad);
    Line::from(vec![
        Span::styled(label, Style::default().fg(C_DIM)),
        Span::styled(padded, Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ])
}

fn rrow_money(label: &'static str, value: f64, color: Color) -> Line<'static> {
    rrow(label.to_string(), format!("${:.2}", value), color)
}

fn rrow_price(label: &'static str, value: f64, color: Color) -> Line<'static> {
    if value > 0.0 {
        rrow(label.to_string(), format!("${:.1}", value), color)
    } else {
        rrow(label.to_string(), String::from("—"), C_DIM)
    }
}

fn fmt_pct(v: f64) -> String { format!("{:.2}%", v) }

fn render_footer(f: &mut Frame, area: Rect) {
    let text = Line::from(vec![
        Span::styled(" F1/F2 ", Style::default().fg(C_ACCENT)),
        Span::styled("section  ", Style::default().fg(C_DIM)),
        Span::styled(" ↑↓/Tab ", Style::default().fg(C_ACCENT)),
        Span::styled("navigate  ", Style::default().fg(C_DIM)),
        Span::styled(" 0-9/. ", Style::default().fg(C_ACCENT)),
        Span::styled("edit  ", Style::default().fg(C_DIM)),
        Span::styled(" s ", Style::default().fg(C_ACCENT2)),
        Span::styled("scenarios  ", Style::default().fg(C_DIM)),
        Span::styled(" ? ", Style::default().fg(C_ACCENT)),
        Span::styled("help  ", Style::default().fg(C_DIM)),
        Span::styled(" q ", Style::default().fg(C_RED)),
        Span::styled("quit", Style::default().fg(C_DIM)),
    ]);
    f.render_widget(
        Paragraph::new(text).alignment(Alignment::Center)
            .style(Style::default().bg(C_BG).fg(C_DIM)),
        area,
    );
}

fn render_scenario(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(70, 75, area);
    f.render_widget(Clear, popup_area);

    let r = match app.calc() {
        Some(r) => r,
        None => return,
    };

    let balance = r.balance;
    let applied = r.applied_pct / 100.0;
    let b_ratio = app.parse_f64(Field::WinLossRatio).unwrap_or(1.5);
    let p = app.parse_f64(Field::WinRate).unwrap_or(55.0) / 100.0;

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Projected BTCUSDT balance after N trades (expected outcome)",
            Style::default().fg(C_TEXT),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  N Trades", Style::default().fg(C_DIM)),
            Span::styled("      Bear (-1σ)     ", Style::default().fg(C_RED)),
            Span::styled("  Base (exp.)  ", Style::default().fg(C_YELLOW)),
            Span::styled("  Bull (+1σ)", Style::default().fg(C_GREEN)),
        ]),
        Line::from(Span::styled(
            "  ──────────────────────────────────────────────────",
            Style::default().fg(C_DIM),
        )),
    ];

    for n in [1u32, 5, 10, 20, 50, 100].iter() {
        let n_wins_base = (*n as f64 * p).round() as u32;
        let n_wins_bear = ((*n as f64 * p) - (*n as f64 * p * (1.0-p)).sqrt()).max(0.0) as u32;
        let n_wins_bull = ((*n as f64 * p) + (*n as f64 * p * (1.0-p)).sqrt()).min(*n as f64) as u32;

        let calc_balance = |wins: u32| -> f64 {
            let losses = n - wins.min(*n);
            balance * (1.0 + applied * b_ratio).powi(wins as i32)
                * (1.0 - applied).powi(losses as i32)
        };

        let bear = calc_balance(n_wins_bear);
        let base = calc_balance(n_wins_base);
        let bull = calc_balance(n_wins_bull);

        let bear_color = if bear >= balance { C_GREEN } else { C_RED };
        let base_color = if base >= balance { C_GREEN } else { C_YELLOW };

        lines.push(Line::from(vec![
            Span::styled(format!("  {:>3} trades  ", n), Style::default().fg(C_DIM)),
            Span::styled(format!("  ${:>12.2}", bear), Style::default().fg(bear_color).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ${:>12.2}", base), Style::default().fg(base_color).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ${:>12.2}", bull), Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Bear = -1σ wins  |  Base = expected  |  Bull = +1σ wins",
        Style::default().fg(C_DIM),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  Press any key to close", Style::default().fg(C_DIM))));

    let popup = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Span::styled(
                    " BTCUSDT Trading Scenarios ",
                    Style::default().fg(C_ACCENT2).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_ACCENT2))
                .style(Style::default().bg(C_PANEL)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(popup, popup_area);
}

fn render_help(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(62, 85, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Kelly Criterion — BTCUSDT Futures Sizing", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled("  f* = (b·p - q) / b", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  p=winrate  q=1-p  b=win/loss ratio", Style::default().fg(C_DIM))),
        Line::from(""),
        Line::from(Span::styled("  ─── Section F1: Core ───────────────────────", Style::default().fg(C_DIM))),
        Line::from(Span::styled("  Balance      Total account capital.", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  Win Rate     % of trades that close in profit.", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  Win/Loss     Average gain / average loss ratio.", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  Kelly Frac   50=Half-Kelly (recommended).", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  Max Risk     Hard cap per trade (safety net).", Style::default().fg(C_TEXT))),
        Line::from(""),
        Line::from(Span::styled("  ─── Section F2: Crypto / Futures ───────────", Style::default().fg(C_DIM))),
        Line::from(Span::styled("  Leverage     Contract leverage multiplier.", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  Taker Fees   Market order fees (for reference).", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  Maker Fees   Limit order fees — used in EV calc.", Style::default().fg(C_TEXT))),
        Line::from(""),
        Line::from(Span::styled("  ─── Navigation ────────────────────────────", Style::default().fg(C_DIM))),
        Line::from(Span::styled("  F1 / F2      Switch section (no digit conflict)", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  ↑ ↓ / Tab    Move between fields", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  0-9 / .      Type values directly", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  Backspace    Delete last character", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  s            Toggle scenario projection", Style::default().fg(C_TEXT))),
        Line::from(""),
        Line::from(Span::styled("  Press any key to close", Style::default().fg(C_DIM))),
    ];

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default()
                .title(Span::styled(" Help ", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_ACCENT))
                .style(Style::default().bg(C_PANEL)))
            .wrap(Wrap { trim: false }),
        popup_area,
    );
}

fn centered_rect(px: u16, py: u16, r: Rect) -> Rect {
    let v = Layout::default().direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100-py)/2),
            Constraint::Percentage(py),
            Constraint::Percentage((100-py)/2),
        ])
        .split(r);
    Layout::default().direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100-px)/2),
            Constraint::Percentage(px),
            Constraint::Percentage((100-px)/2),
        ])
        .split(v[1])[1]
}