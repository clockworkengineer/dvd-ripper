/**
 * @file queue.rs
 * @brief Thread-safe priority job queue manager for multi-disc appliance ripping.
 */

use std::sync::{Arc, Mutex, OnceLock};
use serde::{Deserialize, Serialize};

/// Represents a single queued ripping job item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobItem {
    pub id: String,
    pub title: String,
    pub media_type: String,
    pub drive: String,
    pub status: String,
    pub timestamp: String,
}

static JOB_QUEUE: OnceLock<Arc<Mutex<Vec<JobItem>>>> = OnceLock::new();

/// Returns a reference to the global thread-safe job queue instance.
pub fn get_job_queue_handle() -> &'static Arc<Mutex<Vec<JobItem>>> {
    JOB_QUEUE.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
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
        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    let handle = get_job_queue_handle();
    if let Ok(mut queue) = handle.lock() {
        queue.push(item);
    }
    id
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
        queue.len() < initial_len
    } else {
        false
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
}
