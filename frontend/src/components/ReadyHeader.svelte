<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { FileSet, Operation } from '../operations';

  export let fileSet: FileSet = {
    files: [],
    primaryFile: null,
    allPdfs: false,
    allImages: false,
    hasSinglePdf: false,
    hasMergeSet: false,
    hasImageSet: false,
  };
  export let operation: Operation = 'pdf-image';

  const dispatch = createEventDispatcher<{
    append: void;
    replace: void;
  }>();

  $: eyebrow = fileSet.hasMergeSet ? 'PDF set' : fileSet.allImages ? 'Images to PDF' : 'PDF source';
  $: title = fileSet.hasMergeSet
    ? `${fileSet.files.length} PDFs selected`
    : fileSet.hasImageSet
      ? `${fileSet.files.length} images selected`
      : fileSet.primaryFile?.name;
  $: description = fileSet.hasMergeSet
    ? operation === 'merge'
      ? 'Files will merge in the order shown.'
      : 'Each PDF can export pages as images.'
    : fileSet.allImages
      ? 'Each image becomes a PDF page at its original size.'
      : `${fileSet.primaryFile ? formatBytes(fileSet.primaryFile.size) : ''} selected.`;

  function formatBytes(bytes: number) {
    return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
  }
</script>

<div class="ready-head">
  <div class="ready-document" aria-hidden="true">
    <span></span>
    <span></span>
    <span></span>
  </div>
  <div class="ready-title">
    <p class="eyebrow">{eyebrow}</p>
    <h2>{title}</h2>
    <p>{description}</p>
  </div>
  <div class="ready-actions">
    {#if fileSet.allPdfs || fileSet.allImages}
      <button class="ghost-button" type="button" on:click={() => dispatch('append')}>Add files</button>
    {/if}
    <button class="ghost-button" type="button" on:click={() => dispatch('replace')}>
      {fileSet.hasMergeSet || fileSet.hasImageSet ? 'Change files' : 'Change file'}
    </button>
  </div>
</div>
