use crate::queue::{JobQueue, JobStatus};
use crate::tui::form::{FormField, NewJobForm, FORM_FIELDS};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

pub fn draw_queue(
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
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
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
        Span::raw(
            "n new  j/k move  Enter run  r next  a all  d del  p re-pend  R reload  q quit  | ",
        ),
        Span::raw(status_line),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}

pub fn draw_new_job(area: Rect, f: &mut ratatui::Frame<'_>, form: &NewJobForm, status_line: &str) {
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
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
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
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Parameters")),
        chunks[1],
    );

    f.render_widget(
        Paragraph::new(status_line).block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );

    // Dim overlay hint when empty area (no-op Clear keeps layout stable)
    let _ = Clear;
}
