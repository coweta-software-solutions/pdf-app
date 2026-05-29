<script lang="ts">
  import { onMount } from 'svelte';
  import { downloadJob, pollJob, uploadJob } from './api';
  import PageSelectionControl from './PageSelectionControl.svelte';
  import type { PageSelectionValue } from './pageSelection';

  type Operation = 'split' | 'pdf-image' | 'merge' | 'image-pdf';
  type StatusKind = 'idle' | 'success' | 'error';
  type Theme = 'light' | 'dark';
  type ImageTarget = 'png' | 'jpeg';

  const themeStorageKey = 'pdf-tools-theme';
  let busy = false;
  let dragActive = false;
  let theme: Theme = getInitialTheme();
  let userThemeSet = false;
  let message = '';
  let statusKind: StatusKind = 'idle';
  let progressPercent = 0;
  let progressStage = '';
  let progressDetail = '';
  let files: File[] = [];
  let operation: Operation = 'pdf-image';
  let targetImage: ImageTarget = 'png';
  let extractPages: PageSelectionValue = { mode: 'all', range: '1' };
  let exportPages: PageSelectionValue = { mode: 'all', range: '1' };
  let filePickMode: 'replace' | 'append' = 'replace';

  const operations: Array<{ id: Operation; label: string; description: string }> = [
    { id: 'pdf-image', label: 'Export images', description: 'Render PDF pages as PNG or JPEG files.' },
    { id: 'split', label: 'Extract pages', description: 'Save selected pages as a new PDF.' },
    { id: 'merge', label: 'Merge PDFs', description: 'Combine PDFs in the order shown.' },
    { id: 'image-pdf', label: 'Create PDF', description: 'Turn images into PDF pages.' },
  ];

  $: nextTheme = theme === 'dark' ? 'light' : 'dark';

  onMount(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const storedTheme = readStoredTheme();

    if (storedTheme) {
      userThemeSet = true;
      theme = storedTheme;
      applyTheme(theme);
    } else {
      syncSystemTheme(media);
    }

    const onSystemThemeChange = () => syncSystemTheme(media);
    media.addEventListener('change', onSystemThemeChange);

    return () => media.removeEventListener('change', onSystemThemeChange);
  });

  $: primaryFile = files[0] ?? null;
  $: allPdfs = files.length > 0 && files.every(isPdfFile);
  $: allImages = files.length > 0 && files.every(isImageFile);
  $: hasSinglePdf = files.length === 1 && primaryFile ? isPdfFile(primaryFile) : false;
  $: hasSingleImage = files.length === 1 && primaryFile ? isImageFile(primaryFile) : false;
  $: hasMergeSet = files.length > 1 && allPdfs;
  $: hasImageSet = files.length > 1 && allImages;
  $: visibleOperations = visibleOperationsFor(files);
  $: if (files.length && !visibleOperations.some((item) => item.id === operation)) {
    operation = preferredOperation();
  }
  $: submitLabel = operation === 'split'
    ? 'Download extracted PDF'
    : operation === 'pdf-image'
      ? exportPages.mode === 'all'
        ? 'Download all images'
        : 'Download images'
      : operation === 'image-pdf'
        ? 'Create PDF'
      : 'Download result';
  $: totalBytes = files.reduce((sum, file) => sum + file.size, 0);
  $: orderedFileSet = operation === 'merge' || operation === 'image-pdf';
  $: fileSetLabel = allImages ? 'Image queue' : 'PDF queue';
  $: fileSetDescription = orderedFileSet
    ? 'Processed in the order shown.'
    : 'Each PDF is processed separately.';

  function isPdfFile(file: File) {
    return file.type === 'application/pdf' || file.name.toLowerCase().endsWith('.pdf');
  }

  function isImageFile(file: File) {
    return ['image/png', 'image/jpeg'].includes(file.type) || /\.(png|jpe?g)$/i.test(file.name);
  }

  function preferredOperation(): Operation {
    if (allImages) return 'image-pdf';
    return 'pdf-image';
  }

  function visibleOperationsFor(nextFiles: File[]) {
    const nextAllPdfs = nextFiles.length > 0 && nextFiles.every(isPdfFile);
    const nextAllImages = nextFiles.length > 0 && nextFiles.every(isImageFile);
    if (nextFiles.length > 1 && nextAllPdfs) {
      return operations.filter((item) => item.id === 'pdf-image' || item.id === 'merge');
    }
    if (nextFiles.length === 1 && nextAllPdfs) {
      return operations.filter((item) => item.id === 'split' || item.id === 'pdf-image');
    }
    if (nextAllImages) {
      return operations.filter((item) => item.id === 'image-pdf');
    }
    return [];
  }

  function getInitialTheme(): Theme {
    return document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light';
  }

  function readStoredTheme(): Theme | null {
    const value = localStorage.getItem(themeStorageKey);
    return value === 'light' || value === 'dark' ? value : null;
  }

  function applyTheme(nextTheme: Theme) {
    document.documentElement.dataset.theme = nextTheme;
  }

  function syncSystemTheme(media: MediaQueryList) {
    if (userThemeSet) return;
    theme = media.matches ? 'dark' : 'light';
    applyTheme(theme);
  }

  function toggleTheme() {
    theme = nextTheme;
    userThemeSet = true;
    localStorage.setItem(themeStorageKey, theme);
    applyTheme(theme);
  }

  function browse(mode: 'replace' | 'append' = 'replace') {
    filePickMode = mode;
    const input = uploadInput();
    if (!input) return;
    input.value = '';
    input.click();
  }

  function onFiles(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const selectedFiles = Array.from(input.files ?? []);
    setFiles(filePickMode === 'append' ? [...files, ...selectedFiles] : selectedFiles);
    filePickMode = 'replace';
  }

  function onDrop(event: DragEvent) {
    event.preventDefault();
    dragActive = false;
    setFiles(Array.from(event.dataTransfer?.files ?? []));
  }

  function setFiles(nextFiles: File[]) {
    message = '';
    statusKind = 'idle';
    progressPercent = 0;
    progressStage = '';
    progressDetail = '';
    if (!nextFiles.length) return;

    const supported = nextFiles.filter((file) => isPdfFile(file) || isImageFile(file));
    if (supported.length !== nextFiles.length) {
      fail('Upload a PDF, PNG, or JPEG file.');
      return;
    }

    if (supported.length > 1 && !supported.every(isPdfFile) && !supported.every(isImageFile)) {
      fail('Choose PDFs or images, not both in the same operation.');
      return;
    }

    const previousOperation = operation;
    const nextOperations = visibleOperationsFor(supported);
    files = supported;
    operation = nextOperations.some((item) => item.id === previousOperation)
      ? previousOperation
      : isImageFile(supported[0])
        ? 'image-pdf'
        : 'pdf-image';
    message =
      supported.length > 1
        ? isImageFile(supported[0])
          ? `${supported.length} images ready to convert.`
          : `${supported.length} PDFs ready.`
        : isImageFile(supported[0])
          ? 'Ready to create PDF.'
          : `${supported[0].name} is ready.`;
    statusKind = 'success';
  }

  function removeFile(index: number) {
    const nextFiles = files.filter((_, fileIndex) => fileIndex !== index);
    const nextFilesAreImages = nextFiles.length > 0 && nextFiles.every(isImageFile);
    files = nextFiles;
    message = nextFiles.length ? `${nextFiles.length} ${nextFilesAreImages ? 'images' : 'PDFs'} selected.` : '';
    statusKind = nextFiles.length ? 'success' : 'idle';
  }

  function clearFiles() {
    files = [];
    message = '';
    statusKind = 'idle';
    progressPercent = 0;
    progressStage = '';
    progressDetail = '';
  }

  function moveFile(index: number, direction: -1 | 1) {
    const targetIndex = index + direction;
    if (targetIndex < 0 || targetIndex >= files.length) return;
    const nextFiles = [...files];
    const [file] = nextFiles.splice(index, 1);
    nextFiles.splice(targetIndex, 0, file);
    files = nextFiles;
    message = '';
    statusKind = 'idle';
  }

  function uploadInput() {
    return document.getElementById('file-input') as HTMLInputElement | null;
  }

  function fail(value: string) {
    message = value;
    statusKind = 'error';
  }

  async function submit() {
    message = '';
    statusKind = 'idle';
    progressPercent = 0;
    progressStage = 'Preparing upload';
    progressDetail = '';
    if (!files.length) {
      fail('Upload a file first.');
      return;
    }

    busy = true;
    try {
      await requestJobDownload(buildJobForm());
    } catch (error) {
      fail(error instanceof Error ? error.message : 'Request failed.');
    } finally {
      busy = false;
    }
  }

  function buildJobForm() {
    if (operation === 'merge') {
      if (!hasMergeSet) throw new Error('Choose two or more PDFs to merge.');
      const form = new FormData();
      form.append('action', 'merge');
      for (const file of files) form.append('files', file);
      return form;
    }

    if (operation === 'split') {
      if (!hasSinglePdf) throw new Error('Choose one PDF to extract pages.');
      const form = new FormData();
      form.append('action', 'split');
      form.append('file', files[0]);
      form.append('pages', pageSelectionExpression(extractPages));
      return form;
    }

    const form = new FormData();
    form.append('action', 'convert');

    if (operation === 'pdf-image') {
      if (!allPdfs) throw new Error('Choose one or more PDFs to export pages as images.');
      for (const file of files) form.append('files', file);
      form.append('target', targetImage);
      form.append('pages', pageSelectionExpression(exportPages));
    } else {
      if (!allImages) throw new Error('Choose one or more PNG or JPEG images to create a PDF.');
      for (const file of files) form.append('files', file);
      form.append('target', 'pdf');
      form.append('layout', 'single');
    }

    return form;
  }

  function pageSelectionExpression(value: PageSelectionValue) {
    return value.mode === 'all' ? 'all' : value.range;
  }

  async function requestJobDownload(form: FormData) {
    const jobId = await uploadJob(form, (event) => {
      if (!event.lengthComputable) {
        progressPercent = 10;
        progressStage = 'Uploading files';
        return;
      }
      const uploadPercent = Math.round((event.loaded / event.total) * 20);
      progressPercent = Math.min(25, 5 + uploadPercent);
      progressStage = 'Uploading files';
      progressDetail = `${formatBytes(event.loaded)} of ${formatBytes(event.total)}`;
    });
    progressPercent = Math.max(progressPercent, 25);
    progressStage = 'Processing on server';
    progressDetail = `Job ${jobId}`;

    const finalStatus = await pollJob(jobId, (status) => {
      progressPercent = Math.max(progressPercent, Number(status.percent ?? 0));
      progressStage = status.stage ?? 'Processing on server';
      progressDetail = status.filename ? `Preparing ${status.filename}` : `Job ${jobId}`;
    });
    const { blob, filename: downloadFilename } = await downloadJob(jobId);
    const filename = downloadFilename ?? finalStatus.filename ?? 'download';
    const link = document.createElement('a');
    link.href = URL.createObjectURL(blob);
    link.download = filename;
    link.click();
    URL.revokeObjectURL(link.href);
    progressPercent = 100;
    progressStage = 'Download ready';
    progressDetail = filename;
    message = `Downloaded ${filename}.`;
    statusKind = 'success';
  }

  function formatSize(file: File) {
    return formatBytes(file.size);
  }

  function formatBytes(bytes: number) {
    return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
  }
