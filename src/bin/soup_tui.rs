//! Demo del driver rsoup con interfaz TUI (ratatui) y modo automatizable.
//!
//! Uso:
//!   soup_tui                      -> TUI interactiva
//!   soup_tui --list               -> lista los escenarios
//!   soup_tui --scenario <nombre>  -> ejecuta un escenario (headless, exit 0/1)
//!   soup_tui --host H --port P    -> conexión a otro gestor (defaults: 127.0.0.1:55432)
//!
//! Escenarios: ping, select, commit, rollback, error.

use std::io::stdout;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;
use rsoup::scenarios::{self, ScenarioResult, SCENARIOS};

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 55432;

#[derive(Default)]
struct Args {
    host: String,
    port: u16,
    list: bool,
    scenario: Option<String>,
}

fn parse_args() -> Args {
    let env_host = std::env::var("DRIVER_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let env_port = std::env::var("DRIVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let mut args = Args {
        host: env_host,
        port: env_port,
        ..Default::default()
    };
    let mut raw = std::env::args().skip(1);
    while let Some(arg) = raw.next() {
        match arg.as_str() {
            "--host" => {
                if let Some(v) = raw.next() {
                    args.host = v;
                }
            }
            "--port" => {
                if let Some(v) = raw.next() {
                    if let Ok(p) = v.parse() {
                        args.port = p;
                    }
                }
            }
            "--list" => args.list = true,
            "--scenario" => {
                if let Some(v) = raw.next() {
                    args.scenario = Some(v);
                }
            }
            _ => {}
        }
    }
    args
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args();

    if args.list {
        println!("Escenarios del driver rsoup:");
        for name in SCENARIOS {
            println!("  - {name}");
        }
        return Ok(());
    }

    if let Some(name) = args.scenario {
        return run_headless(&name, &args.host, args.port).await;
    }

    run_interactive(&args.host, args.port).await
}

async fn run_headless(name: &str, host: &str, port: u16) -> Result<()> {
    match scenarios::run(name, host, port).await {
        Ok(result) => {
            let mark = if result.ok { "OK" } else { "FAIL" };
            println!("[{mark}] {name}: {}", result.summary);
            if result.ok {
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("[ERROR] {name}: {error}");
            std::process::exit(1);
        }
    }
}

async fn run_interactive(host: &str, port: u16) -> Result<()> {
    enable_raw_mode().context("no se pudo activar el modo raw")?;
    let mut stdout = stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut selected = 0usize;
    let mut result: Option<ScenarioResult> = None;
    let mut running = false;
    let mut quit = false;

    while !quit {
        if running {
            let name = SCENARIOS[selected];
            result = Some(match scenarios::run(name, host, port).await {
                Ok(res) => res,
                Err(error) => ScenarioResult {
                    name,
                    ok: false,
                    summary: error.to_string(),
                },
            });
            running = false;
        }

        terminal.draw(|frame| draw(frame, host, port, selected, &result, running))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => quit = true,
                        KeyCode::Down => selected = (selected + 1).min(SCENARIOS.len() - 1),
                        KeyCode::Up => selected = selected.saturating_sub(1),
                        KeyCode::Enter => running = true,
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn draw(
    frame: &mut ratatui::Frame,
    host: &str,
    port: u16,
    selected: usize,
    result: &Option<ScenarioResult>,
    running: bool,
) {
    let vertical = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]);
    let [header, body] = vertical.areas(frame.area());

    let title = Paragraph::new(format!(" rsoup — driver SoupDB | {host}:{port} "))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, header);

    let horizontal = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]);
    let [left, right] = horizontal.areas(body);

    let items: Vec<ListItem> = SCENARIOS
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let style = if index == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(*name).style(style)
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" Escenarios "));
    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, left, &mut state);

    let (content, color) = if running {
        ("Ejecutando escenario...".to_string(), Color::Yellow)
    } else if let Some(result) = result {
        let mark = if result.ok { "OK" } else { "FAIL" };
        let color = if result.ok { Color::Green } else { Color::Red };
        (
            format!("[{mark}] {}\n\n{}", result.name, result.summary),
            color,
        )
    } else {
        (
            "Selecciona un escenario y presiona Enter.\n\n↑/↓ navegar · Enter ejecutar · q salir"
                .to_string(),
            Color::White,
        )
    };

    let paragraph = Paragraph::new(content)
        .style(Style::default().fg(color))
        .block(Block::default().borders(Borders::ALL).title(" Resultado "));
    frame.render_widget(paragraph, right);
}
