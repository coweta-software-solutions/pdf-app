export type JobStatus = Record<string, string>;

export type UploadProgress = {
  lengthComputable: boolean;
  loaded: number;
  total: number;
};

export function uploadJob(form: FormData, onProgress: (progress: UploadProgress) => void) {
  return new Promise<string>((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("POST", "/jobs");
    xhr.upload.onprogress = (event) => {
      onProgress({
        lengthComputable: event.lengthComputable,
        loaded: event.loaded,
        total: event.total,
      });
    };
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        resolve(xhr.responseText.trim());
      } else {
        reject(new Error(xhr.responseText || "Could not start job."));
      }
    };
    xhr.onerror = () => reject(new Error("Could not upload files."));
    xhr.send(form);
  });
}

export async function pollJob(
  jobId: string,
  onStatus: (status: JobStatus) => void,
  intervalMs = 500,
) {
  while (true) {
    const response = await fetch(`/jobs/${jobId}`, { cache: "no-store" });
    if (!response.ok) throw new Error(await response.text());
    const status = parseJobStatus(await response.text());
    onStatus(status);

    if (status.status === "done") return status;
    if (status.status === "error") throw new Error(status.error ?? status.stage ?? "Job failed.");
    await delay(intervalMs);
  }
}

export async function downloadJob(jobId: string) {
  const response = await fetch(`/jobs/${jobId}/download`);
  if (!response.ok) {
    throw new Error(await response.text());
  }

  return {
    blob: await response.blob(),
    filename: filenameFromDisposition(response.headers.get("content-disposition")),
  };
}

function parseJobStatus(value: string) {
  return Object.fromEntries(
    value
      .split("\n")
      .map((line) => line.split("="))
      .filter((parts) => parts.length >= 2)
      .map(([key, ...rest]) => [key, rest.join("=")]),
  ) as JobStatus;
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function filenameFromDisposition(value: string | null) {
  if (!value) return null;
  const match = /filename="([^"]+)"/.exec(value);
  return match?.[1] ?? null;
}
