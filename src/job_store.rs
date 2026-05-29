use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use crate::FileDownload;

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
}

impl JobStore {
    pub fn create(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let mut records = self.records.lock().expect("job store lock poisoned");
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
        if let Some(job) = self
            .records
            .lock()
            .expect("job store lock poisoned")
            .get_mut(id)
        {
            job.status = "done".to_string();
            job.percent = 100;
            job.stage = "Ready to download".to_string();
            job.filename = Some(file.filename);
            job.content_type = Some(file.content_type.to_string());
            job.bytes = Some(file.bytes);
            job.error = None;
        }
    }

    pub fn fail(&self, id: &str, message: impl Into<String>) {
        if let Some(job) = self
            .records
            .lock()
            .expect("job store lock poisoned")
            .get_mut(id)
        {
            job.status = "error".to_string();
            job.percent = 100;
            job.stage = "Could not finish".to_string();
            job.error = Some(message.into());
        }
    }

    pub fn snapshot(&self, id: &str) -> Option<JobSnapshot> {
        let records = self.records.lock().expect("job store lock poisoned");
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
        let records = self.records.lock().expect("job store lock poisoned");
        let job = records.get(id)?;
        let bytes = job.bytes.clone()?;
        Some(FileDownload::new(
            job.content_type
                .clone()
                .unwrap_or_else(|| "application/octet-stream".to_string()),
            job.filename
                .clone()
                .unwrap_or_else(|| "download".to_string()),
            bytes,
        ))
    }
}