</script>

<main class="shell" class:empty-shell={!files.length}>
  <button
    class="theme-toggle"
    type="button"
    role="switch"
    aria-checked={theme === 'dark'}
    aria-label={`Switch to ${nextTheme} mode`}
    on:click={toggleTheme}
  >
    <span class="theme-toggle-track" aria-hidden="true">
      <span class="theme-toggle-thumb">
        <svg class="theme-svg theme-svg-sun" viewBox="0 0 24 24">
          <path
            d="M12 3v2.25m6.36.39-1.59 1.59M21 12h-2.25m-.39 6.36-1.59-1.59M12 18.75V21m-4.77-4.23-1.59 1.59M5.25 12H3m4.23-4.77L5.64 5.64M15.75 12a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0Z"
          />
        </svg>
        <svg class="theme-svg theme-svg-moon" viewBox="0 0 24 24">
          <path
            d="M21.75 15A9.72 9.72 0 0 1 18 15.75 9.75 9.75 0 0 1 8.25 6c0-1.33.27-2.6.75-3.75A9.75 9.75 0 0 0 3 11.25 9.75 9.75 0 0 0 12.75 21a9.75 9.75 0 0 0 9-6Z"
          />
        </svg>
      </span>
    </span>
  </button>

  <input
    id="file-input"
    class="file-input"
    type="file"
    accept="application/pdf,image/png,image/jpeg"
    multiple
    aria-label="Upload a PDF or image"
    on:change={onFiles}
  />

  {#if !files.length}
    <section
      class:drag-active={dragActive}
      class="empty-state"
      aria-label="Upload files"
      on:dragenter|preventDefault={() => (dragActive = true)}
      on:dragover|preventDefault={() => (dragActive = true)}
      on:dragleave|preventDefault={() => (dragActive = false)}
      on:drop={onDrop}
    >
      <div class="upload-panel">
        <div class="upload-target" aria-hidden="true">
          <span></span>
          <span></span>
          <span></span>
        </div>
        <button class="primary-button" type="button" on:click={() => browse()}>Choose file</button>
        <p>PDF, PNG, or JPEG</p>
        {#if message}
          <p class:success={statusKind === 'success'} class:error={statusKind === 'error'} aria-live="polite">{message}</p>
        {/if}
      </div>
    </section>
  {:else}
    <section class="ready-shell" aria-label="PDF workspace">
      <section class="ready-card">
        <div class="ready-head">
          <div class="ready-document" aria-hidden="true">
            <span></span>
            <span></span>
            <span></span>
          </div>
          <div class="ready-title">
            <p class="eyebrow">{hasMergeSet ? 'PDF set' : allImages ? 'Images to PDF' : 'PDF source'}</p>
            <h2>
              {hasMergeSet ? `${files.length} PDFs selected` : hasImageSet ? `${files.length} images selected` : primaryFile?.name}
            </h2>
            <p>
              {hasMergeSet
                ? operation === 'merge'
                  ? 'Files will merge in the order shown.'
                  : 'Each PDF can export pages as images.'
                : allImages
                  ? 'Each image becomes a PDF page at its original size.'
                  : `${primaryFile ? formatSize(primaryFile) : ''} selected.`}
            </p>
          </div>
          <div class="ready-actions">
            {#if allPdfs || allImages}
              <button class="ghost-button" type="button" on:click={() => browse('append')}>Add files</button>
            {/if}
            <button class="ghost-button" type="button" on:click={() => browse('replace')}>
              {hasMergeSet || hasImageSet ? 'Change files' : 'Change file'}
            </button>
          </div>
        </div>

        <div class="workspace-grid">
          <section class="file-queue" aria-label={fileSetLabel}>
            <div class="file-queue-head">
              <div>
                <h3>{fileSetLabel}</h3>
                <p>{files.length} files &middot; {formatBytes(totalBytes)} &middot; {fileSetDescription}</p>
              </div>
              <button class="text-button" type="button" on:click={clearFiles}>Clear</button>
            </div>

            <div class="file-summary" role="region" aria-label={`${files.length} selected files`}>
              {#each files as file, index}
                <div class="file-row">
                  <span class="file-index">{index + 1}</span>
                  <div class="file-meta">
                    <span title={file.name}>{file.name}</span>
                    <small>{formatSize(file)}</small>
                  </div>
                  {#if orderedFileSet && files.length > 1}
                    <div class="file-order-controls" aria-label={`Move ${file.name}`}>
                      <button
                        class="icon-button"
                        type="button"
                        disabled={index === 0 || busy}
                        aria-label={`Move ${file.name} earlier`}
                        on:click={() => moveFile(index, -1)}
                      >
                        Up
                      </button>
                      <button
                        class="icon-button"
                        type="button"
                        disabled={index === files.length - 1 || busy}
                        aria-label={`Move ${file.name} later`}
                        on:click={() => moveFile(index, 1)}
                      >
                        Down
                      </button>
                    </div>
                  {/if}
                  <button
                    class="icon-button remove-file"
                    type="button"
                    disabled={busy}
                    aria-label={`Remove ${file.name}`}
                    on:click={() => removeFile(index)}
                  >
                    x
                  </button>
                </div>
              {/each}
            </div>
          </section>

          <form class="tool-form" on:submit|preventDefault={submit}>
            {#if visibleOperations.length > 1}
              <div class="action-list" role="group" aria-label="Choose action">
                {#each visibleOperations as item}
                  <button
                    type="button"
                    class:active={operation === item.id}
                    aria-pressed={operation === item.id}
                    on:click={() => {
                      operation = item.id;
                      message = '';
                      statusKind = 'idle';
                    }}
                  >
                    <span>{item.label}</span>
                    <small>{item.description}</small>
                  </button>
                {/each}
              </div>
            {/if}

            {#if operation === 'split'}
              <PageSelectionControl id="extract-pages" label="Pages to extract" bind:value={extractPages} />
            {/if}

            {#if operation === 'pdf-image'}
              <PageSelectionControl id="export-pages" label="Pages to export" bind:value={exportPages} />
            {/if}

            {#if operation === 'pdf-image'}
              <div class="field">
                <span>Image format</span>
                <div class="segmented-control" role="radiogroup" aria-label="Image format">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={targetImage === 'png'}
                    on:click={() => (targetImage = 'png')}
                  >
                    PNG
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={targetImage === 'jpeg'}
                    on:click={() => (targetImage = 'jpeg')}
                  >
                    JPEG
                  </button>
                </div>
              </div>
            {/if}

            <div class="submit-row">
              {#if busy}
                <div class="progress-status" aria-live="polite">
                  <div>
                    <span>{progressStage}</span>
                    <strong>{progressPercent}%</strong>
                  </div>
                  <progress max="100" value={progressPercent}></progress>
                  {#if progressDetail}<small>{progressDetail}</small>{/if}
                </div>
              {/if}
              <button class="primary-button" type="submit" disabled={busy}>
                {busy ? 'Processing...' : submitLabel}
              </button>
              {#if message}
                <p class:success={statusKind === 'success'} class:error={statusKind === 'error'} aria-live="polite">
                  {message}
                </p>
              {/if}
            </div>
          </form>
        </div>
      </section>
    </section>
  {/if}
</main>
