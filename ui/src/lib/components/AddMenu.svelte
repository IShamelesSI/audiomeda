<script lang="ts">
  // "+ Add …" button (or any icon + label) with a popover list of choices.
  import { scale } from "svelte/transition";
  import { ms } from "$lib/theme/motion";
  import type { ComponentProps } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    label,
    choices,
    onpick,
    icon = "plus",
    title,
    empty = "Nothing else to add",
  }: {
    label: string;
    choices: { id: string; name: string }[];
    onpick: (id: string) => void;
    icon?: ComponentProps<typeof Icon>["name"];
    title?: string;
    empty?: string;
  } = $props();

  let open = $state(false);

  function pick(id: string) {
    open = false;
    onpick(id);
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (open = false)} />

<div class="wrap">
  <button class="add" aria-expanded={open} {title} onclick={() => (open = !open)}>
    <Icon name={icon} size={14} />{label}
  </button>
  {#if open}
    <!-- click outside closes -->
    <button class="backdrop" aria-label="Close" onclick={() => (open = false)}></button>
    <ul class="menu" transition:scale={{ start: 0.95, duration: ms("--dur-fast") }}>
      {#each choices as c (c.id)}
        <li><button onclick={() => pick(c.id)}>{c.name}</button></li>
      {:else}
        <li class="empty">{empty}</li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }
  .add {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-dim);
    font-size: var(--text-sm);
    cursor: pointer;
    transition:
      color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease),
      background var(--dur-fast) var(--ease);
  }
  .add:hover {
    color: var(--text);
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
    border: none;
    background: transparent;
    cursor: default;
  }
  .menu {
    position: absolute;
    z-index: 11;
    top: calc(100% + 6px);
    left: 0;
    min-width: 260px;
    max-height: 320px;
    overflow-y: auto;
    margin: 0;
    padding: var(--space-1);
    list-style: none;
    background: rgba(24, 20, 40, 0.96);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-hover);
    transform-origin: top left;
  }
  .menu button {
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--surface-hover);
  }
  .empty {
    padding: 8px 10px;
    color: var(--text-faint);
    font-size: var(--text-sm);
  }
</style>
