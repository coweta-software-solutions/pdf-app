use crate::ProgressCallback;

pub(crate) fn report(progress: &Option<ProgressCallback>, percent: u8, stage: impl Into<String>) {
    if let Some(progress) = progress {
        progress(percent, stage.into());
    }
}
