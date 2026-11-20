<script lang="ts">
  // The mixer: outputs, apps, input devices, virtual outputs.
  import { onMount } from "svelte";
  import { flip } from "svelte/animate";
  import { fade, scale } from "svelte/transition";
  import { engine, actions, appIcon, startPolling } from "$lib/api/engine.svelte";
  import { colorOf, displayName, fullName, isVirtual, originalName, shortDevice } from "$lib/api/names";
  import type { Label, Snapshot } from "$lib/api/types";
  import { ms } from "$lib/theme/motion";
  import { dragSort, sortByOrder } from "$lib/actions/reorder.svelte";
  import InputCard from "$lib/components/InputCard.svelte";
  import OutputCard from "$lib/components/OutputCard.svelte";
  import AddMenu from "$lib/components/AddMenu.svelte";
  import PresetBar from "$lib/components/PresetBar.svelte";
  import Toasts from "$lib/components/Toasts.svelte";

  onMount(() => startPolling());

  const snap = $derived(engine.snap);

  const outputs = $derived(
    snap
      ? Object.entries(snap.state.outputs).map(([id, column]) => ({
          id,
          column,
          name: displayName(id, snap),
          full: fullName(id, snap),
          original: originalName(id, snap),
          color: colorOf(id, snap),
          label: snap.state.labels[id],
          live: snap.active_outputs.includes(id),
          virtual: isVirtual(column.name),
        }))
      : [],
  );
  const physicalOutputs = $derived(sortByOrder("outputs", outputs.filter((o) => !o.virtual), (o) => o.id));
  const virtualOutputs = $derived(sortByOrder("virtual", outputs.filter((o) => o.virtual), (o) => o.id));
  // sends follow the order of the output cards
  const sendTargets = $derived([...physicalOutputs, ...virtualOutputs].map(({ id, name, color, virtual }) => ({ id, name, color, virtual })));

  const inputs = $derived(
    snap
      ? Object.entries(snap.state.inputs)
          .map(([key, row]) => {
            const label = snap.state.labels[key];
            return {
              key,
              row,
              label,
              kind: (key.startsWith("mic:") ? "mic" : "app") as "mic" | "app",
              name: displayName(key, snap),
              original: originalName(key, snap),
              color: colorOf(key, snap),
              pinned: label?.pinned ?? false,
              status: status(key, snap),
            };
          })
          .sort((a, b) => a.name.localeCompare(b.name))
      : [],
  );
  // apps come and go with their windows; pinned ones stay
  const apps = $derived(sortByOrder("apps", inputs.filter((i) => i.kind === "app" && (i.status !== "closed" || i.pinned)), (i) => i.key));
  const mics = $derived(sortByOrder("mics", inputs.filter((i) => i.kind === "mic"), (i) => i.key));

  function status(key: string, s: Snapshot): "playing" | "open" | "closed" {
    if (key.startsWith("mic:")) return s.active_inputs.includes(key) ? "playing" : "closed";
    if (s.playing_apps.includes(key)) return "playing";
    return s.open_apps.includes(key) ? "open" : "closed";
  }

  /** devices not added yet, named "Custom (Windows name)" if the user renamed them before */
  function addable(devices: Record<string, string>, skip: (id: string) => boolean) {
    if (!snap) return [];
    const s = snap;
    return Object.entries(devices)
      .filter(([id]) => !skip(id))
      .map(([id, name]) => ({ id, name: s.state.labels[id]?.name ? `${s.state.labels[id].name} (${shortDevice(name)})` : name }));
  }
  const addOutput = (virtual: boolean) =>
    snap
      ? addable(snap.renders, (id) => id === snap.state.sink || id in snap.state.outputs || isVirtual(snap.renders[id]) !== virtual)
      : [];
  const addableMics = $derived(snap ? addable(snap.captures, (id) => `mic:${id}` in snap.state.inputs) : []);

  const setLabel = (key: string, current: Label | undefined, change: Label) => actions.setLabel(key, { ...current, ...change });

  // left column width in % of the window; a per-window preference, so localStorage is enough
  const SPLIT_KEY = "audiomeda.split";
  const clampSplit = (v: number) => Math.min(75, Math.max(25, v));
  let split = $state(clampSplit(Number(readSplit()) || 50));
  let columns: HTMLElement | undefined = $state();
  function readSplit() {
    try { return localStorage.getItem(SPLIT_KEY); } catch { return null; }
  }
  function saveSplit() {
    try { localStorage.setItem(SPLIT_KEY, String(Math.round(split * 10) / 10)); } catch {}
  }
  function dragSplit(e: PointerEvent) {
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const r = columns!.getBoundingClientRect();
      split = clampSplit(((ev.clientX - r.left) / r.width) * 100);
    };
    const up = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
      saveSplit();
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
  }
  function keySplit(e: KeyboardEvent) {
    const step = e.key === "ArrowLeft" ? -2 : e.key === "ArrowRight" ? 2 : 0;
    if (!step) return;
    e.preventDefault();
    split = clampSplit(split + step);
    saveSplit();
  }
