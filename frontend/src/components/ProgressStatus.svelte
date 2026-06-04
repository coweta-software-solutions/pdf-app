<script lang="ts">
  import type { JobProgress } from '../jobWorkflow';

  export let progress: JobProgress;
</script>

<div class="progress-status" aria-live="polite">
  <div>
    <span>{progress.stage}</span>
    <strong>{progress.percent}%</strong>
  </div>
  <progress max="100" value={progress.percent}></progress>
  {#if progress.detail}<small>{progress.detail}</small>{/if}
</div>
