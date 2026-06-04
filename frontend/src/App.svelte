<script lang="ts">
  import { onMount } from 'svelte';
  import EmptyUploadState from './components/EmptyUploadState.svelte';
  import FileQueue from './components/FileQueue.svelte';
  import ReadyHeader from './components/ReadyHeader.svelte';
  import ThemeToggle from './components/ThemeToggle.svelte';
  import ToolForm from './components/ToolForm.svelte';
  import { requestJobDownload, type JobProgress } from './jobWorkflow';
  import {
    buildJobForm,
    describeFileSet,
    fileSetDescription,
    fileSetLabel,
    normalizeFileSelection,
    preferredOperation,
    submitLabelFor,
    visibleOperationsFor,
    type ImageTarget,
    type Operation,
  } from './operations';
  import type { PageSelectionValue } from './pageSelection';
  import {
    applyTheme,
    initialTheme,
    nextTheme as getNextTheme,
    readStoredTheme,
    themeStorageKey,
    type Theme,
  } from './theme';

  type StatusKind = 'idle' | 'error';

  let busy = false;
  let dragActive = false;
  let theme: Theme = initialTheme();
  let userThemeSet = false;
  let message = '';
  let statusKind: StatusKind = 'idle';
  let files: File[] = [];
  let operation: Operation = 'pdf-image';
  let targetImage: ImageTarget = 'png';
  let extractPages: PageSelectionValue = { mode: 'all', range: '1' };
  let exportPages: PageSelectionValue = { mode: 'all', range: '1' };
  let filePickMode: 'replace' | 'append' = 'replace';
  let progress: JobProgress = { percent: 0, stage: '', detail: '' };

  $: nextTheme = getNextTheme(theme);
  $: fileSet = describeFileSet(files);
  $: visibleOperations = visibleOperationsFor(files);
  $: if (files.length && !visibleOperations.some((item) => item.id === operation)) {
    operation = preferredOperation(files);
  }
  $: submitLabel = submitLabelFor(operation, exportPages.mode);
  $: orderedFileSet = operation === 'merge' || operation === 'image-pdf';
  $: queueLabel = fileSetLabel(fileSet);
  $: queueDescription = fileSetDescription(operation);

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

  function setFiles(nextFiles: File[]) {
    resetStatus();
    if (!nextFiles.length) return;

    const result = normalizeFileSelection(nextFiles, operation);
    if ('error' in result) {
      fail(result.error);
      return;
    }

    files = result.files;
    operation = result.operation;
  }

  function removeFile(index: number) {
    files = files.filter((_, fileIndex) => fileIndex !== index);
    resetStatus();
  }

  function clearFiles() {
    files = [];
    resetStatus();
  }

  function moveFile(index: number, direction: -1 | 1) {
    const targetIndex = index + direction;
    if (targetIndex < 0 || targetIndex >= files.length) return;
    const nextFiles = [...files];
    const [file] = nextFiles.splice(index, 1);
    nextFiles.splice(targetIndex, 0, file);
    files = nextFiles;
    resetStatus();
  }

  function selectOperation(nextOperation: Operation) {
    operation = nextOperation;
    resetStatus();
  }

  async function submit() {
    resetStatus();
    progress = { percent: 0, stage: 'Preparing upload', detail: '' };
    if (!files.length) {
      fail('Upload a file first.');
      return;
    }

    busy = true;
    try {
      const form = buildJobForm({ operation, files, targetImage, extractPages, exportPages });
      await requestJobDownload(form, (nextProgress) => (progress = nextProgress));
      resetStatus({ keepProgress: true });
    } catch (error) {
      fail(error instanceof Error ? error.message : 'Request failed.');
    } finally {
      busy = false;
    }
  }

  function uploadInput() {
    return document.getElementById('file-input') as HTMLInputElement | null;
  }

  function resetStatus(options: { keepProgress?: boolean } = {}) {
    message = '';
    statusKind = 'idle';
    if (!options.keepProgress) {
      progress = { percent: 0, stage: '', detail: '' };
    }
  }

  function fail(value: string) {
    message = value;
    statusKind = 'error';
  }

</script>

<main class="shell" class:empty-shell={!files.length}>
  <div class="shell-actions">
    <ThemeToggle {theme} {nextTheme} on:toggle={toggleTheme} />
  </div>

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
    <EmptyUploadState
      {dragActive}
      {message}
      hasError={statusKind === 'error'}
      on:browse={() => browse()}
      on:dragactive={(event) => (dragActive = event.detail)}
      on:dropfiles={(event) => setFiles(event.detail)}
    />
  {:else}
    <section class="ready-shell" aria-label="PDF workspace">
      <section class="ready-card">
        <ReadyHeader
          {fileSet}
          {operation}
          on:append={() => browse('append')}
          on:replace={() => browse('replace')}
        />

        <div class="workspace-grid">
          <FileQueue
            {files}
            fileSetLabel={queueLabel}
            fileSetDescription={queueDescription}
            {orderedFileSet}
            {busy}
            on:clear={clearFiles}
            on:remove={(event) => removeFile(event.detail)}
            on:move={(event) => moveFile(event.detail.index, event.detail.direction)}
          />

          <ToolForm
            {operation}
            {visibleOperations}
            {targetImage}
            {extractPages}
            {exportPages}
            {busy}
            {submitLabel}
            {message}
            hasError={statusKind === 'error'}
            {progress}
            on:submit={submit}
            on:operationchange={(event) => selectOperation(event.detail)}
            on:targetimagechange={(event) => (targetImage = event.detail)}
            on:extractpageschange={(event) => (extractPages = event.detail)}
            on:exportpageschange={(event) => (exportPages = event.detail)}
          />
        </div>
      </section>
    </section>
  {/if}
</main>
