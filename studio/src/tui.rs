//! Director TUI — primary way to use IdleScreen Studio.
//!
//! Queue jobs, edit export params, run `render`, watch status. No GUI required.

use crate::error::StudioError;
use crate::job::StudioJob;
use crate::queue::{JobQueue, JobStatus};
use crate::runner::run_job;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use idle_render::JobSpec;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Terminal;
use std::io::{self, stdout};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Official saver basenames (same set as idle-savers / host packages).
pub const EFFECTS: &[&str] = &[
    "beams", "bursts", "chaos", "cosmos", "glyphs", "gnats", "hearth", "radar", "ripple",
    "storm",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Queue,
    NewJob,
}

/// Fields on the new-job form (cycle with Tab / j/k).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormField {
    Effect,
    Duration,
    Width,
    Height,
    Fps,
    Crf,
    Output,
    DryRun,
}

const FORM_FIELDS: &[FormField] = &[
    FormField::Effect,
    FormField::Duration,
    FormField::Width,
    FormField::Height,
    FormField::Fps,
    FormField::Crf,
    FormField::Output,
    FormField::DryRun,
];

struct NewJobForm {
    field_i: usize,
    effect_i: usize,
    duration: String,
    width: String,
    height: String,
    fps: String,
    crf: String,
    output: String,
    dry_run: bool,
    /// When true, typing edits the focused text field.
    editing: bool,
}

impl Default for NewJobForm {
    fn default() -> Self {
        let effect = EFFECTS[0];
        Self {
            field_i: 0,
            effect_i: 0,
            duration: "10s".into(),
            width: "1280".into(),
            height: "720".into(),
            fps: "30".into(),
            crf: "35".into(),
            output: default_output_path(effect),
            dry_run: false,
            editing: false,
        }
    }
}

impl NewJobForm {
    fn field(&self) -> FormField {
        FORM_FIELDS[self.field_i % FORM_FIELDS.len()]
    }

    fn effect(&self) -> &'static str {
        EFFECTS[self.effect_i % EFFECTS.len()]
    }

    fn cycle_effect(&mut self, dir: i32) {
        let n = EFFECTS.len() as i32;
        let i = self.effect_i as i32 + dir;
        self.effect_i = ((i % n) + n) as usize % EFFECTS.len();
        // Refresh default output stem when effect changes (if still on pattern).
        self.output = default_output_path(self.effect());
    }

    fn next_field(&mut self, dir: i32) {
        let n = FORM_FIELDS.len() as i32;
        let i = self.field_i as i32 + dir;
        self.field_i = ((i % n) + n) as usize % FORM_FIELDS.len();
        self.editing = false;
    }

    fn text_mut(&mut self) -> Option<&mut String> {
        match self.field() {
            FormField::Duration => Some(&mut self.duration),
            FormField::Width => Some(&mut self.width),
            FormField::Height => Some(&mut self.height),
            FormField::Fps => Some(&mut self.fps),
            FormField::Crf => Some(&mut self.crf),
            FormField::Output => Some(&mut self.output),
            FormField::Effect | FormField::DryRun => None,
        }
    }

    fn to_job_spec(&self) -> Result<JobSpec, String> {
        let width: u32 = self
            .width
            .trim()
            .parse()
            .map_err(|_| "width must be a number".to_string())?;
        let height: u32 = self
            .height
            .trim()
            .parse()
            .map_err(|_| "height must be a number".to_string())?;
        let fps: u32 = self
            .fps
            .trim()
            .parse()
            .map_err(|_| "fps must be a number".to_string())?;
        let crf: u8 = self
            .crf
            .trim()
            .parse()
            .map_err(|_| "crf must be 0–63".to_string())?;
        if width == 0 || height == 0 || fps == 0 {
            return Err("width, height, and fps must be > 0".into());
        }
        let output = PathBuf::from(self.output.trim());
        if output.as_os_str().is_empty() {
            return Err("output path required".into());
        }
        Ok(JobSpec {
            effect: self.effect().to_string(),
            plugin_path: None,
            seed: 0x00C0_FFEE,
            fps,
            duration: self.duration.trim().to_string(),
            output,
            width,
            height,
            cols: None,
            rows: None,
            dry_run: self.dry_run,
            raw: false,
            segment: None,
            audio: None,
            resume: false,
            crf,
            preset: None,
            encoder: None,
            prefer_hw: true,
            gpu_upscale: false, // match host: CPU upscale path for predictable export
        })
    }
}

