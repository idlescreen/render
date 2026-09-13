//! Key handling for both TUI screens — kept separate so mod.rs stays a
//! small event pump under the line cap.

use crate::job::StudioJob;
use crate::queue::{JobQueue, JobStatus};
use crate::tui::actions::{selected_pending_or_any, spawn_runner, RunWorker};
use crate::tui::form::{FormField, NewJobForm, Screen};
use crossterm::event::{KeyCode, KeyModifiers};
use std::path::Path;
use std::sync::atomic::Ordering;

/// Outcome of a queue-screen keypress.
pub enum KeyOutcome {
    Continue,
    Quit,
}

/// Shared mutable UI state threaded through key handlers.
pub struct Ui<'a> {
    pub queue: &'a mut JobQueue,
    pub queue_path: &'a Path,
    pub screen: &'a mut Screen,
    pub selected: &'a mut usize,
    pub status: &'a mut String,
    pub running: &'a mut Option<RunWorker>,
}

fn start_run(ui: &mut Ui<'_>, indices: Vec<usize>) -> Option<RunWorker> {
    let jobs: Vec<StudioJob> = indices
        .into_iter()
        .filter_map(|i| ui.queue.entries.get(i).map(|e| e.job.clone()))
        .collect();
    if jobs.is_empty() {
        return None;
    }
    let n = jobs.len();
    *ui.status = format!("running {n} job(s) — x cancels");
    Some(spawn_runner(jobs))
}

pub fn queue_key(code: KeyCode, ui: &mut Ui<'_>) -> KeyOutcome {
    if let Some(worker) = ui.running.as_ref() {
        if code == KeyCode::Char('x') {
            worker.cancel.store(true, Ordering::Relaxed);
            *ui.status = "cancelling…".into();
        }
        // Only navigation/cancel while a render runs — mutations wait.
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !ui.queue.entries.is_empty() {
                    *ui.selected = (*ui.selected + 1).min(ui.queue.entries.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => *ui.selected = ui.selected.saturating_sub(1),
            KeyCode::Char('q') | KeyCode::Esc => return KeyOutcome::Quit,
            _ => {}
        }
        return KeyOutcome::Continue;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Esc => return KeyOutcome::Quit,
        KeyCode::Char('j') | KeyCode::Down => {
            if !ui.queue.entries.is_empty() {
                *ui.selected = (*ui.selected + 1).min(ui.queue.entries.len() - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => *ui.selected = ui.selected.saturating_sub(1),
        KeyCode::Char('R') => match JobQueue::load_or_recover(ui.queue_path) {
            Ok(q) => {
                *ui.queue = q;
                *ui.status = "reloaded".into();
            }
            Err(e) => *ui.status = format!("reload failed: {e}"),
        },
        KeyCode::Char('r') => {
            *ui.running = ui
                .queue
                .next_pending_index()
                .and_then(|i| start_run(ui, vec![i]));
            if ui.running.is_none() {
                *ui.status = "no pending jobs".into();
            }
        }
        KeyCode::Char('a') => {
            let idxs: Vec<usize> = ui
                .queue
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.status == JobStatus::Pending)
                .map(|(i, _)| i)
                .collect();
            *ui.running = start_run(ui, idxs);
            if ui.running.is_none() {
                *ui.status = "no pending jobs".into();
            }
        }
        KeyCode::Enter => {
            if let Some(idx) = selected_pending_or_any(ui.queue, *ui.selected) {
                *ui.running = start_run(ui, vec![idx]);
            } else {
                *ui.status = "no job selected".into();
            }
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            if !ui.queue.entries.is_empty() && *ui.selected < ui.queue.entries.len() {
                let id = ui.queue.entries[*ui.selected].job.id.clone();
                ui.queue.entries.remove(*ui.selected);
                if *ui.selected >= ui.queue.entries.len() && *ui.selected > 0 {
                    *ui.selected -= 1;
                }
                let _ = ui.queue.save(ui.queue_path);
                *ui.status = format!("deleted {id}");
            }
        }
        // Re-queue failed/done as pending
        KeyCode::Char('p') if *ui.selected < ui.queue.entries.len() => {
            ui.queue.entries[*ui.selected].status = JobStatus::Pending;
            ui.queue.entries[*ui.selected].message.clear();
            let _ = ui.queue.save(ui.queue_path);
            *ui.status = format!("pending {}", ui.queue.entries[*ui.selected].job.id);
        }
        _ => {}
    }
    KeyOutcome::Continue
}

pub fn form_key(code: KeyCode, modifiers: KeyModifiers, form: &mut NewJobForm, ui: &mut Ui<'_>) {
    if form.editing {
        match code {
            KeyCode::Esc => {
                form.editing = false;
                *ui.status = "edit cancelled".into();
            }
            KeyCode::Enter => {
                form.editing = false;
                *ui.status = "field set".into();
            }
            KeyCode::Backspace => {
                if let Some(t) = form.text_mut() {
                    t.pop();
                }
            }
            KeyCode::Char(c)
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(t) = form.text_mut() {
                    t.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Esc => {
            *ui.screen = Screen::Queue;
            *ui.status = "cancelled new job".into();
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
                *ui.status = "editing — type, Enter done, Esc cancel".into();
            }
        },
        KeyCode::Char('s') | KeyCode::Char('S') => match form.to_job_spec() {
            Ok(spec) => {
                let id = ui.queue.next_id();
                ui.queue.enqueue(StudioJob::new(id.clone(), spec));
                match ui.queue.save(ui.queue_path) {
                    Ok(()) => {
                        *ui.selected = ui.queue.entries.len().saturating_sub(1);
                        *ui.screen = Screen::Queue;
                        *ui.status = format!("enqueued {id}");
                    }
                    Err(e) => *ui.status = format!("save failed: {e}"),
                }
            }
            Err(e) => *ui.status = e,
        },
        KeyCode::Char('q') => {
            *ui.screen = Screen::Queue;
            *ui.status = "cancelled new job".into();
        }
        _ => {}
    }
}
