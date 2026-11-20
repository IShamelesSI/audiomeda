<script lang="ts">
  // The square badge on a card: the app's real icon if we have one, else an icon or the
  // first letter on the device's color (or the accent gradient).
  import type { ComponentProps } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    name,
    src = null,
    icon,
    color,
    dim = false,
    size = 38,
  }: {
    name: string;
    src?: string | null;
    icon?: ComponentProps<typeof Icon>["name"];
    color?: string;
    dim?: boolean;
    size?: number;
  } = $props();
</script>

<div
  class="avatar"
  class:image={!!src}
  class:dim
  style:--size="{size}px"
  style:--tint={color}
  style:--tint-glow={color ? `0 0 14px ${color}88` : undefined}
>
  {#if src}
    <img {src} alt="" />
  {:else if icon}
    <Icon name={icon} size={Math.round(size * 0.47)} />
  {:else}
    {name.charAt(0).toUpperCase()}
  {/if}
</div>

<style>
  .avatar {
    display: grid;
    place-items: center;
    width: var(--size);
    height: var(--size);
    flex: none;
    border-radius: 12px;
    background: var(--tint, var(--accent-grad));
    color: #fff;
    font-family: var(--font-display);
    font-size: var(--text-lg);
    font-weight: 600;
    box-shadow: var(--tint-glow, var(--glow));
    transition:
      filter var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background var(--dur) var(--ease);
  }
  .image {
    background: var(--surface-strong);
    box-shadow: none;
  }
  img {
    width: 78%;
    height: 78%;
    object-fit: contain;
    filter: drop-shadow(0 2px 6px rgba(0, 0, 0, 0.4));
  }
  .dim {
    filter: grayscale(1) brightness(0.7);
    box-shadow: none;
  }
</style>