fn default_output_path(effect: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = std::env::var("HOME")
        .map(|h| format!("{h}/Videos"))
        .unwrap_or_else(|_| "/tmp".into());
    format!("{dir}/idlescreen-{effect}-{ts}.mkv")
}

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
            .draw(|f| {
                match screen {
                    Screen::Queue => draw_queue(f.area(), f, &queue, selected, &status_line),
                    Screen::NewJob => draw_new_job(f.area(), f, &form, &status_line),
                }
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
            Screen::Queue => {
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
                }
            }
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
                        KeyCode::Enter => {
                            match form.field() {
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
                            }
                        }
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

fn draw_queue(
    area: Rect,
    f: &mut ratatui::Frame<'_>,
    queue: &JobQueue,
    selected: usize,
    status_line: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("IdleScreen Studio — Director (TUI)")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = if queue.entries.is_empty() {
        vec![ListItem::new("  (empty — press n to add a job)")]
    } else {
        queue
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let mark = if i == selected { ">" } else { " " };
                let st = match e.status {
                    JobStatus::Pending => "PEND",
                    JobStatus::Running => "RUN ",
                    JobStatus::Done => "DONE",
                    JobStatus::Failed => "FAIL",
                };
                ListItem::new(format!(
                    "{mark} [{st}] {}  {}  {}x{} @{}fps  {}  → {}",
                    e.job.id,
                    e.job.spec.effect,
                    e.job.spec.width,
                    e.job.spec.height,
                    e.job.spec.fps,
                    e.job.spec.duration,
                    e.job.spec.output.display()
                ))
            })
            .collect()
    };
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Queue"));
    f.render_widget(list, chunks[1]);

    let detail = if let Some(e) = queue.entries.get(selected) {
        let msg = if e.message.is_empty() {
            "(no message)".into()
        } else {
            e.message.chars().take(200).collect::<String>()
        };
        format!(
            "id={}  status={:?}\neffect={}  dry_run={}\n{}",
            e.job.id, e.status, e.job.spec.effect, e.job.spec.dry_run, msg
        )
    } else {
        "No selection".into()
    };
    f.render_widget(
        Paragraph::new(detail)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Selected")),
        chunks[2],
    );

    let help = Paragraph::new(Line::from(vec![
        Span::raw("n new  j/k move  Enter run  r next  a all  d del  p re-pend  R reload  q quit  | "),
        Span::raw(status_line),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}

fn draw_new_job(area: Rect, f: &mut ratatui::Frame<'_>, form: &NewJobForm, status_line: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new("New export job")
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL)),
        chunks[0],
    );

    let mut lines = Vec::new();
    for (i, field) in FORM_FIELDS.iter().enumerate() {
        let focus = i == form.field_i;
        let mark = if focus { ">" } else { " " };
        let edit = if focus && form.editing { " [edit]" } else { "" };
        let value = match field {
            FormField::Effect => format!("{}  (h/l cycle)", form.effect()),
            FormField::Duration => form.duration.clone(),
            FormField::Width => form.width.clone(),
            FormField::Height => form.height.clone(),
            FormField::Fps => form.fps.clone(),
            FormField::Crf => form.crf.clone(),
            FormField::Output => form.output.clone(),
            FormField::DryRun => {
                if form.dry_run {
                    "yes".into()
                } else {
                    "no".into()
                }
            }
        };
        let label = match field {
            FormField::Effect => "effect",
            FormField::Duration => "duration",
            FormField::Width => "width",
            FormField::Height => "height",
            FormField::Fps => "fps",
            FormField::Crf => "crf",
            FormField::Output => "output",
            FormField::DryRun => "dry_run",
        };
        let line = format!("{mark} {label:<10} {value}{edit}");
        if focus {
            lines.push(Line::from(Span::styled(
                line,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(line));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Tab/j/k field  h/l effect or dry_run  Enter edit/toggle  s save to queue  Esc cancel",
    ));

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Parameters")),
        chunks[1],
    );

    f.render_widget(
        Paragraph::new(status_line).block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );

    // Dim overlay hint when empty area (no-op Clear keeps layout stable)
    let _ = Clear;
}

fn selected_pending_or_any(queue: &JobQueue, selected: usize) -> Option<usize> {
    if queue.entries.is_empty() {
        return None;
    }
    if selected < queue.entries.len() {
        return Some(selected);
    }
    Some(0)
}

fn run_jobs(queue: &mut JobQueue, path: &Path, all: bool) -> String {
    let mut last = String::from("no pending");
    while let Some(idx) = queue.next_pending_index() {
        last = run_job_at(queue, path, idx);
        if !all {
            break;
        }
    }
    last
}

fn run_job_at(queue: &mut JobQueue, path: &Path, idx: usize) -> String {
    if idx >= queue.entries.len() {
        return "bad index".into();
    }
    queue.entries[idx].status = JobStatus::Running;
    let _ = queue.save(path);
    let job = queue.entries[idx].job.clone();
    match run_job(&job) {
        Ok(msg) => {
            queue.entries[idx].status = JobStatus::Done;
            queue.entries[idx].message = msg;
            let _ = queue.save(path);
            format!("done {}", job.id)
        }
        Err(e) => {
            queue.entries[idx].status = JobStatus::Failed;
            queue.entries[idx].message = e.to_string();
            let _ = queue.save(path);
            format!("failed {}: {e}", job.id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_form_has_official_effect() {
        let f = NewJobForm::default();
        assert!(EFFECTS.contains(&f.effect()));
        assert!(f.output.contains("idlescreen-"));
        assert!(f.output.ends_with(".mkv"));
    }

    #[test]
    fn cycle_effect_wraps() {
        let mut f = NewJobForm::default();
        for _ in 0..EFFECTS.len() + 2 {
            f.cycle_effect(1);
        }
        assert!(EFFECTS.contains(&f.effect()));
    }

    #[test]
    fn to_job_spec_parses() {
        let f = NewJobForm::default();
        let s = f.to_job_spec().expect("spec");
        assert_eq!(s.effect, "beams");
        assert_eq!(s.width, 1280);
        assert_eq!(s.fps, 30);
    }
}
