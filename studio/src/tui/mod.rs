//! Director TUI — primary way to use IdleScreen Studio.
//!
//! Queue jobs, edit export params, run `render`, watch status. No GUI required.

pub mod actions;
pub mod draw;
pub mod form;

use crate::error::StudioError;
use crate::job::StudioJob;
use crate::queue::{JobQueue, JobStatus};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout};
use std::path::Path;
use std::time::Duration;

use self::actions::{run_job_at, run_jobs, selected_pending_or_any};
use self::draw::{draw_new_job, draw_queue};
use self::form::{FormField, NewJobForm, Screen};

pub use self::form::EFFECTS;

/// Run the interactive Director until quit. This is the primary Studio UX.
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
    let mut screen = Screen::Queue;
    let mut form = NewJobForm::default();
    let mut quit = false;

    while !quit {
        if selected >= queue.entries.len() && !queue.entries.is_empty() {
            selected = queue.entries.len() - 1;
        }
        terminal
            .draw(|f| match screen {
                Screen::Queue => draw_queue(f.area(), f, &queue, selected, &status_line),
                Screen::NewJob => draw_new_job(f.area(), f, &form, &status_line),
            })
            .map_err(|e| StudioError::Queue(e.to_string()))?;

        if !event::poll(Duration::from_millis(200)).unwrap_or(false) {
            continue;
        }
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match screen {
            Screen::Queue => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => quit = true,
                KeyCode::Char('j') | KeyCode::Down => {
                    if !queue.entries.is_empty() {
                        selected = (selected + 1).min(queue.entries.len() - 1);
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Char('n') => {
                    form = NewJobForm::default();
                    screen = Screen::NewJob;
                    status_line = "new job — Tab fields, Enter edit/save, Esc cancel".into();
                }
                KeyCode::Char('R') => {
                    queue = JobQueue::load(queue_path)?;
                    status_line = "reloaded".into();
                }
                KeyCode::Char('r') => {
                    status_line = run_jobs(&mut queue, queue_path, false);
                }
                KeyCode::Char('a') => {
                    status_line = run_jobs(&mut queue, queue_path, true);
                }
                KeyCode::Enter => {
                    if let Some(idx) = selected_pending_or_any(&queue, selected) {
                        status_line = run_job_at(&mut queue, queue_path, idx);
                    } else {
                        status_line = "no job selected".into();
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    if !queue.entries.is_empty() && selected < queue.entries.len() {
                        let id = queue.entries[selected].job.id.clone();
                        queue.entries.remove(selected);
                        if selected >= queue.entries.len() && selected > 0 {
                            selected -= 1;
                        }
                        let _ = queue.save(queue_path);
                        status_line = format!("deleted {id}");
                    }
                }
                KeyCode::Char('p') => {
                    // Re-queue failed/done as pending
                    if selected < queue.entries.len() {
                        queue.entries[selected].status = JobStatus::Pending;
                        queue.entries[selected].message.clear();
                        let _ = queue.save(queue_path);
                        status_line = format!("pending {}", queue.entries[selected].job.id);
                    }
                }
                _ => {}
            },
            Screen::NewJob => {
                if form.editing {
                    match key.code {
                        KeyCode::Esc => {
                            form.editing = false;
                            status_line = "edit cancelled".into();
                        }
                        KeyCode::Enter => {
                            form.editing = false;
                            status_line = "field set".into();
                        }
                        KeyCode::Backspace => {
                            if let Some(t) = form.text_mut() {
                                t.pop();
                            }
                        }
                        KeyCode::Char(c)
                            if !key.modifiers.contains(KeyModifiers::CONTROL)
                                && !key.modifiers.contains(KeyModifiers::ALT) =>
                        {
                            if let Some(t) = form.text_mut() {
                                t.push(c);
                            }
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            screen = Screen::Queue;
                            status_line = "cancelled new job".into();
                        }
                        KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => form.next_field(1),
                        KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => form.next_field(-1),
                        KeyCode::Left | KeyCode::Char('h') => {
                            if form.field() == FormField::Effect {
                                form.cycle_effect(-1);
                            } else if form.field() == FormField::DryRun {
                                form.dry_run = !form.dry_run;
                            }
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            if form.field() == FormField::Effect {
                                form.cycle_effect(1);
                            } else if form.field() == FormField::DryRun {
                                form.dry_run = !form.dry_run;
                            }
                        }
                        KeyCode::Char(' ') if form.field() == FormField::DryRun => {
                            form.dry_run = !form.dry_run;
                        }
                        KeyCode::Enter => match form.field() {
                            FormField::Effect => form.cycle_effect(1),
                            FormField::DryRun => form.dry_run = !form.dry_run,
                            FormField::Duration
                            | FormField::Width
                            | FormField::Height
                            | FormField::Fps
                            | FormField::Crf
                            | FormField::Output => {
                                form.editing = true;
                                status_line = "editing — type, Enter done, Esc cancel".into();
                            }
                        },
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            // Save / enqueue
                            match form.to_job_spec() {
                                Ok(spec) => {
                                    let id = format!("job-{}", queue.entries.len() + 1);
                                    queue.enqueue(StudioJob::new(id.clone(), spec));
                                    if let Err(e) = queue.save(queue_path) {
                                        status_line = format!("save failed: {e}");
                                    } else {
                                        selected = queue.entries.len().saturating_sub(1);
                                        screen = Screen::Queue;
                                        status_line = format!("enqueued {id}");
                                    }
                                }
                                Err(e) => status_line = e,
                            }
                        }
                        KeyCode::Char('q') => {
                            screen = Screen::Queue;
                            status_line = "cancelled new job".into();
                        }
                        _ => {}
                    }
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
