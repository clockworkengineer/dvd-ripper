use serde::{Deserialize, Serialize};
/**
 * @file queue.rs
 * @brief Thread-safe priority job queue manager for multi-disc appliance ripping.
 */
use std::sync::{Arc, Mutex, OnceLock};

use std::path::PathBuf;

/// Represents a single queued ripping job item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobItem {
    pub id: String,
    pub title: String,
    pub media_type: String,
    pub drive: String,
    pub status: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spool_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub priority: u8,
}

/// Represents a tracked TV series season box set state across multi-disc insertions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxSetRecord {
    pub show_name: String,
    pub season: u32,
    pub last_episode: u32,
    pub total_discs_ripped: u32,
    pub updated_at: String,
}

/// Formats a clean human-readable job queue status summary (e.g. "Queue status: 3 total queued (1 active)").
#[allow(dead_code)]
pub fn format_queue_summary(active_count: usize, total_queued: usize) -> String {
    if total_queued == 0 && active_count == 0 {
        "Job queue is empty".to_string()
    } else {
        format!(
            "Queue status: {} total queued ({} active)",
            total_queued, active_count
        )
    }
}

static JOB_QUEUE: OnceLock<Arc<Mutex<Vec<JobItem>>>> = OnceLock::new();
static BOXSET_MANAGER: OnceLock<Arc<Mutex<Vec<BoxSetRecord>>>> = OnceLock::new();

/// Returns a reference to the global thread-safe job queue instance.
pub fn get_job_queue_handle() -> &'static Arc<Mutex<Vec<JobItem>>> {
    JOB_QUEUE.get_or_init(|| Arc::new(Mutex::new(load_job_queue())))
}

/// Returns a reference to the global thread-safe box set manager instance.
pub fn get_boxset_manager_handle() -> &'static Arc<Mutex<Vec<BoxSetRecord>>> {
    BOXSET_MANAGER.get_or_init(|| Arc::new(Mutex::new(load_boxsets(None))))
}

fn resolve_boxsets_path(custom_path: Option<&str>) -> PathBuf {
    if let Some(p) = custom_path {
        return PathBuf::from(p);
    }
    crate::utils::get_app_file_path("boxsets.json")
}

/// Loads persistent box set records from ~/.dvd-ripper/boxsets.json
pub fn load_boxsets(custom_path: Option<&str>) -> Vec<BoxSetRecord> {
    let p = resolve_boxsets_path(custom_path);
    crate::utils::load_json_file(&p).unwrap_or_default()
}

/// Saves box set records to ~/.dvd-ripper/boxsets.json using atomic file writing.
pub fn save_boxsets(records: &[BoxSetRecord], custom_path: Option<&str>) -> anyhow::Result<()> {
    let p = resolve_boxsets_path(custom_path);
    crate::utils::save_json_file(&p, &records)
}

/// Trait defining job queue repository contract (ISP/DIP).
#[allow(dead_code)]
pub trait JobQueueRepository {
    fn name(&self) -> &str;
    fn add_job(&self, title: &str, media_type: &str, drive: &str) -> String;
    fn list_jobs(&self) -> Vec<JobItem>;
    fn remove_job(&self, id: &str) -> bool;
}

/// Resolves the absolute path to job_queue.json in the user's application data directory.
fn resolve_queue_path() -> PathBuf {
    crate::utils::get_app_file_path("job_queue.json")
}

/// Loads persistent queued ripping job items from job_queue.json.
pub fn load_job_queue() -> Vec<JobItem> {
    let p = resolve_queue_path();
    crate::utils::load_json_file(&p).unwrap_or_default()
}

/// Saves queued ripping job items to job_queue.json using atomic JSON persistence.
pub fn save_job_queue(jobs: &[JobItem]) -> anyhow::Result<()> {
    let p = resolve_queue_path();
    crate::utils::save_json_file(&p, &jobs)
}

/// Concrete file-persistent job queue repository implementation (SOLID/DIP).
#[derive(Debug, Default)]
pub struct FileJobQueueRepository;

impl JobQueueRepository for FileJobQueueRepository {
    fn name(&self) -> &str {
        "File-Persistent Job Queue Repository"
    }

    fn add_job(&self, title: &str, media_type: &str, drive: &str) -> String {
        add_job(title, media_type, drive)
    }

    fn list_jobs(&self) -> Vec<JobItem> {
        list_jobs()
    }

    fn remove_job(&self, id: &str) -> bool {
        remove_job(id)
    }
}

