use super::*;
use idle_render::JobSpec;

fn spec() -> JobSpec {
    JobSpec {
        effect: "beams".into(),
        plugin_path: None,
        seed: 1,
        fps: 30,
        duration: "1s".into(),
        output: PathBuf::from("/tmp/x.mkv"),
        width: 64,
        height: 64,
        cols: None,
        rows: None,
        dry_run: true,
        raw: false,
        segment: None,
        audio: None,
        resume: false,
        crf: 35,
        preset: None,
        encoder: None,
        prefer_hw: true,
        gpu_upscale: true,
        format: None,
        container: None,
        baseline_dir: None,
        snapshot_last_only: false,
        update_baselines: false,
        cpu_raster: false,
    }
}

#[test]
fn roundtrip_queue() {
    let dir = std::env::temp_dir().join(format!("idle-studio-q-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("q.json");
    let mut q = JobQueue::default();
    q.enqueue(StudioJob::new("a".into(), spec()));
    q.save(&path).unwrap();
    let loaded = JobQueue::load(&path).unwrap();
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].status, JobStatus::Pending);
    assert_eq!(loaded.entries[0].job.spec.effect, "beams");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn next_id_skips_existing_after_delete() {
    let mut q = JobQueue::default();
    q.enqueue(StudioJob::new("job-1".into(), spec()));
    q.enqueue(StudioJob::new("job-2".into(), spec()));
    q.enqueue(StudioJob::new("job-3".into(), spec()));
    q.entries.remove(1); // delete job-2; len()+1 would reuse job-3
    assert_eq!(q.next_id(), "job-4");
}

#[test]
fn corrupt_queue_moves_aside_and_recovers() {
    let dir = std::env::temp_dir().join(format!("idle-studio-c-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("q.json");
    fs::write(&path, "{not json").unwrap();
    let q = JobQueue::load_or_recover(&path).unwrap();
    assert!(q.entries.is_empty());
    assert!(!path.exists());
    assert!(fs::read_dir(&dir)
        .unwrap()
        .any(|e| { e.unwrap().file_name().to_string_lossy().contains("corrupt") }));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn save_replaces_symlink_not_target() {
    let dir = std::env::temp_dir().join(format!("idle-studio-s-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let victim = dir.join("victim.json");
    fs::write(&victim, "sentinel").unwrap();
    let link = dir.join("q.json");
    std::os::unix::fs::symlink(&victim, &link).unwrap();
    let mut q = JobQueue::default();
    q.enqueue(StudioJob::new("a".into(), spec()));
    q.save(&link).unwrap();
    // The symlink itself is replaced; the victim is untouched.
    assert!(!link.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&victim).unwrap(), "sentinel");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_refuses_symlink() {
    let dir = std::env::temp_dir().join(format!("idle-studio-l-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let real = dir.join("real.json");
    fs::write(&real, "{}").unwrap();
    let link = dir.join("q.json");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(JobQueue::load(&link).is_err());
    let _ = fs::remove_dir_all(&dir);
}
