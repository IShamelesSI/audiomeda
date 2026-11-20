<script lang="ts">
  // Horizontal fader. A native range input underneath keeps keyboard and screen-reader
  // support; the visible track and fill are drawn on top.
  import { gainFromPosition, positionFromGain } from "$lib/api/engine.svelte";

  let {
    gain,
    onchange,
    label,
    muted = false,
    disabled = false,
    small = false,
    showValue = true,
    color,
  }: {
    gain: number;
    onchange: (gain: number) => void;
    label: string;
    muted?: boolean;
    disabled?: boolean;
    small?: boolean;
    showValue?: boolean;
    /** device color; default is the accent gradient */
    color?: string;
  } = $props();

  // while the user is moving it, their position wins over the (slightly older) engine value
  let held = $state(false);
  let local = $state(0);
  let release: ReturnType<typeof setTimeout> | undefined;
  const pos = $derived(held ? local : positionFromGain(gain));

  function oninput(e: Event) {
    local = +(e.currentTarget as HTMLInputElement).value;
    held = true;
    clearTimeout(release);
    release = setTimeout(() => (held = false), 700);
    onchange(gainFromPosition(local));
  }
</script>

<div class="fader" class:small class:muted class:disabled class:held style:--pos={pos} style:--fill={color} style:--fill-glow={color ? `0 0 12px ${color}` : undefined}>
  <div class="track">
    <div class="fill"></div>
  </div>
  <input
    type="range"
    min="0"
    max="1"
    step="0.005"
    value={pos}
    aria-label={label}
    aria-valuetext="{Math.round(pos * 100)}%"
    {disabled}
    {oninput}
  />
  {#if showValue}
    <span class="value">{Math.round(pos * 100)}</span>
  {/if}
</div>

<style>
  .fader {
    --h: var(--track-h);
    --thumb-d: var(--thumb-size);
    position: relative;
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: var(--space-3);
    min-width: 80px;
  }
  .small {
    --h: var(--track-h-sm);
    --thumb-d: var(--thumb-size-sm);
    gap: var(--space-2);
  }
  .track {
    grid-area: 1 / 1;
    height: var(--h);
    border-radius: var(--radius-pill);
    background: var(--track);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    width: calc(var(--pos) * 100%);
    background: var(--fill, var(--accent-grad));
    box-shadow: var(--fill-glow, var(--glow));
    border-radius: inherit;
    transition:
      width var(--dur) var(--ease),
      background var(--dur) var(--ease);
  }
  .held .fill {
    transition: none; /* follow the pointer exactly while dragging */
  }
  .muted .fill {
    background: var(--fill-muted);
    box-shadow: none;
  }
  input {
    grid-area: 1 / 1;
    width: 100%;
    margin: 0;
    height: calc(var(--thumb-d) + 8px);
    background: transparent;
    appearance: none;
    cursor: pointer;
  }
  input::-webkit-slider-runnable-track {
    background: transparent;
  }
  input::-webkit-slider-thumb {
    appearance: none;
    width: var(--thumb-d);
    height: var(--thumb-d);
    border-radius: 50%;
    background: var(--thumb);
    box-shadow:
      0 0 0 4px rgba(255, 255, 255, 0.08),
      var(--glow);
    transition: transform var(--dur-fast) var(--ease-spring);
  }
  input:hover::-webkit-slider-thumb {
    transform: scale(1.15);
  }
  input:active::-webkit-slider-thumb {
    transform: scale(1.28);
  }
  .muted input::-webkit-slider-thumb {
    box-shadow: 0 0 0 4px rgba(255, 255, 255, 0.05);
  }
  .disabled {
    opacity: 0.4;
  }
  .disabled input {
    cursor: default;
  }
  .value {
    min-width: 2.2em;
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  .small .value {
    font-size: var(--text-xs);
  }
</style>
