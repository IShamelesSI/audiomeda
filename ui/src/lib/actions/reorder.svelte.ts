// Drag-to-reorder for cards. The order is a per-window preference, kept in localStorage.

const KEY = "audiomeda.order";

function load(): Record<string, string[]> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {
    return {};
  }
}

/** group -> keys in the user's order; keys never dragged go last, in their given order */
const order = $state<Record<string, string[]>>(load());

export function sortByOrder<T>(group: string, items: T[], key: (t: T) => string): T[] {
  const o = order[group] ?? [];
  const at = (k: string) => {
    const i = o.indexOf(k);
    return i < 0 ? o.length : i;
  };
  return [...items].sort((a, b) => at(key(a)) - at(key(b)));
}

/** Moves `dragged` to where `target` is among the visible `keys`; hidden keys keep their place after. */
function moveKey(group: string, keys: string[], dragged: string, target: string) {
  const after = keys.indexOf(dragged) < keys.indexOf(target);
  const list = keys.filter((k) => k !== dragged);
  list.splice(list.indexOf(target) + (after ? 1 : 0), 0, dragged);
  order[group] = [...list, ...(order[group] ?? []).filter((k) => !list.includes(k))];
}

function save() {
  try {
    localStorage.setItem(KEY, JSON.stringify(order));
  } catch {}
}

function scrollParent(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    if (/(auto|scroll)/.test(getComputedStyle(p).overflowY)) return p;
  }
  return null;
}

/** Svelte action: drag the element (not its controls) onto a sibling of the same group to reorder. */
export function dragSort(node: HTMLElement, opts: { group: string; key: string; keys: () => string[] }) {
  let o = opts;
  const tag = () => {
    node.dataset.dragGroup = o.group;
    node.dataset.dragKey = o.key;
  };
  tag();

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("button, input, select, textarea, a, label, [role=dialog]")) return;
    const sx = e.clientX;
    const sy = e.clientY;
    let dragging = false;
    let last: string | undefined; // only react when the hovered card changes, so cards don't flip back and forth
    let px = sx;
    let py = sy;

    const hover = () => {
      const el = document.elementFromPoint(px, py)?.closest<HTMLElement>("[data-drag-group]");
      const target = el && el !== node && el.dataset.dragGroup === o.group ? el.dataset.dragKey : undefined;
      if (target && target !== last) moveKey(o.group, o.keys(), o.key, target);
      last = target;
    };

    // near the top/bottom edge of the scrolling column, scroll it so off-screen spots can be reached
    const scroller = scrollParent(node);
    let raf = 0;
    const autoScroll = () => {
      if (scroller) {
        const r = scroller.getBoundingClientRect();
        const edge = 48;
        const dy = py < r.top + edge ? -(r.top + edge - py) : py > r.bottom - edge ? py - (r.bottom - edge) : 0;
        if (dy) {
          scroller.scrollTop += Math.max(-20, Math.min(20, dy / 2));
          hover();
        }
      }
      raf = requestAnimationFrame(autoScroll);
    };

    const move = (ev: PointerEvent) => {
      px = ev.clientX;
      py = ev.clientY;
      if (!dragging) {
        if (Math.hypot(ev.clientX - sx, ev.clientY - sy) < 6) return;
        dragging = true;
        node.classList.add("dragging");
        document.body.classList.add("drag-sorting");
        raf = requestAnimationFrame(autoScroll);
      }
      hover();
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (!dragging) return;
      cancelAnimationFrame(raf);
      node.classList.remove("dragging");
      document.body.classList.remove("drag-sorting");
      save();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  node.addEventListener("pointerdown", down);
  return {
    update(n: typeof opts) {
      o = n;
      tag();
    },
    destroy() {
      node.removeEventListener("pointerdown", down);
    },
  };
}
