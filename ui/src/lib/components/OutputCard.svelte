<script lang="ts">
  // One output device: master fader, mute, rename/recolor, remove.
  import type { Column, Label } from "$lib/api/types";
  import Fader from "./Fader.svelte";
  import IconButton from "./IconButton.svelte";
  import Avatar from "./Avatar.svelte";
  import LabelEditor from "./LabelEditor.svelte";

  let {
    name,
    original,
    live,
    column,
    color,
    label,
    onvolume,
    onmute,
    onlabel,
    onremove,
  }: {
    /** "Custom (Original)" or just the original */
    name: string;
    original: string;
    live: boolean;
    column: Column;
    color?: string;
    label?: Label;
    onvolume: (gain: number) => void;
    onmute: (mute: boolean) => void;
    onlabel: (label: Label) => void;
    onremove: () => void;
  } = $props();
</script>

<div class="output" class:offline={!live} style:--c={color}>
  <div class="head">
    <Avatar name={original} icon="output" {color} size={30} dim={column.mute || !live} />
    <div class="title">
      <span class="name" title={column.name}>{name}</span>
      {#if !live}<span class="tag">Unplugged</span>{/if}
    </div>
    <LabelEditor {original} {label} onsave={onlabel} />
    <IconButton
      icon={column.mute ? "speaker-off" : "speaker"}
      label={column.mute ? `Unmute ${name}` : `Mute ${name}`}
      active={column.mute}
      pressed={column.mute}
      onclick={() => onmute(!column.mute)}
    />
    <IconButton icon="x" label="Remove output {name}" danger onclick={onremove} />
  </div>
  <Fader small gain={column.volume} muted={column.mute} {color} label="{name} output volume" onchange={onvolume} />
</div>

<style>
  .output {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    width: 100%;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    transition:
      border-color var(--dur) var(--ease),
      opacity var(--dur) var(--ease);
  }
  .output:hover {
    border-color: var(--c, var(--border-strong));
  }
  .offline {
    opacity: 0.55;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    margin-left: var(--space-1);
  }
  .name {
    font-size: var(--text-sm);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tag {
    font-size: var(--text-xs);
    color: var(--text-faint);
  }
</style>
