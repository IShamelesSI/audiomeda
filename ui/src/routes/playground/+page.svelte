<script lang="ts">
  // Every component in every state, with fake data and no engine. Tune the look here.
  import type { Row, Column } from "$lib/api/types";
  import Fader from "$lib/components/Fader.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import InputCard from "$lib/components/InputCard.svelte";
  import OutputCard from "$lib/components/OutputCard.svelte";
  import AddMenu from "$lib/components/AddMenu.svelte";
  import PresetBar from "$lib/components/PresetBar.svelte";
  import Toasts from "$lib/components/Toasts.svelte";

  let gain = $state(0.36);
  let mute = $state(false);
  let presets = $state(["Gaming", "Singing", "VR"]);
  let toasts = $state<{ id: number; text: string }[]>([]);

  const outputs = [
    { id: "hp", name: "Cans", color: "#ef4444", virtual: false },
    { id: "vr", name: "VR headset", virtual: false },
    { id: "mic-a", name: "Discord mic", color: "#10b981", virtual: true },
  ];
  let pinned = $state(true);
  const row = (volume: number, mute: boolean, cells: Row["cells"]): Row => ({ volume, mute, cells });
  let rows = $state<Record<string, Row>>({
    live: row(0.64, false, { hp: { volume: 0.09, on: true }, vr: { volume: 0.36, on: true } }),
    muted: row(0.8, true, { hp: { volume: 1, on: true } }),
    offline: row(1, false, { hp: { volume: 1, on: true } }),
    mic: row(1, false, { "mic-a": { volume: 1, on: true } }),
  });
  let cols = $state<Record<string, Column>>({
    hp: { name: "Speakers (USB Headset)", volume: 1, mute: false },
    vr: { name: "VR headset", volume: 0.5, mute: true },
  });

  const handlers = (k: string) => ({
    onvolume: (g: number) => (rows[k].volume = g),
    onmute: (m: boolean) => (rows[k].mute = m),
    oncell: (o: string, g: number) => (rows[k].cells[o] = { volume: g, on: true }),
    oncellon: (o: string, on: boolean) => (rows[k].cells[o] = { volume: rows[k].cells[o]?.volume ?? 0, on }),
    onreset: () => {},
    onlabel: () => {},
  });
</script>

<main>
  <h1>Playground</h1>
  <p class="hint">Fake data, no engine. Edit <code>src/lib/theme/tokens.css</code> and watch it change. <a href="/">Back to the mixer</a></p>

  <section>
    <h2>Fader</h2>
    <div class="row">
      <div class="cell"><span>normal</span><Fader {gain} label="demo" onchange={(g) => (gain = g)} /></div>
      <div class="cell"><span>small</span><Fader small {gain} label="demo small" onchange={(g) => (gain = g)} /></div>
      <div class="cell"><span>muted</span><Fader muted {gain} label="demo muted" onchange={(g) => (gain = g)} /></div>
      <div class="cell"><span>disabled</span><Fader disabled {gain} label="demo disabled" onchange={() => {}} /></div>
      <div class="cell"><span>0 and 100</span><Fader gain={0} label="zero" onchange={() => {}} /><Fader gain={1} label="full" onchange={() => {}} /></div>
    </div>
  </section>

  <section>
    <h2>Buttons</h2>
    <div class="row">
      <IconButton icon={mute ? "speaker-off" : "speaker"} label="Mute" active={mute} pressed={mute} onclick={() => (mute = !mute)} />
      <IconButton icon="x" label="Remove" danger onclick={() => {}} />
      <IconButton icon="reset" label="Reset" onclick={() => {}} />
      <AddMenu label="Add output" choices={[{ id: "a", name: "Monitor" }, { id: "b", name: "CABLE-B Input" }]} onpick={() => {}} />
      <AddMenu label="Add mic" choices={[]} onpick={() => {}} />
      <button class="demo" onclick={() => (toasts = [...toasts, { id: Date.now(), text: "That device is the sink; it can't also be an output" }])}>
        Show toast
      </button>
    </div>
  </section>

  <section>
    <h2>Presets</h2>
    <PresetBar
      {presets}
      onload={() => {}}
      ondelete={(p) => (presets = presets.filter((x) => x !== p))}
      onsave={(p) => (presets = [...presets, p])}
    />
  </section>

  <section>
    <h2>Outputs</h2>
    <div class="row">
      <OutputCard name="Cans (USB Headset)" original="USB Headset" color="#ef4444" label={{ name: "Cans", color: "#ef4444" }} live column={cols.hp} onvolume={(g) => (cols.hp.volume = g)} onmute={(m) => (cols.hp.mute = m)} onlabel={() => {}} onremove={() => {}} />
      <OutputCard name="VR headset (muted)" original="VR headset" live column={cols.vr} onvolume={(g) => (cols.vr.volume = g)} onmute={(m) => (cols.vr.mute = m)} onlabel={() => {}} onremove={() => {}} />
      <OutputCard name="Monitor" original="Monitor" color="#a855f7" live={false} column={cols.hp} onvolume={() => {}} onmute={() => {}} onlabel={() => {}} onremove={() => {}} />
    </div>
  </section>

  <section>
    <h2>Input cards</h2>
    <div class="grid">
      <InputCard name="Opera (playing)" kind="app" status="playing" row={rows.live} {outputs} {...handlers("live")} onlabel={undefined} onpin={() => {}} />
      <InputCard name="Discord (muted)" kind="app" status="open" row={rows.muted} {outputs} {...handlers("muted")} onlabel={undefined} onpin={() => {}} />
      <InputCard name="Spotify (pinned, closed)" kind="app" status="closed" {pinned} row={rows.offline} {outputs} {...handlers("offline")} onlabel={undefined} onpin={(p) => (pinned = p)} />
      <InputCard name="Studio mic" original="USB Headset" kind="mic" status="playing" color="#f97316" row={rows.mic} {outputs} {...handlers("mic")} onremove={() => {}} />
    </div>
  </section>

  <Toasts {toasts} />
</main>

<style>
  main {
    height: 100vh;
    overflow-y: auto;
    padding: var(--space-5);
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
  }
  .hint {
    color: var(--text-dim);
    font-size: var(--text-sm);
  }
  .hint a {
    color: var(--accent);
  }
  section {
    margin-top: var(--space-5);
  }
  h2 {
    font-size: var(--text-xs);
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
  }
  .cell {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    width: 220px;
    font-size: var(--text-xs);
    color: var(--text-faint);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-min), 1fr));
    gap: var(--space-4);
  }
  .demo {
    padding: 7px 12px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-pill);
    background: var(--surface);
    font-size: var(--text-sm);
    cursor: pointer;
  }
</style>
