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
const C_BG: Color = Color::Rgb(10, 10, 20);
const C_PANEL: Color = Color::Rgb(18, 18, 35);
const C_ACCENT: Color = Color::Rgb(80, 200, 255);
const C_GREEN: Color = Color::Rgb(80, 255, 160);
const C_YELLOW: Color = Color::Rgb(255, 210, 80);
const C_RED: Color = Color::Rgb(255, 80, 100);
const C_DIM: Color = Color::Rgb(80, 80, 120);
const C_TEXT: Color = Color::Rgb(210, 210, 240);
const C_HIGHLIGHT: Color = Color::Rgb(40, 40, 70);

// ── Fields ─────────────────────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq)]
enum Field {
    Balance,
    WinRate,
    WinLossRatio,
    FractionKelly,
    MaxRisk,
}

const FIELDS: [Field; 5] = [
    Field::Balance,
    Field::WinRate,
    Field::WinLossRatio,
    Field::FractionKelly,
    Field::MaxRisk,
];

// ── App state ──────────────────────────────────────────────────────────────────
struct App {
    // raw string buffers for each field
    balance: String,
    win_rate: String,
    win_loss_ratio: String,
    fraction_kelly: String,
    max_risk: String,
    // which field is selected
    selected: usize,
    // show help popup
    show_help: bool,
}

impl App {
    fn new() -> Self {
        Self {
            balance: String::from("10000"),
            win_rate: String::from("55"),
            win_loss_ratio: String::from("1.5"),
            fraction_kelly: String::from("50"),
            max_risk: String::from("5"),
            selected: 0,
            show_help: false,
        }
    }

    fn field_buf_mut(&mut self, f: Field) -> &mut String {
        match f {
            Field::Balance => &mut self.balance,
            Field::WinRate => &mut self.win_rate,
            Field::WinLossRatio => &mut self.win_loss_ratio,
            Field::FractionKelly => &mut self.fraction_kelly,
            Field::MaxRisk => &mut self.max_risk,
        }
    }

    fn field_buf(&self, f: Field) -> &str {
        match f {
            Field::Balance => &self.balance,
            Field::WinRate => &self.win_rate,
            Field::WinLossRatio => &self.win_loss_ratio,
            Field::FractionKelly => &self.fraction_kelly,
            Field::MaxRisk => &self.max_risk,
        }
    }

    fn current_field(&self) -> Field {
        FIELDS[self.selected]
    }

    // ── Parse helpers ────────────────────────────────────────────────────────
    fn parse_f64(&self, f: Field) -> Option<f64> {
        self.field_buf(f).parse::<f64>().ok()
    }

    // ── Kelly calculation ────────────────────────────────────────────────────
    fn calc(&self) -> Option<KellyResult> {
        let balance = self.parse_f64(Field::Balance)?;
        let win_rate_pct = self.parse_f64(Field::WinRate)?;
        let b = self.parse_f64(Field::WinLossRatio)?;
        let fraction_pct = self.parse_f64(Field::FractionKelly)?;
        let max_risk_pct = self.parse_f64(Field::MaxRisk)?;

        if !(0.0..=100.0).contains(&win_rate_pct) { return None; }
        if b <= 0.0 { return None; }
        if balance <= 0.0 { return None; }
        if !(0.0..=200.0).contains(&fraction_pct) { return None; }
        if !(0.0..=100.0).contains(&max_risk_pct) { return None; }

        let p = win_rate_pct / 100.0;
        let q = 1.0 - p;

        // Full Kelly: f* = (bp - q) / b
        let full_kelly = (b * p - q) / b;

        // Fractional Kelly
        let frac = fraction_pct / 100.0;
        let kelly_frac = full_kelly * frac;

        // Cap at max risk
        let max_risk_f = max_risk_pct / 100.0;
        let kelly_applied = kelly_frac.min(max_risk_f).max(0.0);

        let position_size = balance * kelly_applied;
        let max_loss = position_size; // assuming full position at risk
        let expected_gain = position_size * b * p - position_size * q;
        let edge = b * p - q;
        let ruin_approx = if edge > 0.0 {
            // Simplified ruin probability for reference
            let ruin = ((1.0 - edge) / (1.0 + edge)).powf(balance / position_size.max(1.0));
            ruin
        } else {
            1.0
        };

        Some(KellyResult {
            balance,
            full_kelly_pct: full_kelly * 100.0,
            frac_kelly_pct: kelly_frac * 100.0,
            applied_pct: kelly_applied * 100.0,
            position_size,
            max_loss,
            expected_gain,
            edge,
            ruin_approx,
            is_negative_ev: edge <= 0.0,
        })
    }
}

