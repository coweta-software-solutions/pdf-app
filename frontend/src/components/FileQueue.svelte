<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let files: File[] = [];
  export let fileSetLabel = 'PDF queue';
  export let fileSetDescription = '';
  export let orderedFileSet = false;
  export let busy = false;

  const dispatch = createEventDispatcher<{
    clear: void;
    remove: number;
    move: { index: number; direction: -1 | 1 };
  }>();

  $: totalBytes = files.reduce((sum, file) => sum + file.size, 0);

  function formatBytes(bytes: number): string {
    return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
  }
</script>

<section class="file-queue" aria-label={fileSetLabel}>
  <div class="file-queue-head">
    <div>
      <h3>{fileSetLabel}</h3>
      <p>{files.length} files &middot; {formatBytes(totalBytes)} &middot; {fileSetDescription}</p>
    </div>
    <button class="text-button" type="button" on:click={() => dispatch('clear')}>Clear</button>
  </div>

  <div class="file-summary" role="region" aria-label={`${files.length} selected files`}>
    {#each files as file, index}
      <div class="file-row">
        <span class="file-index">{index + 1}</span>
        <div class="file-meta">
          <span title={file.name}>{file.name}</span>
          <small>{formatBytes(file.size)}</small>
        </div>
        {#if orderedFileSet && files.length > 1}
          <div class="file-order-controls" aria-label={`Move ${file.name}`}>
            <button
              class="icon-button"
              type="button"
              disabled={index === 0 || busy}
              aria-label={`Move ${file.name} earlier`}
              on:click={() => dispatch('move', { index, direction: -1 })}
            >
              Up
            </button>
            <button
              class="icon-button"
              type="button"
              disabled={index === files.length - 1 || busy}
              aria-label={`Move ${file.name} later`}
              on:click={() => dispatch('move', { index, direction: 1 })}
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
          on:click={() => dispatch('remove', index)}
        >
          x
        </button>
      </div>
    {/each}
  </div>
</section>
