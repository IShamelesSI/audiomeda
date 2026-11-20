<script lang="ts">
  import "$lib/theme/tokens.css";
  import "$lib/theme/global.css";

  let { children } = $props();
</script>

<!-- a rendering bug shows a message instead of an empty window -->
<svelte:boundary>
  {@render children()}

  {#snippet failed(error, reset)}
    <div class="crash">
      <h1>Something broke in the mixer window</h1>
      <pre>{error instanceof Error ? error.message : String(error)}</pre>
      <button onclick={reset}>Try again</button>
      <p>The engine keeps mixing; only this window is affected.</p>
    </div>
  {/snippet}
</svelte:boundary>

<style>
  .crash {
    max-width: 640px;
    margin: 15vh auto 0;
    padding: var(--space-5);
    background: var(--surface);
    border: 1px solid var(--danger);
    border-radius: var(--radius-lg);
  }
  h1 {
    margin: 0 0 var(--space-3);
    font-size: var(--text-lg);
  }
  pre {
    white-space: pre-wrap;
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.3);
    font-size: var(--text-sm);
    user-select: text;
  }
  button {
    padding: 7px 16px;
    border: none;
    border-radius: var(--radius-pill);
    background: var(--accent-grad);
    color: #fff;
    cursor: pointer;
  }
  p {
    color: var(--text-dim);
    font-size: var(--text-sm);
  }
</style>
