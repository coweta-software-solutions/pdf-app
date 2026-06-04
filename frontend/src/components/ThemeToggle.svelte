<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { Theme } from '../theme';

  export let theme: Theme;
  export let nextTheme: Theme;

  const dispatch = createEventDispatcher<{ toggle: void }>();
</script>

<button
  class="theme-toggle"
  type="button"
  role="switch"
  aria-checked={theme === 'dark'}
  aria-label={`Switch to ${nextTheme} mode`}
  on:click={() => dispatch('toggle')}
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
