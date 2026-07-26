//! Minimal Director TUI: list queue, run next/all, quit.

use crate::error::StudioError;
use crate::queue::{JobQueue, JobStatus};
use crate::runner::run_job;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;
use std::io::{self, stdout};
use std::path::Path;
use std::time::Duration;

/// Run the interactive Director until quit.
pub fn run_tui(queue_path: &Path) -> Result<(), StudioError> {
    enable_raw_mode().map_err(|e| StudioError::Queue(e.to_string()))?;
    stdout()
        .execute(EnterAlternateScreen)
        .map_err(|e| StudioError::Queue(e.to_string()))?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| StudioError::Queue(e.to_string()))?;

    let mut queue = JobQueue::load(queue_path)?;
    let mut selected: usize = 0;
    let mut status_line = format!("queue: {}", queue_path.display());
    let mut quit = false;

    while !quit {
        if selected >= queue.entries.len() && !queue.entries.is_empty() {
            selected = queue.entries.len() - 1;
        }
        terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(5),
                        Constraint::Length(3),
                    ])
                    .split(f.area());
                let title = Paragraph::new("IdleScreen Studio — Director")
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(title, chunks[0]);

                let items: Vec<ListItem> = queue
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        let mark = if i == selected { ">" } else { " " };
                        let line = format!(
                            "{mark} {} {:?} {} → {}",
                            e.job.id,
                            e.status,
                            e.job.spec.effect,
                            e.job.spec.output.display()
                        );
                        ListItem::new(line)
                    })
                    .collect();
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title("Jobs"))
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD));
                f.render_widget(list, chunks[1]);

                let help = Paragraph::new(Line::from(vec![
                    Span::raw("j/k move  r run-next  a run-all  R reload  q quit  | "),
                    Span::raw(status_line.as_str()),
                ]))
                .block(Block::default().borders(Borders::ALL));
                f.render_widget(help, chunks[2]);
            })
            .map_err(|e| StudioError::Queue(e.to_string()))?;

        if event::poll(Duration::from_millis(200)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => quit = true,
                    KeyCode::Char('j') | KeyCode::Down => {
                        if !queue.entries.is_empty() {
                            selected = (selected + 1).min(queue.entries.len() - 1);
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Char('R') => {
                        queue = JobQueue::load(queue_path)?;
                        status_line = "reloaded".into();
                    }
                    KeyCode::Char('r') => {
                        status_line = run_one(&mut queue, queue_path, false);
                    }
                    KeyCode::Char('a') => {
                        status_line = run_one(&mut queue, queue_path, true);
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode().map_err(|e| StudioError::Queue(e.to_string()))?;
    stdout()
        .execute(LeaveAlternateScreen)
        .map_err(|e| StudioError::Queue(e.to_string()))?;
    let _ = io::Write::flush(&mut stdout());
    Ok(())
}

fn run_one(queue: &mut JobQueue, path: &Path, all: bool) -> String {
    let mut last = String::from("no pending");
    while let Some(idx) = queue.next_pending_index() {
        queue.entries[idx].status = JobStatus::Running;
        let _ = queue.save(path);
        let job = queue.entries[idx].job.clone();
        match run_job(&job) {
            Ok(msg) => {
                queue.entries[idx].status = JobStatus::Done;
                queue.entries[idx].message = msg;
                last = format!("done {}", job.id);
            }
            Err(e) => {
                queue.entries[idx].status = JobStatus::Failed;
                queue.entries[idx].message = e.to_string();
                last = format!("failed {}: {e}", job.id);
            }
        }
        let _ = queue.save(path);
        if !all {
            break;
        }
    }
    last
}
