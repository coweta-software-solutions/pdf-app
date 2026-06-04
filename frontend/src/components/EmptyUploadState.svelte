<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let dragActive = false;
  export let message = '';
  export let hasError = false;

  const dispatch = createEventDispatcher<{
    browse: void;
    dragactive: boolean;
    dropfiles: File[];
  }>();

  function dropFiles(event: DragEvent) {
    dispatch('dragactive', false);
    dispatch('dropfiles', Array.from(event.dataTransfer?.files ?? []));
  }
</script>

<section
  class:drag-active={dragActive}
  class="empty-state"
  aria-label="Upload files"
  on:dragenter|preventDefault={() => dispatch('dragactive', true)}
  on:dragover|preventDefault={() => dispatch('dragactive', true)}
  on:dragleave|preventDefault={() => dispatch('dragactive', false)}
  on:drop|preventDefault={dropFiles}
>
  <div class="upload-panel">
    <div class="upload-target" aria-hidden="true">
      <span></span>
      <span></span>
      <span></span>
    </div>
    <button class="primary-button" type="button" on:click={() => dispatch('browse')}>Choose file</button>
    <p>PDF, PNG, or JPEG</p>
    {#if message && hasError}
      <p class="error" aria-live="polite">{message}</p>
    {/if}
  </div>
</section>
