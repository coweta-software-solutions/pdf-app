<script lang="ts">
  import type { PageSelectionMode, PageSelectionValue } from './pageSelection';

  export let id = 'page-selection';
  export let label = 'Pages';
  export let value: PageSelectionValue = { mode: 'range', range: '1' };
  export let help = 'Use commas and ranges, for example 1-3, 5, 8-10.';

  function selectMode(mode: PageSelectionMode) {
    value = { ...value, mode };
  }

  function updateRange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    value = { mode: 'range', range: input.value };
  }
</script>

<div class="page-selection">
  <div class="field">
    <span>{label}</span>
    <div class="segmented-control" role="radiogroup" aria-label={label}>
      <button
        type="button"
        role="radio"
        aria-checked={value.mode === 'all'}
        on:click={() => selectMode('all')}
      >
        All pages
      </button>
      <button
        type="button"
        role="radio"
        aria-checked={value.mode === 'range'}
        on:click={() => selectMode('range')}
      >
        Range
      </button>
    </div>
  </div>

  {#if value.mode === 'range'}
    <label class="field page-range-field" for={`${id}-range`}>
      <span>Range</span>
      <input
        id={`${id}-range`}
        value={value.range}
        placeholder="1-3, 5, 8-10"
        aria-describedby={`${id}-help`}
        on:input={updateRange}
      />
      <small id={`${id}-help`}>{help}</small>
    </label>
  {/if}
</div>