</script>

{#snippet outputCards(list: typeof outputs, group: string)}
  <div class="grid">
    {#each list as o (o.id)}
      <div class="out-wrap drag" use:dragSort={{ group, key: o.id, keys: () => list.map((x) => x.id) }} animate:flip={{ duration: ms("--dur-slow") }} transition:scale={{ start: 0.94, duration: ms("--dur") }}>
        <OutputCard
          name={o.full}
          original={o.original}
          live={o.live}
          column={o.column}
          color={o.color}
          label={o.label}
          onvolume={(g) => actions.setOutputVolume(o.id, g)}
          onmute={(m) => actions.setOutputMute(o.id, m)}
          onlabel={(l) => setLabel(o.id, o.label, l)}
          onremove={() => actions.removeOutput(o.id)}
        />
      </div>
    {/each}
  </div>
{/snippet}

{#snippet inputCards(list: typeof inputs, group: string)}
  <div class="grid">
    {#each list as i (i.key)}
      <div class="drag" use:dragSort={{ group, key: i.key, keys: () => list.map((x) => x.key) }} animate:flip={{ duration: ms("--dur-slow") }} in:scale={{ start: 0.94, duration: ms("--dur") }} out:fade={{ duration: ms("--dur-fast") }}>
        <InputCard
          name={i.name}
          original={i.original}
          kind={i.kind}
          status={i.status}
          row={i.row}
          outputs={sendTargets}
          color={i.color}
          label={i.label}
          pinned={i.pinned}
          iconSrc={i.kind === "app" ? appIcon(snap?.state.app_paths[i.key]) : null}
          onvolume={(g) => actions.setInputVolume(i.key, g)}
          onmute={(m) => actions.setInputMute(i.key, m)}
          oncell={(o, g) => actions.setCellVolume(i.key, o, g)}
          oncellon={(o, on) => actions.setCellOn(i.key, o, on)}
          onreset={() => actions.resetInput(i.key)}
          onpin={i.kind === "app" ? (p) => setLabel(i.key, i.label, { pinned: p }) : undefined}
          onlabel={i.kind === "mic" ? (l) => setLabel(i.key, i.label, l) : undefined}
          onremove={i.kind === "mic" ? () => actions.removeMic(i.key.slice(4)) : undefined}
        />
      </div>
    {/each}
  </div>
{/snippet}

<div class="app">
  <header class="top">
    <img src="/logo.png" alt="" class="logo" />
    <h1>Audiomeda</h1>
    <span class="conn" class:down={engine.error}>{engine.error ? "Engine offline" : "Connected"}</span>
    {#if import.meta.env.DEV}<a class="dev" href="/playground">Playground</a>{/if}
    <div class="spacer"></div>
    {#if snap}
      <PresetBar
        presets={Object.keys(snap.state.presets)}
        onload={actions.loadPreset}
        ondelete={actions.deletePreset}
        onsave={actions.savePreset}
      />
    {/if}
  </header>

  {#if engine.error}
    <div class="banner" transition:fade={{ duration: ms("--dur") }}>
      Can't reach the Audiomeda engine. Start <code>audiomeda-engine.exe</code>; this window reconnects by itself.
    </div>
  {/if}

  {#if snap}
    <main bind:this={columns} style:--split="{split}%">
      <div class="col">
      <section>
        <div class="section-head">
          <h2>Outputs</h2>
          <AddMenu label="Add output" choices={addOutput(false)} onpick={actions.addOutput} />
        </div>
        {#if physicalOutputs.length}
          {@render outputCards(physicalOutputs, "outputs")}
        {:else}
          <p class="empty">No outputs. Add your headphones or speakers.</p>
        {/if}
      </section>

      <section>
        <div class="section-head"><h2>Apps <span class="count">{apps.length}</span></h2></div>
        {#if apps.length}
          {@render inputCards(apps, "apps")}
        {:else}
          <p class="empty">No apps open. Start something that plays sound and it shows up here.</p>
        {/if}
      </section>
      </div>

      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="splitter"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize columns"
        aria-valuenow={Math.round(split)}
        aria-valuemin={25}
        aria-valuemax={75}
        tabindex="0"
        onpointerdown={dragSplit}
        onkeydown={keySplit}
        ondblclick={() => ((split = 50), saveSplit())}
      ></div>

      <div class="col">
      <section>
        <div class="section-head">
          <h2>Input devices</h2>
          <AddMenu label="Add mic" choices={addableMics} onpick={actions.addMic} />
        </div>
        {#if mics.length}
          {@render inputCards(mics, "mics")}
        {:else}
          <p class="empty">No mics added.</p>
        {/if}
      </section>

      <section>
        <div class="section-head">
          <h2>Virtual outputs</h2>
          <AddMenu label="Add virtual output" choices={addOutput(true)} onpick={actions.addOutput} />
        </div>
        {#if virtualOutputs.length}
          {@render outputCards(virtualOutputs, "virtual")}
        {:else}
          <p class="empty">No virtual outputs. Add a cable to use it as a mic in Discord or OBS.</p>
        {/if}
      </section>
      </div>
    </main>
  {/if}

  <Toasts toasts={engine.toasts} />
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
    border-bottom: 1px solid var(--border);
  }
  .logo {
    width: 34px;
    height: 34px;
    filter: drop-shadow(0 0 10px rgba(167, 139, 250, 0.5));
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-xl);
    font-weight: 600;
    letter-spacing: 0.01em;
    background: var(--accent-grad);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .conn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-xs);
    color: var(--text-dim);
  }
  .conn::before {
    content: "";
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }
  .conn.down::before {
    background: var(--danger);
    box-shadow: 0 0 8px var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .dev {
    font-size: var(--text-xs);
    color: var(--text-faint);
  }
  .banner {
    margin: var(--space-4) var(--space-5) 0;
    padding: var(--space-3) var(--space-4);
    border: 1px solid var(--danger);
    border-radius: var(--radius);
    background: var(--danger-soft);
    font-size: var(--text-sm);
  }
  /* two columns filling the window height; each scrolls on its own */
  main {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, var(--split)) auto minmax(0, 1fr);
  }
  .col {
    min-height: 0;
    overflow-y: auto;
    padding: var(--space-4) var(--space-5) var(--space-6);
  }
  .splitter {
    position: relative;
    width: 9px;
    cursor: col-resize;
    touch-action: none;
    outline: none;
  }
  .splitter::before {
    content: "";
    position: absolute;
    inset: var(--space-4) 4px;
    border-radius: var(--radius-pill);
    background: var(--border);
    transition: background var(--dur) var(--ease), box-shadow var(--dur) var(--ease), inset var(--dur) var(--ease);
  }
  .splitter:hover::before,
  .splitter:active::before,
  .splitter:focus-visible::before {
    inset: var(--space-4) 3px;
    background: var(--accent);
    box-shadow: var(--glow);
  }
  section + section {
    margin-top: var(--space-6);
  }
  .section-head {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin-bottom: var(--space-3);
  }
  h2 {
    margin: 0;
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .count {
    margin-left: 4px;
    padding: 1px 7px;
    border-radius: var(--radius-pill);
    background: var(--surface-strong);
    letter-spacing: 0;
  }
  .out-wrap {
    display: flex;
  }
  /* drag a card by any spot that isn't a control */
  .drag {
    cursor: grab;
  }
  .drag > :global(*) {
    transition: transform var(--dur) var(--ease-spring), box-shadow var(--dur) var(--ease), opacity var(--dur) var(--ease);
  }
  .drag:global(.dragging) {
    position: relative;
    z-index: 2;
  }
  .drag:global(.dragging) > :global(*) {
    transform: scale(1.03) !important; /* beats the card's own hover lift */
    box-shadow: var(--shadow-hover), var(--glow);
    opacity: 0.92;
  }
  :global(body.drag-sorting),
  :global(body.drag-sorting *) {
    cursor: grabbing !important;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-min), 1fr));
    gap: var(--space-4);
  }
  .empty {
    margin: 0;
    color: var(--text-faint);
    font-size: var(--text-sm);
  }
  code {
    font-size: 0.95em;
  }
</style>
