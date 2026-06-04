use std::sync::Arc;

use pdfium_render::prelude::Pdfium;
use tokio::sync::Semaphore;

use crate::{
    error::{AppError, AppResult},
    job_store::JobStore,
};

#[derive(Clone)]
pub struct AppState {
    cpu: Arc<Semaphore>,
    pdfium: Arc<Pdfium>,
    max_render_pages: usize,
    pub jobs: JobStore,
}

pub type ProgressCallback = Arc<dyn Fn(u8, String) + Send + Sync>;

impl AppState {
    pub fn from_env() -> AppResult<Self> {
        let host_parallelism = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let default_permits = host_parallelism.clamp(1, 4);
        let cpu_permits = read_env_usize("PDF_TOOLS_CPU_PERMITS", default_permits).max(1);
        let max_render_pages = read_env_usize("MAX_RENDER_PAGES", 100).max(1);
        let pdfium = Arc::new(bind_pdfium()?);

        Ok(Self {
            cpu: Arc::new(Semaphore::new(cpu_permits)),
            pdfium,
            max_render_pages,
            jobs: JobStore::default(),
        })
    }

    pub async fn run_cpu<T, F>(&self, work: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce() -> AppResult<T> + Send + 'static,
    {
        let permit = self.cpu.clone().acquire_owned().await?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await
        .map_err(AppError::from)?
    }

    pub fn max_render_pages(&self) -> usize {
        self.max_render_pages
    }

    pub fn pdfium(&self) -> Arc<Pdfium> {
        self.pdfium.clone()
    }

    #[doc(hidden)]
    pub fn for_tests(pdfium: Arc<Pdfium>) -> Self {
        Self {
            cpu: Arc::new(Semaphore::new(1)),
            pdfium,
            max_render_pages: 100,
            jobs: JobStore::default(),
        }
    }
}

pub fn read_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

pub fn read_env_u16(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn bind_pdfium() -> AppResult<Pdfium> {
    let bindings = match std::env::var("PDF_TOOLS_PDFIUM_PATH") {
        Ok(path) if !path.trim().is_empty() => Pdfium::bind_to_library(path.trim())
            .map_err(|err| AppError::external_tool(format!("could not load PDFium: {err}")))?,
        _ => Pdfium::bind_to_system_library()
            .map_err(|err| AppError::external_tool(format!("could not load PDFium: {err}")))?,
    };

    Ok(Pdfium::new(bindings))
}

#[cfg(test)]
pub(crate) fn test_pdfium() -> Option<Arc<Pdfium>> {
    use std::sync::OnceLock;

    static PDFIUM: OnceLock<Option<Arc<Pdfium>>> = OnceLock::new();
    PDFIUM
        .get_or_init(|| {
            let path = std::env::var("PDF_TOOLS_PDFIUM_PATH").ok()?;
            if path.trim().is_empty() {
                return None;
            }
            Pdfium::bind_to_library(path.trim())
                .ok()
                .map(Pdfium::new)
                .map(Arc::new)
        })
        .clone()
}