struct KellyResult {
    balance: f64,
    full_kelly_pct: f64,
    frac_kelly_pct: f64,
    applied_pct: f64,
    position_size: f64,
    max_loss: f64,
    expected_gain: f64,
    edge: f64,
    ruin_approx: f64,
    is_negative_ev: bool,
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
            // Global quit
            if key.code == KeyCode::Char('q')
                || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                break;
            }
            if key.code == KeyCode::Char('?') {
                app.show_help = !app.show_help;
                continue;
            }
            if app.show_help {
                app.show_help = false;
                continue;
            }

            match key.code {
                KeyCode::Tab | KeyCode::Down => {
                    app.selected = (app.selected + 1) % FIELDS.len();
                }
                KeyCode::BackTab | KeyCode::Up => {
                    app.selected = (app.selected + FIELDS.len() - 1) % FIELDS.len();
                }
                KeyCode::Backspace => {
                    let f = app.current_field();
                    app.field_buf_mut(f).pop();
                }
                KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                    let f = app.current_field();
                    let buf = app.field_buf_mut(f);
                    // prevent double dots
                    if c == '.' && buf.contains('.') { continue; }
                    buf.push(c);
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

// ── UI ─────────────────────────────────────────────────────────────────────────
fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Full background
    f.render_widget(
        Block::default().style(Style::default().bg(C_BG)),
        size,
    );

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // title
            Constraint::Min(1),     // body
            Constraint::Length(1),  // footer
        ])
        .split(size);

    render_title(f, outer[0]);
    render_body(f, app, outer[1]);
    render_footer(f, outer[2]);

    if app.show_help {
        render_help(f, size);
    }
}

fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled("⬡ ", Style::default().fg(C_ACCENT)),
        Span::styled("KELLY CRITERION", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("  ─  ", Style::default().fg(C_DIM)),
        Span::styled("Position Sizing Calculator", Style::default().fg(C_TEXT)),
    ]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(C_DIM))
            .style(Style::default().bg(C_BG)),
    );
    f.render_widget(title, area);
}

fn render_body(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    render_inputs(f, app, cols[0]);
    render_results(f, app, cols[1]);
}

