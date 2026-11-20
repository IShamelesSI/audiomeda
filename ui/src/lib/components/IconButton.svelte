<script lang="ts">
  import type { ComponentProps } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    icon,
    label,
    onclick,
    active = false,
    on = false,
    danger = false,
    pressed,
  }: {
    icon: ComponentProps<typeof Icon>["name"];
    label: string;
    onclick: () => void;
    /** red highlighted look (e.g. muted) */
    active?: boolean;
    /** accent highlighted look (e.g. pinned) */
    on?: boolean;
    danger?: boolean;
    /** set for toggle buttons so screen readers announce on/off */
    pressed?: boolean;
  } = $props();

  // replays the icon's pop each click
  let pop = $state(false);
</script>

<button
  class:active
  class:on
  class:danger
  aria-label={label}
  title={label}
  aria-pressed={pressed}
  onclick={() => {
    pop = true;
    onclick();
  }}
>
  <span class="icon" class:pop onanimationend={() => (pop = false)}><Icon name={icon} /></span>
</button>

<style>
  /* outlined so it reads as a button; the outline lights up on hover */
  button {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex: none;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text-dim);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease),
      box-shadow var(--dur) var(--ease),
      transform var(--dur-fast) var(--ease-spring);
  }
  button:hover {
    background: var(--surface-hover);
    color: var(--text);
    border-color: var(--accent);
    box-shadow: var(--glow);
  }
  button:active {
    transform: scale(0.9);
  }
  .icon {
    display: grid;
    place-items: center;
  }
  .pop {
    animation: pop var(--dur-slow) var(--ease-spring);
  }
  @keyframes pop {
    0% { transform: scale(1) rotate(0); }
    35% { transform: scale(0.7) rotate(-18deg); }
    70% { transform: scale(1.2) rotate(8deg); }
    100% { transform: scale(1) rotate(0); }
  }
  .on,
  .on:hover {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
  }
  .on:hover {
    box-shadow: var(--glow);
  }
  .active,
  .active:hover {
    background: var(--danger-soft);
    border-color: var(--danger);
    color: var(--danger);
  }
  .active:hover {
    box-shadow: 0 0 14px var(--danger-soft), 0 0 8px var(--danger);
  }
  .danger:hover {
    color: var(--danger);
    border-color: var(--danger);
    box-shadow: 0 0 10px var(--danger-soft);
  }
</style>
