use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::help::{HelpMessage, HelpState};
use crate::ui::common::spinner_char;
use crate::ui::theme::{ACCENT, BG_PANEL, BORDER, DIM, ERROR, MUTED, TEXT, TEXT_BRIGHT};

//
// Render the documentation-helper overlay centered over the whole frame. The
// box shows the conversation (wrapped, scrollable), an input line, and a
// footer with the context-inclusion state and key hints.
//

pub fn render(f: &mut Frame, help: &HelpState) {
    let area = centered_rect(f.area(), 78, 80);
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .title(Span::styled(
            " Praxis Help ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(BG_PANEL));
    let inner = block.inner(area);
    f.render_widget(block, area);

    //
    // The input grows to wrap long questions across multiple rows so text
    // never runs off the edge. It is capped at MAX_INPUT_ROWS; beyond that it
    // scrolls to keep the caret (always at the end while typing) in view. A
    // one-row slack when wrapping absorbs word-wrap rounding so the caret line
    // is not clipped.
    //
    let input_line = build_input_line(help);
    let inner_width = inner.width.max(1);
    let base_rows = (input_line.width() as u16).max(1).div_ceil(inner_width);
    let est_rows = if base_rows > 1 { base_rows + 1 } else { 1 };
    let input_rows = est_rows.clamp(1, MAX_INPUT_ROWS);
    let input_scroll = est_rows.saturating_sub(input_rows);

    let rows = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(input_rows),
        Constraint::Length(1),
    ])
    .split(inner);

    render_conversation(f, rows[0], help);
    f.render_widget(
        Paragraph::new(input_line)
            .wrap(Wrap { trim: false })
            .scroll((input_scroll, 0)),
        rows[2],
    );
    render_footer(f, rows[3], help);
}

const MAX_INPUT_ROWS: u16 = 6;

fn render_conversation(f: &mut Frame, area: Rect, help: &HelpState) {
    let mut lines: Vec<Line> = Vec::new();

    if help.messages.is_empty() {
        lines.push(Line::from(Span::styled(
            "Ask anything about using Praxis — features, configuration, workflows.",
            Style::default().fg(MUTED),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "e.g. \"how do I run a recon operation?\" or \"what is this screen for?\"",
            Style::default().fg(DIM),
        )));
    }

    for message in &help.messages {
        match message {
            HelpMessage::User(text) => {
                lines.push(Line::from(Span::styled(
                    "You",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )));
                for l in text.lines() {
                    lines.push(Line::from(Span::styled(
                        l.to_string(),
                        Style::default().fg(TEXT_BRIGHT),
                    )));
                }
                lines.push(Line::from(""));
            }
            HelpMessage::Assistant(text) => {
                for l in text.lines() {
                    lines.push(Line::from(Span::styled(
                        l.to_string(),
                        Style::default().fg(TEXT),
                    )));
                }
                lines.push(Line::from(""));
            }
            HelpMessage::Error(text) => {
                lines.push(Line::from(Span::styled(
                    format!("Error: {}", text),
                    Style::default().fg(ERROR),
                )));
                lines.push(Line::from(""));
            }
        }
    }

    //
    // While waiting for the first token, show the same animated spinner the
    // orchestrator uses. Once assistant text starts arriving it streams in its
    // place, matching the orchestrator's behaviour.
    //
    let awaiting_text = !matches!(
        help.messages.last(),
        Some(HelpMessage::Assistant(t)) if !t.trim().is_empty()
    );
    if help.is_streaming && awaiting_text {
        lines.push(Line::from(Span::styled(
            format!("{} thinking", spinner_char()),
            Style::default().fg(MUTED),
        )));
    }

    //
    // `scroll` counts rows up from the bottom; translate to a top offset so
    // the newest output is visible by default and Up/PageUp reveal history.
    // Rows are counted post-wrap (each line occupies ceil(width / area_width)
    // rows) so the follow-bottom offset lands on the true end of long,
    // wrapped answers rather than an underestimate.
    //
    let width = area.width.max(1);
    let total: u16 = lines
        .iter()
        .map(|l| ((l.width() as u16).max(1)).div_ceil(width))
        .sum();
    let height = area.height.max(1);
    let max_top = total.saturating_sub(height);
    let top = max_top.saturating_sub(help.scroll);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((top, 0));
    f.render_widget(paragraph, area);
}

//
// Build the input as a single Line with a block cursor at the byte-offset
// caret position. The caller renders it with wrapping so long input flows onto
// additional rows; the caret span wraps along with the surrounding text.
//
fn build_input_line(help: &HelpState) -> Line<'static> {
    let (before, after) = help.input.split_at(help.cursor.min(help.input.len()));
    let mut spans = vec![
        Span::styled("> ", Style::default().fg(ACCENT)),
        Span::styled(before.to_string(), Style::default().fg(TEXT_BRIGHT)),
    ];

    let mut chars = after.chars();
    match chars.next() {
        Some(c) => {
            spans.push(Span::styled(
                c.to_string(),
                Style::default()
                    .fg(TEXT_BRIGHT)
                    .add_modifier(Modifier::REVERSED),
            ));
            spans.push(Span::styled(
                chars.as_str().to_string(),
                Style::default().fg(TEXT_BRIGHT),
            ));
        }
        None => {
            spans.push(Span::styled("\u{2588}", Style::default().fg(ACCENT)));
        }
    }

    Line::from(spans)
}

fn render_footer(f: &mut Frame, area: Rect, help: &HelpState) {
    let mut spans: Vec<Span> = Vec::new();

    let context_label = if help.context.is_none() {
        Span::styled("context: none", Style::default().fg(DIM))
    } else if help.include_context {
        let src = help.context_source.as_deref().unwrap_or("screen");
        Span::styled(
            format!("context: {} (on)", src),
            Style::default().fg(ACCENT),
        )
    } else {
        Span::styled("context: off", Style::default().fg(MUTED))
    };
    spans.push(context_label);

    spans.push(Span::styled("   ", Style::default()));

    let hints = if help.is_streaming {
        "Ctrl+C stop · Esc close"
    } else if help.context.is_some() {
        "Enter send · Ctrl+T toggle context · Esc close"
    } else {
        "Enter send · Esc close"
    };
    spans.push(Span::styled(hints, Style::default().fg(DIM)));

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

//
// A centered rectangle sized to `percent_x` × `percent_y` of `area`.
//

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}
