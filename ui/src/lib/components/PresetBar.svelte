<script lang="ts">
  // Saved presets as chips (click to load, × to delete) plus a field to save the current mix.
  import { scale } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { ms } from "$lib/theme/motion";
  import Icon from "./Icon.svelte";

  let {
    presets,
    onload,
    ondelete,
    onsave,
  }: {
    presets: string[];
    onload: (name: string) => void;
    ondelete: (name: string) => void;
    onsave: (name: string) => void;
  } = $props();

  let name = $state("");

  function save(e: SubmitEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    onsave(name.trim());
    name = "";
  }
</script>

<div class="bar">
  {#each presets as p (p)}
    <span class="chip" animate:flip={{ duration: ms("--dur") }} transition:scale={{ duration: ms("--dur") }}>
      <button class="load" title="Load preset {p}" onclick={() => onload(p)}>{p}</button>
      <button class="del" aria-label="Delete preset {p}" title="Delete preset {p}" onclick={() => ondelete(p)}>
        <Icon name="x" size={12} />
      </button>
    </span>
  {/each}
  <form onsubmit={save}>
    <input bind:value={name} placeholder="Save mix as…" aria-label="New preset name" maxlength="40" />
    <button type="submit" aria-label="Save preset" title="Save preset" disabled={!name.trim()}>
      <Icon name="save" size={14} />
    </button>
  </form>
</div>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: var(--surface);
    overflow: hidden;
    transition: border-color var(--dur-fast) var(--ease);
  }
  .chip:hover {
    border-color: var(--accent);
  }
  .chip button {
    border: none;
    background: transparent;
    cursor: pointer;
  }
  .load {
    padding: 6px 4px 6px 12px;
    font-size: var(--text-sm);
  }
  .load:active {
    color: var(--accent-1);
  }
  .del {
    display: grid;
    place-items: center;
    padding: 6px 8px 6px 4px;
    color: var(--text-faint);
  }
  .del:hover {
    color: var(--danger);
  }
  form {
    display: flex;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: rgba(0, 0, 0, 0.2);
    transition: border-color var(--dur-fast) var(--ease);
  }
  form:focus-within {
    border-color: var(--accent);
  }
  input {
    width: 130px;
    padding: 6px 4px 6px 12px;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    user-select: text;
  }
  input::placeholder {
    color: var(--text-faint);
  }
  form button {
    display: grid;
    place-items: center;
    padding: 6px 10px 6px 6px;
    border: none;
    background: transparent;
    color: var(--accent);
    cursor: pointer;
  }
  form button:disabled {
    color: var(--text-faint);
    cursor: default;
  }
</style>