/// Enqueues a new ripping job item into the queue.
pub fn add_job(title: &str, media_type: &str, drive: &str) -> String {
    let id = format!("job_{}", uuid_v4_short());
    let item = JobItem {
        id: id.clone(),
        title: title.to_string(),
        media_type: media_type.to_string(),
        drive: drive.to_string(),
        status: "Queued".to_string(),
        timestamp: crate::utils::now_timestamp_str(),
        spool_path: None,
        output_path: None,
        progress: 0.0,
        priority: 1,
    };
    let handle = get_job_queue_handle();
    if let Ok(mut queue) = handle.lock() {
        queue.push(item);
        let _ = save_job_queue(&queue);
    }
    id
}

/// Updates the status and progress percentage (0.0..=1.0) of a queued job.
pub fn update_job_status(id: &str, status: &str, progress: f64) -> bool {
    let handle = get_job_queue_handle();
    if let Ok(mut queue) = handle.lock()
        && let Some(job) = queue.iter_mut().find(|j| j.id == id)
    {
        job.status = status.to_string();
        job.progress = progress.clamp(0.0, 1.0);
        let _ = save_job_queue(&queue);
        return true;
    }
    false
}

/// Moves a queued job one slot higher in priority order.
pub fn move_job_up(id: &str) -> bool {
    let handle = get_job_queue_handle();
    if let Ok(mut queue) = handle.lock()
        && let Some(idx) = queue.iter().position(|j| j.id == id)
        && idx > 0
    {
        queue.swap(idx, idx - 1);
        let _ = save_job_queue(&queue);
        return true;
    }
    false
}

/// Moves a queued job one slot lower in priority order.
pub fn move_job_down(id: &str) -> bool {
    let handle = get_job_queue_handle();
    if let Ok(mut queue) = handle.lock()
        && let Some(idx) = queue.iter().position(|j| j.id == id)
        && idx + 1 < queue.len()
    {
        queue.swap(idx, idx + 1);
        let _ = save_job_queue(&queue);
        return true;
    }
    false
}

/// Spools a disc's `VIDEO_TS` structure (or creates a spooled ISO image) into `<spool_root>/.spool/<job_id>`
/// so the physical optical drive can be immediately ejected while background CPU/GPU workers encode the spooled media.
pub fn spool_disc_for_job(
    job_id: &str,
    source_drive: &str,
    spool_root: &std::path::Path,
) -> anyhow::Result<PathBuf> {
    let spool_dir = spool_root.join(".spool").join(job_id);
    std::fs::create_dir_all(&spool_dir)?;
    let spool_iso = spool_dir.join("spooled_disc.iso");

    update_job_status(job_id, "Spooling Optical Disc...", 0.05);
    let _ = crate::iso::dump_disc_to_iso(source_drive, &spool_iso, Some(job_id), None, None)?;

    let handle = get_job_queue_handle();
    if let Ok(mut queue) = handle.lock()
        && let Some(job) = queue.iter_mut().find(|j| j.id == job_id)
    {
        job.spool_path = Some(spool_iso.to_string_lossy().to_string());
        job.status = "Spooled (Ready to Encode)".to_string();
        job.progress = 0.5;
        let _ = save_job_queue(&queue);
    }

    Ok(spool_iso)
}

/// Returns a clone of all current job items in the queue.
pub fn list_jobs() -> Vec<JobItem> {
    let handle = get_job_queue_handle();
    if let Ok(queue) = handle.lock() {
        queue.clone()
    } else {
        Vec::new()
    }
}

/// Removes a job item from the queue by ID.
pub fn remove_job(id: &str) -> bool {
    let handle = get_job_queue_handle();
    if let Ok(mut queue) = handle.lock() {
        let initial_len = queue.len();
        queue.retain(|j| j.id != id);
        let removed = queue.len() < initial_len;
        if removed {
            let _ = save_job_queue(&queue);
        }
        removed
    } else {
        false
    }
}

/// Clears all job items from the queue.
#[allow(dead_code)]
pub fn clear_queue() {
    let handle = get_job_queue_handle();
    if let Ok(mut queue) = handle.lock() {
        queue.clear();
        let _ = save_job_queue(&queue);
    }
}

/// Selects the next job ready for background encoding, prioritizing `"Spooled (Ready to Encode)"` over `"Queued"`.
pub fn next_ready_job() -> Option<JobItem> {
    let handle = get_job_queue_handle();
    if let Ok(queue) = handle.lock() {
        if let Some(spooled) = queue
            .iter()
            .find(|j| j.status == "Spooled (Ready to Encode)")
        {
            return Some(spooled.clone());
        }
        if let Some(queued) = queue.iter().find(|j| j.status == "Queued") {
            return Some(queued.clone());
        }
    }
    None
}

