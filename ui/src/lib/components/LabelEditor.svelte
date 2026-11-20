<script lang="ts">
  // Pencil button that opens a popover to rename a device and pick its color.
  import { scale } from "svelte/transition";
  import { ms } from "$lib/theme/motion";
  import type { Label } from "$lib/api/types";
  import IconButton from "./IconButton.svelte";

  const SWATCHES = ["#a855f7", "#6366f1", "#3b82f6", "#06b6d4", "#10b981", "#84cc16", "#eab308", "#f97316", "#ef4444", "#ec4899"];

  let {
    original,
    label,
    onsave,
  }: { original: string; label: Label | undefined; onsave: (label: Label) => void } = $props();

  let open = $state(false);
  let name = $state("");
  let color = $state<string | null>(null);

  function start() {
    name = label?.name ?? "";
    color = label?.color ?? null;
    open = true;
  }

  function save(e?: SubmitEvent) {
    e?.preventDefault();
    onsave({ ...label, name: name.trim() || null, color });
    open = false;
  }

  function reset() {
    onsave({ ...label, name: null, color: null });
    open = false;
  }
</script>

<svelte:window onkeydown={(e) => open && e.key === "Escape" && (open = false)} />

<div class="wrap">
  <IconButton icon="edit" label="Rename or recolor {original}" onclick={start} />
  {#if open}
    <button class="backdrop" aria-label="Close" onclick={() => (open = false)}></button>
    <form class="pop" onsubmit={save} transition:scale={{ start: 0.95, duration: ms("--dur-fast") }}>
      <label>
        <span>Name</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={name} placeholder={original} maxlength="40" autofocus />
      </label>
      <div class="hint">Shown as “{name.trim() || original}{name.trim() ? ` (${original})` : ""}”</div>
      <span class="caption">Color</span>
      <div class="swatches" role="radiogroup" aria-label="Color">
        <button
          type="button"
          class="swatch none"
          class:picked={!color}
          role="radio"
          aria-checked={!color}
          aria-label="Default color"
          title="Default"
          onclick={() => (color = null)}
        ></button>
        {#each SWATCHES as c (c)}
          <button
            type="button"
            class="swatch"
            class:picked={color === c}
            style:--c={c}
            role="radio"
            aria-checked={color === c}
            aria-label="Color {c}"
            onclick={() => (color = c)}
          ></button>
        {/each}
        <label class="swatch custom" title="Custom color" style:--c={color ?? "#888888"}>
          <input type="color" value={color ?? "#a855f7"} oninput={(e) => (color = e.currentTarget.value)} aria-label="Custom color" />
        </label>
      </div>
      <div class="actions">
        <button type="button" class="ghost" onclick={reset}>Reset</button>
        <button type="submit" class="primary">Save</button>
      </div>
    </form>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
    border: none;
    background: transparent;
    cursor: default;
  }
  .pop {
    position: absolute;
    z-index: 11;
    top: calc(100% + 6px);
    right: 0;
    width: 280px;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    background: rgba(24, 20, 40, 0.97);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-hover);
    transform-origin: top right;
  }
  label span,
  .caption {
    display: block;
    margin-bottom: 4px;
    font-size: var(--text-xs);
    color: var(--text-dim);
  }
  input:not([type="color"]) {
    width: 100%;
    padding: 7px 10px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    outline: none;
    background: rgba(0, 0, 0, 0.25);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    user-select: text;
  }
  input:not([type="color"]):focus {
    border-color: var(--accent);
  }
  .hint {
    font-size: var(--text-xs);
    color: var(--text-faint);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .swatch {
    position: relative;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 2px solid transparent;
    border-radius: 50%;
    background: var(--c);
    cursor: pointer;
    transition:
      transform var(--dur-fast) var(--ease-spring),
      border-color var(--dur-fast) var(--ease);
  }
  .swatch:hover {
    transform: scale(1.15);
  }
  .swatch.picked {
    border-color: #fff;
    box-shadow: 0 0 10px var(--c);
  }
  .none {
    background: var(--accent-grad);
  }
  .custom {
    background: conic-gradient(red, yellow, lime, cyan, blue, magenta, red);
    overflow: hidden;
  }
  .custom input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-1);
  }
  .actions button {
    padding: 6px 14px;
    border-radius: var(--radius-pill);
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .ghost {
    border: 1px solid var(--border-strong);
    background: transparent;
  }
  .primary {
    border: none;
    background: var(--accent-grad);
    color: #fff;
    font-weight: 600;
  }
</style>