fn render_inputs(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Inputs ", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_DIM))
        .style(Style::default().bg(C_PANEL));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner);

    let fields_info: &[(Field, &str, &str, &str)] = &[
        (Field::Balance,       "Balance",         "$",   "Total trading capital available"),
        (Field::WinRate,       "Win Rate",        "%",   "% of trades that are winners (0-100)"),
        (Field::WinLossRatio,  "Win/Loss Ratio",  "x",   "Avg win / avg loss  e.g. 1.5"),
        (Field::FractionKelly, "Kelly Fraction",  "%",   "100=Full, 50=Half-Kelly (recommended)"),
        (Field::MaxRisk,       "Max Risk/Trade",  "%",   "Hard cap on % of balance per trade"),
    ];

    for (i, (field, label, unit, hint)) in fields_info.iter().enumerate() {
        let is_sel = app.selected == i;
        let buf = app.field_buf(*field);
        let valid = app.parse_f64(*field).is_some();

        let label_style = if is_sel {
            Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_TEXT)
        };

        let value_style = if !valid && !buf.is_empty() {
            Style::default().fg(C_RED)
        } else if is_sel {
            Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_TEXT)
        };

        let bg = if is_sel { C_HIGHLIGHT } else { C_PANEL };

        let cursor = if is_sel { "█" } else { "" };

        let row_block = Block::default()
            .style(Style::default().bg(bg));
        f.render_widget(row_block, rows[i]);

        // Label row
        let label_area = Rect {
            x: rows[i].x + 1,
            y: rows[i].y,
            width: rows[i].width.saturating_sub(2),
            height: 1,
        };
        let value_area = Rect {
            x: rows[i].x + 1,
            y: rows[i].y + 1,
            width: rows[i].width.saturating_sub(2),
            height: 1,
        };

        let prefix = if is_sel { "▶ " } else { "  " };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(C_ACCENT)),
                Span::styled(*label, label_style),
                Span::styled(format!(" ({})", hint), Style::default().fg(C_DIM)),
            ])),
            label_area,
        );

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(*unit, Style::default().fg(C_DIM)),
                Span::styled(" ", Style::default()),
                Span::styled(buf, value_style),
                Span::styled(cursor, Style::default().fg(C_ACCENT)),
            ])),
            value_area,
        );
    }
}

fn render_results(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Results ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_DIM))
        .style(Style::default().bg(C_PANEL));
    let inner = block.inner(area);
    f.render_widget(block, area);

    match app.calc() {
        None => {
            let msg = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  ⚠  Enter valid values in",
                    Style::default().fg(C_YELLOW),
                )),
                Line::from(Span::styled(
                    "     all fields to calculate.",
                    Style::default().fg(C_DIM),
                )),
            ])
            .style(Style::default().bg(C_PANEL));
            f.render_widget(msg, inner);
        }
        Some(r) => {
            if r.is_negative_ev {
                render_negative_ev(f, &r, inner);
            } else {
                render_positive_ev(f, &r, inner);
            }
        }
    }
}

fn render_negative_ev(f: &mut Frame, r: &KellyResult, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  ✖  NEGATIVE EV — DO NOT TRADE",
            Style::default().fg(C_RED).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Edge: ", Style::default().fg(C_DIM)),
            Span::styled(
                format!("{:.2}%", r.edge * 100.0),
                Style::default().fg(C_RED).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Kelly recommends no position.",
            Style::default().fg(C_TEXT),
        )),
        Line::from(Span::styled(
            "  This setup has no statistical",
            Style::default().fg(C_TEXT),
        )),
        Line::from(Span::styled(
            "  edge — expected value is negative.",
            Style::default().fg(C_TEXT),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(C_PANEL)),
        area,
    );
}

fn render_positive_ev(f: &mut Frame, r: &KellyResult, area: Rect) {
    // Risk bar
    let bar_width = area.width.saturating_sub(4) as usize;
    let filled = ((r.applied_pct / 100.0) * bar_width as f64).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let bar_color = if r.applied_pct < 3.0 { C_GREEN }
        else if r.applied_pct < 7.0 { C_YELLOW }
        else { C_RED };

    let bar: String = "█".repeat(filled) + &"░".repeat(empty);

    let lines = vec![
        Line::from(""),
        // ── Kelly values ──
        result_row("  Full Kelly",   format!("{:.2}%", r.full_kelly_pct), C_TEXT),
        result_row("  Frac Kelly",   format!("{:.2}%", r.frac_kelly_pct), C_ACCENT),
        result_row("  Applied",      format!("{:.2}%", r.applied_pct),    bar_color),
        Line::from(""),
        // ── Bar ──
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(&bar, Style::default().fg(bar_color)),
        ]),
        Line::from(vec![
            Span::styled("  Risk: ", Style::default().fg(C_DIM)),
            Span::styled(
                format!("{:.2}% of capital", r.applied_pct),
                Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        // ── Money ──
        Line::from(Span::styled("  ─── Amounts ────────────────", Style::default().fg(C_DIM))),
        result_row_money("  Balance",        r.balance,        C_TEXT),
        result_row_money("  Position Size",  r.position_size,  C_GREEN),
        result_row_money("  Max Loss",       r.max_loss,       C_RED),
        result_row_money("  Expected Gain",  r.expected_gain,  C_YELLOW),
        Line::from(""),
        // ── Stats ──
        Line::from(Span::styled("  ─── Statistics ─────────────", Style::default().fg(C_DIM))),
        result_row("  Edge",          format!("{:.2}%", r.edge * 100.0),         C_GREEN),
        result_row("  Ruin Risk",     format!("{:.4}%", r.ruin_approx * 100.0),  C_YELLOW),
    ];

    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(C_PANEL)),
        area,
    );
}