/// Cleans up temporary `.spool/<job_id>` files once background encoding completes.
pub fn cleanup_job_spool(job_id: &str, spool_root: &std::path::Path) -> bool {
    let spool_dir = spool_root.join(".spool").join(job_id);
    let mut removed_fs = false;
    if spool_dir.exists() {
        removed_fs = std::fs::remove_dir_all(&spool_dir).is_ok();
    }
    let handle = get_job_queue_handle();
    if let Ok(mut queue) = handle.lock()
        && let Some(job) = queue.iter_mut().find(|j| j.id == job_id)
    {
        job.spool_path = None;
        let _ = save_job_queue(&queue);
    }
    removed_fs
}

fn normalize_show_key(show_name: &str) -> String {
    crate::utils::sanitize_filename(show_name).to_lowercase()
}

/// Returns the next episode number to start with for a given show and season.
pub fn get_next_boxset_episode(show_name: &str, season: u32) -> u32 {
    let handle = get_boxset_manager_handle();
    if let Ok(records) = handle.lock() {
        let clean_show = normalize_show_key(show_name);
        for r in records.iter() {
            if normalize_show_key(&r.show_name) == clean_show && r.season == season {
                return r.last_episode + 1;
            }
        }
    }
    1
}

/// Records that a disc with `episode_count` episodes was ripped for the given show and season.
pub fn record_boxset_episodes_ripped(show_name: &str, season: u32, episode_count: u32) -> u32 {
    let handle = get_boxset_manager_handle();
    let mut new_last = episode_count;
    if let Ok(mut records) = handle.lock() {
        let clean_show = normalize_show_key(show_name);
        let now = crate::utils::now_timestamp_str();
        let mut found = false;

        for r in records.iter_mut() {
            if normalize_show_key(&r.show_name) == clean_show && r.season == season {
                r.last_episode += episode_count;
                r.total_discs_ripped += 1;
                r.updated_at = now.clone();
                new_last = r.last_episode;
                found = true;
                break;
            }
        }

        if !found {
            records.push(BoxSetRecord {
                show_name: show_name.to_string(),
                season,
                last_episode: episode_count,
                total_discs_ripped: 1,
                updated_at: now,
            });
        }

        let _ = save_boxsets(&records, None);
    }
    new_last
}

/// Resets the box set tracking for a given show and season.
pub fn reset_boxset_tracker(show_name: &str, season: u32) -> bool {
    let handle = get_boxset_manager_handle();
    if let Ok(mut records) = handle.lock() {
        let clean_show = normalize_show_key(show_name);
        let initial_len = records.len();
        records.retain(|r| !(normalize_show_key(&r.show_name) == clean_show && r.season == season));
        let removed = records.len() < initial_len;
        if removed {
            let _ = save_boxsets(&records, None);
        }
        removed
    } else {
        false
    }
}

/// Returns a copy of all tracked box set records.
pub fn list_boxsets() -> Vec<BoxSetRecord> {
    let handle = get_boxset_manager_handle();
    if let Ok(records) = handle.lock() {
        records.clone()
    } else {
        Vec::new()
    }
}

/// Liskov Substitution Principle (LSP/SOLID): Trait contract for box set episode progress tracking.
#[allow(dead_code)]
pub trait BoxSetEpisodeTracker {
    fn get_next_episode(&self, show_name: &str, season: u32) -> u32;
    fn record_episodes(&self, show_name: &str, season: u32, count: usize) -> u32;
    fn reset_show(&self, show_name: &str, season: u32) -> bool;
}

#[derive(Debug, Default)]
pub struct FileBoxSetEpisodeTracker;

impl BoxSetEpisodeTracker for FileBoxSetEpisodeTracker {
    fn get_next_episode(&self, show_name: &str, season: u32) -> u32 {
        get_next_boxset_episode(show_name, season)
    }

    fn record_episodes(&self, show_name: &str, season: u32, count: usize) -> u32 {
        record_boxset_episodes_ripped(show_name, season, count as u32)
    }

    fn reset_show(&self, show_name: &str, season: u32) -> bool {
        reset_boxset_tracker(show_name, season)
    }
}

/// Liskov Substitution Principle (LSP/SOLID): Trait contract for job queue storage.
#[allow(dead_code)]
pub trait JobQueueStore {
    fn enqueue(&self, title: &str, media_type: &str, input_drive: &str) -> String;
    fn list(&self) -> Vec<JobItem>;
    fn remove(&self, id: &str) -> bool;
}

#[derive(Debug, Default)]
pub struct MemoryJobQueueStore;

impl JobQueueStore for MemoryJobQueueStore {
    fn enqueue(&self, title: &str, media_type: &str, input_drive: &str) -> String {
        add_job(title, media_type, input_drive)
    }

    fn list(&self) -> Vec<JobItem> {
        list_jobs()
    }

