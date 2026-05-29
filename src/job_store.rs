use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use crate::FileDownload;

const TERMINAL_JOB_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_TERMINAL_JOBS: usize = 100;

#[derive(Clone)]
pub struct JobStore {
    records: Arc<Mutex<HashMap<String, JobRecord>>>,
    next_id: Arc<AtomicU64>,
}

impl Default for JobStore {
    fn default() -> Self {
        Self {
            records: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }
}

#[derive(Clone)]
pub struct JobSnapshot {
    pub status: String,
    pub percent: u8,
    pub stage: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub error: Option<String>,
}

struct JobRecord {
    status: String,
    percent: u8,
    stage: String,
    filename: Option<String>,
    content_type: Option<String>,
    bytes: Option<Vec<u8>>,
    error: Option<String>,
    terminal_at: Option<Instant>,
}

impl JobStore {
    pub fn create(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let now = Instant::now();
        let mut records = self.records.lock().expect("job store lock poisoned");
        prune_records(&mut records, now);
        records.insert(
            id.clone(),
            JobRecord {
                status: "queued".to_string(),
                percent: 0,
                stage: "Waiting to start".to_string(),
                filename: None,
                content_type: None,
                bytes: None,
                error: None,
                terminal_at: None,
            },
        );
        id
    }

    pub fn update(&self, id: &str, percent: u8, stage: impl Into<String>) {
        if let Some(job) = self
            .records
            .lock()
            .expect("job store lock poisoned")
            .get_mut(id)
        {
            job.status = "running".to_string();
            job.percent = percent.min(99);
            job.stage = stage.into();
        }
    }

    pub fn complete(&self, id: &str, file: FileDownload) {
        let now = Instant::now();
        let mut records = self.records.lock().expect("job store lock poisoned");
        if let Some(job) = records.get_mut(id) {
            job.status = "done".to_string();
            job.percent = 100;
            job.stage = "Ready to download".to_string();
            job.filename = Some(file.filename);
            job.content_type = Some(file.content_type);
            job.bytes = Some(file.bytes);
            job.error = None;
            job.terminal_at = Some(now);
        }
        prune_records(&mut records, now);
    }

    pub fn fail(&self, id: &str, message: impl Into<String>) {
        let now = Instant::now();
        let mut records = self.records.lock().expect("job store lock poisoned");
        if let Some(job) = records.get_mut(id) {
            job.status = "error".to_string();
            job.percent = 100;
            job.stage = "Could not finish".to_string();
            job.error = Some(message.into());
            job.bytes = None;
            job.terminal_at = Some(now);
        }
        prune_records(&mut records, now);
    }

    pub fn snapshot(&self, id: &str) -> Option<JobSnapshot> {
        let mut records = self.records.lock().expect("job store lock poisoned");
        prune_records(&mut records, Instant::now());
        let job = records.get(id)?;
        Some(JobSnapshot {
            status: job.status.clone(),
            percent: job.percent,
            stage: job.stage.clone(),
            filename: job.filename.clone(),
            content_type: job.content_type.clone(),
            error: job.error.clone(),
        })
    }

    pub fn download(&self, id: &str) -> Option<FileDownload> {
        let mut records = self.records.lock().expect("job store lock poisoned");
        prune_records(&mut records, Instant::now());
        let job = records.remove(id)?;
        let bytes = job.bytes?;
        Some(FileDownload::new(
            job.content_type
                .unwrap_or_else(|| "application/octet-stream".to_string()),
            job.filename.unwrap_or_else(|| "download".to_string()),
            bytes,
        ))
    }
}

fn prune_records(records: &mut HashMap<String, JobRecord>, now: Instant) {
    records.retain(|_, job| {
        job.terminal_at
            .map(|terminal_at| now.duration_since(terminal_at) < TERMINAL_JOB_TTL)
            .unwrap_or(true)
    });

    let terminal_count = records
        .values()
        .filter(|job| job.terminal_at.is_some())
        .count();
    let overflow = terminal_count.saturating_sub(MAX_TERMINAL_JOBS);
    if overflow == 0 {
        return;
    }

    let mut terminal_jobs = records
        .iter()
        .filter_map(|(id, job)| job.terminal_at.map(|terminal_at| (id.clone(), terminal_at)))
        .collect::<Vec<_>>();
    terminal_jobs.sort_by_key(|(_, terminal_at)| *terminal_at);

    for (id, _) in terminal_jobs.into_iter().take(overflow) {
        records.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn download(bytes: &[u8]) -> FileDownload {
        FileDownload::new("application/pdf", "result.pdf", bytes.to_vec())
    }

    #[test]
    fn download_consumes_job_record() {
        let store = JobStore::default();
        let id = store.create();
        store.complete(&id, download(b"pdf"));

        assert_eq!(store.download(&id).unwrap().bytes, b"pdf");
        assert!(store.snapshot(&id).is_none());
        assert!(store.download(&id).is_none());
    }

    #[test]
    fn terminal_job_count_is_bounded() {
        let store = JobStore::default();
        let first_id = store.create();
        store.complete(&first_id, download(b"first"));

        for _ in 0..MAX_TERMINAL_JOBS {
            let id = store.create();
            store.complete(&id, download(b"next"));
        }

        assert!(store.snapshot(&first_id).is_none());
    }
}
