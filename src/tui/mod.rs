//! Director TUI — primary way to use IdleScreen Studio.
//!
//! Queue jobs, edit export params, run `render`, watch status. No GUI required.

pub mod actions;
pub mod draw;
pub mod form;
mod keys;

use crate::error::StudioError;
use crate::queue::JobQueue;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout};
use std::path::Path;
use std::time::Duration;

use self::actions::{apply_run_event, RunEvent, RunWorker};
use self::draw::{draw_new_job, draw_queue};
use self::form::{NewJobForm, Screen};
use self::keys::{form_key, queue_key, KeyOutcome};

pub use self::form::EFFECTS;

/// Restores the terminal (raw mode + alternate screen) on every exit path —
/// including `?` early returns and panics. Without this, a corrupt queue or
/// a draw error used to leave the caller's shell in raw mode.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self, StudioError> {
        enable_raw_mode().map_err(|e| StudioError::Queue(e.to_string()))?;
        stdout()
            .execute(EnterAlternateScreen)
            .map_err(|e| StudioError::Queue(e.to_string()))?;
        let prior = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = stdout().execute(LeaveAlternateScreen);
            prior(info);
        }));
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = io::Write::flush(&mut stdout());
    }
}

/// Run the interactive Director until quit. This is the primary Studio UX.
pub fn run_tui(queue_path: &Path) -> Result<(), StudioError> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| StudioError::Queue(e.to_string()))?;

    let mut queue = JobQueue::load_or_recover(queue_path)?;
    let mut selected: usize = 0;
    let mut status_line = format!("queue: {}", queue_path.display());
    let mut screen = Screen::Queue;
    let mut form = NewJobForm::default();
    let mut running: Option<RunWorker> = None;
    let mut quit = false;

    while !quit {
        // Drain worker events every loop turn — render status never waits
        // on keypresses.
        if let Some(worker) = &running {
            let mut worker_done = false;
            while let Ok(ev) = worker.rx.try_recv() {
                if matches!(ev, RunEvent::AllDone) {
                    worker_done = true;
                }
                if let Some(msg) = apply_run_event(&mut queue, queue_path, ev) {
                    status_line = msg;
                }
            }
            if worker_done {
                running = None;
            }
        }

        if selected >= queue.entries.len() && !queue.entries.is_empty() {
            selected = queue.entries.len() - 1;
        }
        terminal
            .draw(|f| match screen {
                Screen::Queue => draw_queue(f.area(), f, &queue, selected, &status_line),
                Screen::NewJob => draw_new_job(f.area(), f, &form, &status_line),
            })
            .map_err(|e| StudioError::Queue(e.to_string()))?;

        if !event::poll(Duration::from_millis(100)).unwrap_or(false) {
            continue;
        }
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match screen {
            Screen::Queue => {
                // `n` needs `form` too, so it lives at the dispatch level.
                if key.code == KeyCode::Char('n') && running.is_none() {
                    form = NewJobForm::default();
                    screen = Screen::NewJob;
                    status_line = "new job — Tab fields, Enter edit/save, Esc cancel".into();
                } else {
                    let mut ui = keys::Ui {
                        queue: &mut queue,
                        queue_path,
                        screen: &mut screen,
                        selected: &mut selected,
                        status: &mut status_line,
                        running: &mut running,
                    };
                    if matches!(queue_key(key.code, &mut ui), KeyOutcome::Quit) {
                        quit = true;
                    }
                }
            }
            Screen::NewJob => {
                let mut ui = keys::Ui {
                    queue: &mut queue,
                    queue_path,
                    screen: &mut screen,
                    selected: &mut selected,
                    status: &mut status_line,
                    running: &mut running,
                };
                form_key(key.code, key.modifiers, &mut form, &mut ui);
            }
        }
    }

    // Quitting mid-render: cancel and give the worker a beat to SIGKILL the
    // child — otherwise the render orphans and keeps writing a half-done file.
    if let Some(worker) = running.take() {
        worker
            .cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match worker.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(RunEvent::AllDone) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }
    Ok(())
}
