<script lang="ts">
  // Newer versions of an instance's mods, resource packs and shaders.
  import { mc } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import type { McInstance, McUpdate } from "../lib/types";
  import Icon from "../components/Icon.svelte";

  let { inst, running, found = $bindable(0), onchanged }: { inst: McInstance; running: boolean; found?: number; onchanged: () => void } = $props();

  let list = $state<McUpdate[] | null>(null);
  let checking = $state(false);
  let doing = $state<Set<string>>(new Set());
  let done = $state<string[]>([]);

  async function check() {
    checking = true;
    done = [];
    try {
      list = await mc.updates(inst.id);
      found = list.length;
    } catch (e) {
      toast(String(e), "error");
    } finally {
      checking = false;
    }
  }
  $effect(() => {
    void inst.id;
    list = null;
    check();
  });

  async function apply(u: McUpdate) {
    doing = new Set(doing).add(u.fileName);
    try {
      await mc.updateApply(inst.id, u);
      list = (list ?? []).filter((x) => x !== u);
      found = list.length;
      done = [...done, u.title];
      onchanged();
    } catch (e) {
      toast(`${u.title}: ${e}`, "error");
    } finally {
      const d = new Set(doing);
      d.delete(u.fileName);
      doing = d;
    }
  }
  async function all() {
    for (const u of [...(list ?? [])]) await apply(u);
    toast(`Updated ${done.length} item${done.length === 1 ? "" : "s"}`);
  }
  const FOLDER: Record<string, string> = { mods: "Mod", resourcepacks: "Resource pack", shaderpacks: "Shader" };
</script>

<div class="row spread bar">
  <div>
    <h3>Updates</h3>
    <p class="hint">Checks Modrinth for newer versions made for Minecraft {inst.mcVersion}{inst.loader !== "vanilla" ? ` and ${inst.loader}` : ""}. Files from elsewhere can't be checked.</p>
  </div>
  <div class="row">
    <button class="ghost" disabled={checking} onclick={check}><Icon name="refresh" size={15} /> {checking ? "Checking…" : "Check again"}</button>
    {#if list && list.length > 1}<button disabled={running || doing.size > 0} onclick={all}><Icon name="download" size={15} /> Update all ({list.length})</button>{/if}
  </div>
</div>
{#if running}<p class="warn">Close the game before updating; Windows keeps its files locked while it runs.</p>{/if}

{#if list === null}
  <p class="none">{checking ? "Checking for updates…" : ""}</p>
{:else if list.length === 0}
  <div class="clear"><Icon name="check" size={22} /><div><strong>Everything is up to date</strong>{#if done.length}<p class="muted">Updated {done.join(", ")}.</p>{/if}</div></div>
{:else}
  <ul class="ups">
    {#each list as u (u.folder + u.fileName)}
      <li>
        {#if u.icon}<img src={u.icon} alt="" width="40" height="40" loading="lazy" />{:else}<span class="noicon"><Icon name="cube" size={18} /></span>{/if}
        <div class="txt">
          <span class="nm">{u.title} <span class="pill tiny">{FOLDER[u.folder]}</span></span>
          <span class="ver"><span class="faint">{u.current || "?"}</span> <span class="faint">→</span> <strong>{u.latest}</strong></span>
        </div>
        <button disabled={running || doing.has(u.fileName)} onclick={() => apply(u)}>{doing.has(u.fileName) ? "Updating…" : "Update"}</button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .bar { margin-bottom: 0.6rem; align-items: flex-start; }
  .bar button { display: inline-flex; align-items: center; gap: 0.35rem; white-space: nowrap; }
  .bar > .row { flex: none; }
  .hint { font-size: 0.82rem; color: var(--faint); margin-top: 0.2rem; max-width: 60ch; }
  .warn { color: var(--peach); font-size: 0.85rem; margin-bottom: 0.6rem; }
  .none { color: var(--muted); padding: 1.2rem 0; }
  .clear { display: flex; gap: 0.8rem; align-items: center; padding: 1rem 1.1rem; border-radius: var(--r-lg); background: color-mix(in srgb, var(--mint) 8%, var(--sunk)); border: 1px solid color-mix(in srgb, var(--mint) 25%, var(--line-soft)); }
  .clear :global(svg) { color: var(--mint); }
  .ups { list-style: none; margin: 0; padding: 0; }
  .ups li { display: flex; align-items: center; gap: 0.8rem; padding: 0.55rem 0; border-bottom: 1px solid var(--line-soft); }
  img, .noicon { width: 40px; height: 40px; border-radius: var(--r-sm); flex: none; }
  .noicon { display: grid; place-items: center; background: var(--sunk); color: var(--faint); }
  .txt { flex: 1; min-width: 0; display: grid; gap: 0.1rem; }
  .nm { font-weight: 600; display: flex; gap: 0.4rem; align-items: center; }
  .tiny { font-size: 0.68rem; }
  .ver { font-size: 0.82rem; display: inline-flex; gap: 0.35rem; align-items: center; color: var(--mint); }
</style>
