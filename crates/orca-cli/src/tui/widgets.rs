use orca_core::output::{AgentPaneState, AgentStatus, IterationSummaryState};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::theme::{
    LARGE_HEADER_HEIGHT, ORCA_AMBER, ORCA_BANNER_LINES, ORCA_GOLD, ORCA_LIGHT_GOLD, ORCA_ORANGE,
    ORCA_RED,
};

pub(super) fn draw_panes(frame: &mut ratatui::Frame<'_>, area: Rect, panes: &[&AgentPaneState]) {
    let constraints = vec![Constraint::Ratio(1, panes.len() as u32); panes.len()];
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .spacing(1)
        .split(area);

    for (pane, chunk) in panes.iter().zip(chunks.iter()) {
        let inner_width = chunk.width.saturating_sub(2) as usize;
        let title = pane_title(pane, inner_width);
        let style = match pane.status {
            AgentStatus::Running => Style::default().fg(Color::Cyan),
            AgentStatus::Succeeded => Style::default().fg(Color::Green),
            AgentStatus::Failed => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        };
        let visible_height = chunk.height.saturating_sub(2) as usize;
        let lines = pane
            .lines
            .iter()
            .rev()
            .take(visible_height.saturating_mul(3).max(visible_height))
            .rev()
            .map(|line| Line::from(Span::raw(line.clone())))
            .collect::<Vec<_>>();
        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .style(style),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(Clear, *chunk);
        frame.render_widget(paragraph, *chunk);
    }
}

pub(super) fn draw_iteration_summary(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    summary: &IterationSummaryState,
) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let lines = vec![
        Line::from(Span::raw(truncate_for_width(&summary.summary, inner_width))),
        Line::from(Span::raw(truncate_for_width(
            &summary.next_step,
            inner_width,
        ))),
    ];
    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(format!(" Iteration {} summary ", summary.iteration))
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
    );
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

pub(super) fn draw_header(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let banner_width = ORCA_BANNER_LINES
        .iter()
        .map(|line| UnicodeWidthStr::width(*line))
        .max()
        .unwrap_or(0);
    if area.height >= LARGE_HEADER_HEIGHT && banner_width <= inner_width {
        draw_large_header(frame, area);
        return;
    }

    let full = "🐋 ORCA 🐋";
    let line = if UnicodeWidthStr::width(full) <= inner_width {
        Line::from(vec![
            Span::styled("🐋 ", Style::default().fg(ORCA_LIGHT_GOLD)),
            Span::styled(
                "OR",
                Style::default().fg(ORCA_RED).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "CA",
                Style::default().fg(ORCA_GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" 🐋", Style::default().fg(ORCA_LIGHT_GOLD)),
        ])
    } else {
        Line::from(Span::styled(
            truncate_for_width("ORCA", inner_width),
            Style::default().fg(ORCA_AMBER).add_modifier(Modifier::BOLD),
        ))
    };

    let header = Paragraph::new(line)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(Clear, area);
    frame.render_widget(header, area);
}

fn draw_large_header(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let banner_colors = [
        ORCA_RED,
        ORCA_ORANGE,
        ORCA_AMBER,
        Color::Rgb(255, 162, 0),
        ORCA_GOLD,
        ORCA_LIGHT_GOLD,
    ];
    let mut lines = ORCA_BANNER_LINES
        .iter()
        .zip(banner_colors)
        .map(|(line, color)| {
            Line::from(Span::styled(
                *line,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ))
        })
        .collect::<Vec<_>>();
    lines.push(Line::from(vec![
        Span::styled(
            "ORCA",
            Style::default().fg(ORCA_GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Agents orchestration platform",
            Style::default().fg(ORCA_LIGHT_GOLD),
        ),
    ]));

    let header = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(Clear, area);
    frame.render_widget(header, area);
}

pub(super) fn draw_footer(frame: &mut ratatui::Frame<'_>, area: Rect, stop_requested: bool) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let text = if stop_requested {
        "Stopping agents... running child processes are being terminated. Fallback: rerun with --plain if the TUI misbehaves."
    } else {
        "Stop: press q, Esc, or Ctrl-C. Fallback: rerun with --plain for line-oriented output."
    };
    let style = if stop_requested {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let footer = Paragraph::new(truncate_for_width(text, inner_width))
        .style(style)
        .block(Block::default().title("Controls").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(Clear, area);
    frame.render_widget(footer, area);
}

fn pane_title(pane: &AgentPaneState, max_width: usize) -> String {
    let model = pane.model.as_deref().unwrap_or("n/a");
    let title = format!(
        " {} | {} | model: {} | {} ",
        pane.id,
        pane.label,
        model,
        status_label(pane.status)
    );
    truncate_for_width(&title, max_width)
}

fn status_label(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Running => "running",
        AgentStatus::Succeeded => "done",
        AgentStatus::Failed => "failed",
    }
}

pub(super) fn truncate_for_width(line: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(line) < max_width {
        return line.to_string();
    }

    let mut truncated = String::new();
    let mut width = 0usize;
    for ch in line.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width >= max_width {
            truncated.push('…');
            return truncated;
        }
        truncated.push(ch);
        width += ch_width;
    }
    truncated
}
