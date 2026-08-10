use idle_render::JobSpec;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const EFFECTS: &[&str] = &[
    "beams", "bursts", "chaos", "cosmos", "glyphs", "gnats", "hearth", "radar", "ripple",
    "storm",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Queue,
    NewJob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Effect,
    Duration,
    Width,
    Height,
    Fps,
    Crf,
    Output,
    DryRun,
}

pub const FORM_FIELDS: &[FormField] = &[
    FormField::Effect,
    FormField::Duration,
    FormField::Width,
    FormField::Height,
    FormField::Fps,
    FormField::Crf,
    FormField::Output,
    FormField::DryRun,
];

pub struct NewJobForm {
    pub field_i: usize,
    pub effect_i: usize,
    pub duration: String,
    pub width: String,
    pub height: String,
    pub fps: String,
    pub crf: String,
    pub output: String,
    pub dry_run: bool,
    /// When true, typing edits the focused text field.
    pub editing: bool,
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
    pub fn field(&self) -> FormField {
        FORM_FIELDS[self.field_i % FORM_FIELDS.len()]
    }

    pub fn effect(&self) -> &'static str {
        EFFECTS[self.effect_i % EFFECTS.len()]
    }

    pub fn cycle_effect(&mut self, dir: i32) {
        let n = EFFECTS.len() as i32;
        let i = self.effect_i as i32 + dir;
        self.effect_i = ((i % n) + n) as usize % EFFECTS.len();
        // Refresh default output stem when effect changes (if still on pattern).
        self.output = default_output_path(self.effect());
    }

    pub fn next_field(&mut self, dir: i32) {
        let n = FORM_FIELDS.len() as i32;
        let i = self.field_i as i32 + dir;
        self.field_i = ((i % n) + n) as usize % FORM_FIELDS.len();
        self.editing = false;
    }

    pub fn text_mut(&mut self) -> Option<&mut String> {
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

    pub fn to_job_spec(&self) -> Result<JobSpec, String> {
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
            format: None,
            container: None,
            baseline_dir: None,
            snapshot_last_only: false,
            update_baselines: false,
            cpu_raster: false,
        })
    }
}

pub fn default_output_path(effect: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = std::env::var("HOME")
        .map(|h| format!("{h}/Videos"))
        .unwrap_or_else(|_| "/tmp".into());
    format!("{dir}/idlescreen-{effect}-{ts}.mkv")
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
