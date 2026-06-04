<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import PageSelectionControl from '../PageSelectionControl.svelte';
  import type { JobProgress } from '../jobWorkflow';
  import type { ImageTarget, Operation, OperationDefinition } from '../operations';
  import type { PageSelectionValue } from '../pageSelection';
  import ProgressStatus from './ProgressStatus.svelte';

  export let operation: Operation;
  export let visibleOperations: OperationDefinition[] = [];
  export let targetImage: ImageTarget;
  export let extractPages: PageSelectionValue = { mode: 'all', range: '1' };
  export let exportPages: PageSelectionValue = { mode: 'all', range: '1' };
  export let busy = false;
  export let submitLabel = 'Download result';
  export let message = '';
  export let hasError = false;
  export let progress: JobProgress;

  const dispatch = createEventDispatcher<{
    submit: void;
    operationchange: Operation;
    targetimagechange: ImageTarget;
    extractpageschange: PageSelectionValue;
    exportpageschange: PageSelectionValue;
  }>();

  let previousExtractPages = extractPages;
  let previousExportPages = exportPages;

  $: if (extractPages !== previousExtractPages) {
    previousExtractPages = extractPages;
    dispatch('extractpageschange', extractPages);
  }

  $: if (exportPages !== previousExportPages) {
    previousExportPages = exportPages;
    dispatch('exportpageschange', exportPages);
  }
</script>

<form class="tool-form" on:submit|preventDefault={() => dispatch('submit')}>
  {#if visibleOperations.length > 1}
    <div class="action-list" role="group" aria-label="Choose action">
      {#each visibleOperations as item}
        <button
          type="button"
          class:active={operation === item.id}
          aria-pressed={operation === item.id}
          on:click={() => dispatch('operationchange', item.id)}
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
          on:click={() => dispatch('targetimagechange', 'png')}
        >
          PNG
        </button>
        <button
          type="button"
          role="radio"
          aria-checked={targetImage === 'jpeg'}
          on:click={() => dispatch('targetimagechange', 'jpeg')}
        >
          JPEG
        </button>
      </div>
    </div>
  {/if}

  <div class="submit-row">
    {#if busy}
      <ProgressStatus {progress} />
    {/if}
    <button class="primary-button" type="submit" disabled={busy}>
      {busy ? 'Processing...' : submitLabel}
    </button>
    {#if message && hasError}
      <p class="error" aria-live="polite">
        {message}
      </p>
    {/if}
  </div>
</form>
