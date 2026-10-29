<script lang="ts">
  // Shortcut editor: click a shortcut, press the new keys. Some can also work
  // while Arcadia is in the background.
  import { ACTIONS, clashes, comboOf, isGlobal, keyOf, pretty, recording, resetKeys, setGlobal, setKey } from "../../lib/keys.svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../../components/Icon.svelte";
  import Toggle from "../../components/Toggle.svelte";

  let editing = $state<string | null>(null);
  let pending = $state("");
  let q = $state("");

  function start(id: string) {
    editing = id;
    pending = "";
    recording.on = true;
  }
  function stop() {
    editing = null;
    pending = "";
    recording.on = false;
  }
  function key(e: KeyboardEvent) {
    if (!editing) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey) return stop();
    if ((e.key === "Backspace" || e.key === "Delete") && !e.ctrlKey && !e.altKey) {
      setKey(editing, "");
      return stop();
    }
    const c = comboOf(e);
    if (!c) return;
    // Plain letters would fire while typing elsewhere; ask for a modifier.
    if (!/(Ctrl|Alt|Super)\+/.test(c) && !/^F\d+$/.test(c.split("+").pop()!)) {
      pending = c;
      return;
    }
    setKey(editing, c);
    stop();
  }
  const groups = $derived(
    ["App", "Pages", "Music", "Games"].map((g) => ({
      g,
      list: ACTIONS.filter((a) => a.group === g && (!q.trim() || a.label.toLowerCase().includes(q.trim().toLowerCase()))),
    })).filter((x) => x.list.length),
  );
  const changed = $derived(Object.keys(app.ui.keys.bindings).length + app.ui.keys.global.length > 0);
</script>

<svelte:window onkeydown={key} />

<section class="section">
  <div class="row spread">
    <h3>Keyboard</h3>
    <div class="row">
      <input placeholder="Find a shortcut" bind:value={q} aria-label="Find a shortcut" style="width: 13rem" />
      {#if changed}<button class="ghost" onclick={resetKeys}>Reset all</button>{/if}
    </div>
  </div>
  <p class="hint">Click a shortcut and press the keys you want. Backspace removes it, Esc cancels. <strong>Anywhere</strong> makes it work even while you're in a game or another app.</p>

  {#each groups as { g, list } (g)}
    <div class="group">{g}</div>
    <ul class="keys">
      {#each list as a (a.id)}
        {@const k = keyOf(a.id)}
        {@const clash = clashes(a.id, k)}
        <li>
          <span class="name">{a.label}</span>
          {#if clash.length}<span class="warn" title="Also used by {clash.map((c) => c.label).join(', ')}"><Icon name="shield" size={13} /> Same as {clash[0].label}</span>{/if}
          <span class="grow"></span>
          {#if a.canGlobal}
            <label class="anywhere" title="Works while Arcadia is in the background">
              <span class="faint small">Anywhere</span>
              <Toggle label="{a.label} works anywhere" checked={isGlobal(a.id)} onchange={(v) => setGlobal(a.id, v)} />
            </label>
          {/if}
          <button class="combo" class:rec={editing === a.id} class:empty={!k} onclick={() => (editing === a.id ? stop() : start(a.id))} aria-label="Change shortcut for {a.label}">
            {#if editing === a.id}
              {pending ? `${pretty(pending)}: add Ctrl or Alt` : "Press keys…"}
            {:else if k}
              {#each k.split("+") as part, i}{#if i}<span class="plus">+</span>{/if}<kbd>{pretty(part)}</kbd>{/each}
            {:else}
              Add shortcut
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/each}
  <p class="faint small">Music page keys (Space, arrows, F for full screen) work while you're on that page. Windows media keys always control Music.</p>
</section>

<style>
  .group { margin: 1rem 0 0.3rem; font-size: 0.75rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); }
  .keys { list-style: none; margin: 0; padding: 0; }
  .keys li { display: flex; align-items: center; gap: 0.8rem; padding: 0.45rem 0; border-bottom: 1px solid var(--line-soft); }
  .name { font-weight: 500; }
  .grow { flex: 1; }
  .warn { display: inline-flex; gap: 0.3rem; align-items: center; font-size: 0.75rem; color: var(--peach); }
  .anywhere { display: inline-flex; align-items: center; gap: 0.45rem; }
  .small { font-size: 0.75rem; }
  .combo { min-width: 10.5rem; display: inline-flex; align-items: center; justify-content: center; gap: 0.2rem; padding: 0.3rem 0.6rem; font-size: 0.82rem; font-weight: 500; background: var(--sunk); }
  .combo.empty { color: var(--faint); border-style: dashed; }
  .combo.rec { border-color: var(--lilac); box-shadow: 0 0 0 3px color-mix(in srgb, var(--lilac) 25%, transparent); color: var(--lilac); }
  kbd { display: inline-block; min-width: 1.5em; padding: 0.05rem 0.4rem; border-radius: 5px; background: var(--raised); border: 1px solid var(--line); border-bottom-width: 2px; font-family: var(--body); font-size: 0.78rem; font-weight: 600; text-align: center; }
  .plus { color: var(--faint); font-size: 0.75rem; }
</style>