fn result_row(label: &'static str, value: String, color: Color) -> Line<'static> {
    let padded = format!("{:>width$}", value, width = 20usize.saturating_sub(label.len()));
    Line::from(vec![
        Span::styled(label, Style::default().fg(C_DIM)),
        Span::styled(padded, Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ])
}

fn result_row_money(label: &'static str, value: f64, color: Color) -> Line<'static> {
    let formatted = format!("${:.2}", value);
    let padded = format!("{:>width$}", formatted, width = 20usize.saturating_sub(label.len()));
    Line::from(vec![
        Span::styled(label, Style::default().fg(C_DIM)),
        Span::styled(padded, Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ])
}

fn render_footer(f: &mut Frame, area: Rect) {
    let text = Line::from(vec![
        Span::styled(" ↑↓/Tab ", Style::default().fg(C_ACCENT)),
        Span::styled("navigate  ", Style::default().fg(C_DIM)),
        Span::styled(" 0-9/. ", Style::default().fg(C_ACCENT)),
        Span::styled("edit  ", Style::default().fg(C_DIM)),
        Span::styled(" ? ", Style::default().fg(C_ACCENT)),
        Span::styled("help  ", Style::default().fg(C_DIM)),
        Span::styled(" q ", Style::default().fg(C_ACCENT)),
        Span::styled("quit", Style::default().fg(C_DIM)),
    ]);
    f.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().bg(C_BG).fg(C_DIM)),
        area,
    );
}

fn render_help(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 70, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  What is the Kelly Criterion?",
            Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  A formula for the optimal fraction",
            Style::default().fg(C_TEXT),
        )),
        Line::from(Span::styled(
            "  of capital to risk per trade.",
            Style::default().fg(C_TEXT),
        )),
        Line::from(""),
        Line::from(Span::styled("  f* = (b*p - q) / b", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled("  p  = win rate (probability of winning)", Style::default().fg(C_DIM))),
        Line::from(Span::styled("  q  = 1 - p  (probability of losing)", Style::default().fg(C_DIM))),
        Line::from(Span::styled("  b  = win/loss ratio (avg win / avg loss)", Style::default().fg(C_DIM))),
        Line::from(""),
        Line::from(Span::styled(
            "  ─── Parameters ─────────────────────",
            Style::default().fg(C_DIM),
        )),
        Line::from(Span::styled("  Balance      Your total account capital.", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  Win Rate     % of trades that close green.", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("  Win/Loss     Avg winner / avg loser in $.", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("               e.g. 1.5 = winners 50% larger.", Style::default().fg(C_DIM))),
        Line::from(Span::styled("  Kelly Frac   Scales down Full Kelly:", Style::default().fg(C_TEXT))),
        Line::from(Span::styled("               100=aggressive  50=recommended", Style::default().fg(C_DIM))),
        Line::from(Span::styled("  Max Risk     Hard cap per trade (safety net).", Style::default().fg(C_TEXT))),
        Line::from(""),
        Line::from(Span::styled(
            "  Edge = b*p - q   (positive = tradeable)",
            Style::default().fg(C_YELLOW),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close",
            Style::default().fg(C_DIM),
        )),
    ];

    let popup = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Span::styled(
                    " Help ",
                    Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(C_ACCENT))
                .style(Style::default().bg(C_PANEL)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(popup, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}