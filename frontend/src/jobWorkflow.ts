import { downloadJob, pollJob, uploadJob } from "./api";

export type JobProgress = {
  percent: number;
  stage: string;
  detail: string;
};

export async function requestJobDownload(
  form: FormData,
  onProgress: (progress: JobProgress) => void,
): Promise<string> {
  let percent = 0;

  const updateProgress = (progress: JobProgress) => {
    percent = progress.percent;
    onProgress(progress);
  };

  const jobId = await uploadJob(form, (event) => {
    if (!event.lengthComputable) {
      updateProgress({ percent: 10, stage: "Uploading files", detail: "" });
      return;
    }

    const uploadPercent = Math.round((event.loaded / event.total) * 20);
    updateProgress({
      percent: Math.min(25, 5 + uploadPercent),
      stage: "Uploading files",
      detail: `${formatBytes(event.loaded)} of ${formatBytes(event.total)}`,
    });
  });

  updateProgress({
    percent: Math.max(percent, 25),
    stage: "Processing on server",
    detail: `Job ${jobId}`,
  });

  const finalStatus = await pollJob(jobId, (status) => {
    updateProgress({
      percent: Math.max(percent, Number(status.percent ?? 0)),
      stage: status.stage ?? "Processing on server",
      detail: status.filename ? `Preparing ${status.filename}` : `Job ${jobId}`,
    });
  });

  const { blob, filename: downloadFilename } = await downloadJob(jobId);
  const filename = downloadFilename ?? finalStatus.filename ?? "download";
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = filename;
  link.click();
  URL.revokeObjectURL(link.href);

  updateProgress({ percent: 100, stage: "Download ready", detail: filename });
  return filename;
}

function formatBytes(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}