    fn remove(&self, id: &str) -> bool {
        remove_job(id)
    }
}

fn uuid_v4_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(123456);
    format!("{:08x}", nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_queue_enqueue_list_remove() {
        let id1 = add_job("Aliens", "Movie", "D:\\");
        assert!(id1.starts_with("job_"));

        let jobs = list_jobs();
        assert!(!jobs.is_empty());
        assert!(jobs.iter().any(|j| j.id == id1));

        let removed = remove_job(&id1);
        assert!(removed);
    }

    #[test]
    fn test_boxset_manager_tracking_and_reset() {
        let show = "The Office Test";
        let season = 1;

        let next_1 = get_next_boxset_episode(show, season);
        assert_eq!(next_1, 1);

        let new_last_1 = record_boxset_episodes_ripped(show, season, 4);
        assert_eq!(new_last_1, 4);

        let next_2 = get_next_boxset_episode(show, season);
        assert_eq!(next_2, 5);

        let new_last_2 = record_boxset_episodes_ripped(show, season, 4);
        assert_eq!(new_last_2, 8);

        let boxsets = list_boxsets();
        assert!(
            boxsets
                .iter()
                .any(|b| b.show_name == show && b.last_episode == 8)
        );

        let reset = reset_boxset_tracker(show, season);
        assert!(reset);

        let next_after_reset = get_next_boxset_episode(show, season);
        assert_eq!(next_after_reset, 1);
    }

    #[test]
    fn test_format_queue_summary() {
        assert_eq!(format_queue_summary(0, 0), "Job queue is empty");
        assert_eq!(
            format_queue_summary(1, 3),
            "Queue status: 3 total queued (1 active)"
        );
    }

    #[test]
    fn test_queue_traits_and_clear_and_file_persistence() {
        // 1. Clear queue
        add_job("Movie A", "Movie", "D:\\");
        clear_queue();
        assert_eq!(list_jobs().len(), 0);

        // 2. MemoryJobQueueStore
        let mem_store = MemoryJobQueueStore;
        let j_id = mem_store.enqueue("Title 1", "Movie", "D:\\");
        assert_eq!(mem_store.list().len(), 1);
        assert!(mem_store.remove(&j_id));
        assert_eq!(mem_store.list().len(), 0);

        // 3. FileBoxSetEpisodeTracker
        let tracker = FileBoxSetEpisodeTracker;
        let ep = tracker.get_next_episode("Firefly Special", 1);
        assert_eq!(ep, 1);
        let recorded = tracker.record_episodes("Firefly Special", 1, 3);
        assert_eq!(recorded, 3);
        assert!(tracker.reset_show("Firefly Special", 1));

        // 4. Custom path save and load boxsets
        let temp = std::env::temp_dir().join(format!("test_boxsets_{}.json", uuid_v4_short()));
        let records = vec![BoxSetRecord {
            show_name: "Custom Show".to_string(),
            season: 2,
            last_episode: 5,
            total_discs_ripped: 1,
            updated_at: "2026-01-01".to_string(),
        }];
        let save_res = save_boxsets(&records, Some(&temp.to_string_lossy()));
        assert!(save_res.is_ok());
        let loaded = load_boxsets(Some(&temp.to_string_lossy()));
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].show_name, "Custom Show");
        let _ = std::fs::remove_file(&temp);

        // 5. Handles
        assert!(get_job_queue_handle().lock().is_ok());
        assert!(get_boxset_manager_handle().lock().is_ok());

        // 6. Priority ordering & status update
        clear_queue();
        let id_first = add_job("First Disc", "Movie", "D:\\");
        let id_second = add_job("Second Disc", "TV Series", "E:\\");
        assert!(update_job_status(
            &id_second,
            "Spooled (Ready to Encode)",
            0.5
        ));
        assert!(move_job_up(&id_second));
        let reordered = list_jobs();
        assert_eq!(reordered[0].id, id_second);
        assert_eq!(reordered[0].progress, 0.5);
        assert!(move_job_down(&id_second));
        let restored = list_jobs();
        assert_eq!(restored[0].id, id_first);

        // 7. next_ready_job prioritizes spooled jobs & cleanup_job_spool cleans up
        let next = next_ready_job().unwrap();
        assert_eq!(next.id, id_second);
        let spool_root = std::env::temp_dir().join(format!("spool_test_{}", uuid_v4_short()));
        let job_spool_dir = spool_root.join(".spool").join(&id_second);
        std::fs::create_dir_all(&job_spool_dir).unwrap();
        assert!(cleanup_job_spool(&id_second, &spool_root));
        assert!(!job_spool_dir.exists());
        let _ = std::fs::remove_dir_all(&spool_root);

        clear_queue();
    }
}
