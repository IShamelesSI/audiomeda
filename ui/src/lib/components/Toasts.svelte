<script lang="ts">
  import { fly } from "svelte/transition";
  import { ms } from "$lib/theme/motion";

  let { toasts }: { toasts: { id: number; text: string }[] } = $props();
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast" transition:fly={{ y: 16, duration: ms("--dur") }}>{t.text}</div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: var(--space-5);
    bottom: var(--space-5);
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .toast {
    max-width: 360px;
    padding: var(--space-3) var(--space-4);
    background: rgba(24, 20, 40, 0.96);
    border: 1px solid var(--border-strong);
    border-left: 3px solid var(--danger);
    border-radius: var(--radius);
    box-shadow: var(--shadow-hover);
    font-size: var(--text-sm);
  }
</style>
