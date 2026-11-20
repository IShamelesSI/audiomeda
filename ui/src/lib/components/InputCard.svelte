<script lang="ts">
  // One app or mic: master fader + mute, and a send to every output.
  import type { Label, Row } from "$lib/api/types";
  import Fader from "./Fader.svelte";
  import IconButton from "./IconButton.svelte";
  import Icon from "./Icon.svelte";
  import Avatar from "./Avatar.svelte";
  import LabelEditor from "./LabelEditor.svelte";

  export interface OutputInfo {
    id: string;
    name: string;
    color?: string;
    virtual: boolean;
  }

  let {
    name,
    kind,
    status,
    row,
    outputs,
    iconSrc = null,
    color,
    pinned = false,
    original = name,
    label,
    onvolume,
    onmute,
    oncell,
    oncellon,
    onreset,
    onpin,
    onlabel,
    onremove,
  }: {
    name: string;
    kind: "app" | "mic";
    /** playing = making sound, open = has a window, closed = not running / unplugged */
    status: "playing" | "open" | "closed";
    row: Row;
    outputs: OutputInfo[];
    iconSrc?: string | null;
    color?: string;
    pinned?: boolean;
    original?: string;
    label?: Label;
    onvolume: (gain: number) => void;
    onmute: (mute: boolean) => void;
    oncell: (output: string, gain: number) => void;
    oncellon: (output: string, on: boolean) => void;
    onreset: () => void;
    onpin?: (pinned: boolean) => void;
    onlabel?: (label: Label) => void;
    onremove?: () => void;
  } = $props();

  const statusText = $derived(
    status === "closed"
      ? kind === "mic" ? "Unplugged" : "Not running"
      : row.mute ? "Muted" : status === "playing" ? (kind === "mic" ? "Live" : "Playing") : "Open",
  );
  const physical = $derived(outputs.filter((o) => !o.virtual));
  const virtual = $derived(outputs.filter((o) => o.virtual));
</script>

{#snippet send(out: OutputInfo)}
  {@const cell = row.cells[out.id]}
  {@const on = cell?.on ?? false}
  <div class="send" class:off={!on}>
    <button
      class="chip"
      class:on
      style:--c={out.color}
      aria-pressed={on}
      title={on ? `Stop sending ${name} to ${out.name}` : `Send ${name} to ${out.name}`}
      onclick={() => oncellon(out.id, !on)}
    >
      <span class="led"></span>{out.name}
    </button>
    <Fader
      small
      gain={cell?.volume ?? 0}
      muted={!on || row.mute}
      color={out.color}
      label="{name} on {out.name}"
      onchange={(g) => oncell(out.id, g)}
    />
  </div>
{/snippet}

<article class="card {status}" class:muted={row.mute} style:--c={color}>
  <header>
    <Avatar {name} src={iconSrc} icon={kind === "mic" ? "mic" : undefined} {color} dim={row.mute || status === "closed"} />
    <div class="title">
      <h3 title={name}>{name}</h3>
      <span class="status"><span class="dot"></span>{statusText}</span>
    </div>
    {#if onpin}
      <IconButton
        icon="pin"
        label={pinned ? `Unpin ${name}` : `Pin ${name} so it stays when closed`}
        pressed={pinned}
        on={pinned}
        onclick={() => onpin(!pinned)}
      />
    {/if}
    {#if onlabel}
      <LabelEditor {original} {label} onsave={onlabel} />
    {/if}
    <IconButton
      icon={row.mute ? "speaker-off" : "speaker"}
      label={row.mute ? `Unmute ${name}` : `Mute ${name}`}
      active={row.mute}
      pressed={row.mute}
      onclick={() => onmute(!row.mute)}
    />
  </header>

  <Fader gain={row.volume} muted={row.mute} {color} label="{name} volume" onchange={onvolume} />

  {#if outputs.length}
    <div class="sends">
      {#each physical as out (out.id)}{@render send(out)}{/each}
      {#if physical.length && virtual.length}<div class="divider">Virtual</div>{/if}
      {#each virtual as out (out.id)}{@render send(out)}{/each}
    </div>
  {/if}

  <footer>
    <button class="link" onclick={onreset} title="Back to the default route for new apps">
      <Icon name="reset" size={13} /> Reset
    </button>
    {#if onremove}
      <button class="link danger" onclick={onremove}><Icon name="x" size={13} /> Remove</button>
    {/if}
  </footer>
</article>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-4) var(--space-3);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow);
    transition:
      transform var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      border-color var(--dur) var(--ease),
      opacity var(--dur) var(--ease);
  }
  .card:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow-hover);
    border-color: var(--c, var(--border-strong));
  }
  .closed {
    opacity: 0.55;
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .title {
    flex: 1;
    min-width: 0;
    margin-left: var(--space-1);
  }
  h3 {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-md);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-xs);
    color: var(--text-dim);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent-2);
    opacity: 0.7;
  }
  .playing .dot {
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
    opacity: 1;
  }
  .muted .dot {
    background: var(--danger);
    box-shadow: 0 0 8px var(--danger);
    opacity: 1;
  }
  .closed .dot {
    background: var(--off);
    box-shadow: none;
  }
  .sends {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding-top: var(--space-3);
    border-top: 1px solid var(--border);
  }
  .divider {
    margin-top: var(--space-1);
    font-size: var(--text-xs);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .send {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    align-items: center;
    gap: var(--space-3);
  }
  .chip {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    padding: 4px 10px 4px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-dim);
    font-size: var(--text-sm);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease);
  }
  .chip:hover {
    background: var(--surface-hover);
    color: var(--text);
    border-color: var(--border-strong);
  }
  .chip.on {
    color: var(--text);
    border-color: var(--border-strong);
    background: var(--accent-soft);
  }
  .led {
    width: 7px;
    height: 7px;
    flex: none;
    border-radius: 50%;
    background: var(--off);
    transition:
      background var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease);
  }
  .on .led {
    background: var(--c, var(--accent-2));
    box-shadow: 0 0 8px var(--c, var(--accent-2));
  }
  footer {
    display: flex;
    gap: var(--space-3);
    justify-content: flex-end;
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px;
    border: none;
    background: none;
    color: var(--text-faint);
    font-size: var(--text-xs);
    cursor: pointer;
    transition: color var(--dur-fast) var(--ease);
  }
  .link:hover {
    color: var(--text);
  }
  .link.danger:hover {
    color: var(--danger);
  }
</style>
