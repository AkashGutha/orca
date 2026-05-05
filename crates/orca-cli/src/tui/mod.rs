use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::render::plain;
use orca_core::output::{OutputEvent, OutputState};

mod theme;
mod widgets;

use theme::{
    COMPACT_HEADER_HEIGHT, LARGE_HEADER_HEIGHT, MAX_VISIBLE_PANES, MIN_LARGE_HEADER_AREA_HEIGHT,
};
use widgets::{draw_footer, draw_header, draw_iteration_summary, draw_panes};
pub async fn run(
    mut receiver: mpsc::UnboundedReceiver<OutputEvent>,
    stop_requested: Arc<AtomicBool>,
) {
    let mut terminal = match setup_terminal() {
        Ok(terminal) => terminal,
        Err(_) => {
            plain::run(receiver).await;
            return;
        }
    };
    let _guard = TerminalGuard;
    let mut state = OutputState::default();

    loop {
        loop {
            match receiver.try_recv() {
                Ok(event) => {
                    if !state.apply(event) {
                        let _ = terminal.draw(|frame| draw(frame, &state));
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let _ = terminal.draw(|frame| draw(frame, &state));
                    return;
                }
            }
        }

        let _ = terminal.draw(|frame| draw(frame, &state));

        if event::poll(Duration::from_millis(80)).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press && is_stop_key(key) => {
                    stop_requested.store(true, Ordering::SeqCst);
                    state.request_stop();
                    let _ = terminal.draw(|frame| draw(frame, &state));
                }
                _ => {}
            }
        }

        tokio::time::sleep(Duration::from_millis(80)).await;
    }
}

fn is_stop_key(key: crossterm::event::KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || key.code == KeyCode::Char('q')
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;
    Ok(terminal)
}

fn draw(frame: &mut ratatui::Frame<'_>, state: &OutputState) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    let header_height = if area.height >= MIN_LARGE_HEADER_AREA_HEIGHT {
        LARGE_HEADER_HEIGHT
    } else {
        COMPACT_HEADER_HEIGHT
    };
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);
    let header_area = vertical[0];
    let body_area = vertical[1];
    let footer_area = vertical[2];
    draw_header(frame, header_area);

    let panes = state.panes().take(MAX_VISIBLE_PANES).collect::<Vec<_>>();
    if let Some(summary) = state.iteration_summary() {
        draw_iteration_summary(frame, body_area, summary);
        draw_footer(frame, footer_area, state.stop_requested());
        return;
    }

    if panes.is_empty() {
        let message = state
            .active_phase()
            .map(|phase| format!("Starting {phase} phase..."))
            .unwrap_or_else(|| "Waiting for agents...".to_string());
        let paragraph =
            Paragraph::new(message).block(Block::default().title("ORCA").borders(Borders::ALL));
        frame.render_widget(Clear, body_area);
        frame.render_widget(paragraph, body_area);
        draw_footer(frame, footer_area, state.stop_requested());
        return;
    }

    draw_panes(frame, body_area, &panes);

    draw_footer(frame, footer_area, state.stop_requested());
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}
