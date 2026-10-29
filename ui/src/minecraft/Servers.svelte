<script lang="ts">
  // The instance's multiplayer servers: who's online, ping, and joining one
  // straight from the launcher.
  import { mc } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { mcs } from "../lib/mc.svelte";
  import type { McInstance, McSavedServer, McServerStatus } from "../lib/types";
  import Icon from "../components/Icon.svelte";

  let { inst, running }: { inst: McInstance; running: boolean } = $props();

  let list = $state<McSavedServer[]>([]);
  let status = $state<Record<string, McServerStatus | string | undefined>>({});
  async function load() {
    list = await mc.servers(inst.id).catch(() => []);
    for (const s of list) ping(s.address);
  }
  $effect(() => {
    void inst.id;
    load();
  });
  async function ping(address: string) {
    status[address] = undefined;
    try {
      status[address] = await mc.serverPing(address);
    } catch (e) {
      status[address] = String(e);
    }
  }

  let name = $state("");
  let address = $state("");
  async function add() {
    try {
      await mc.serverAdd(inst.id, name.trim() || address.trim(), address.trim());
      name = address = "";
      await load();
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function remove(s: McSavedServer) {
    await mc.serverRemove(inst.id, s.address);
    list = list.filter((x) => x !== s);
  }
  async function join(s: McSavedServer) {
    try {
      mcs.progress = { task: "launch", message: `Starting and joining ${s.name}`, done: 0, total: 0 };
      await mc.play(inst.id, s.address);
    } catch (e) {
      mcs.progress = null;
      toast(String(e), "error");
    }
  }
  const st = (a: string) => status[a];
  const isOk = (v: unknown): v is McServerStatus => typeof v === "object" && v !== null;
  const bars = (ms: number) => (ms < 80 ? 4 : ms < 150 ? 3 : ms < 300 ? 2 : 1);
</script>

<div class="row spread bar">
  <h3>Servers <span class="n">{list.length}</span></h3>
  <button class="ghost" onclick={load}><Icon name="refresh" size={15} /> Refresh</button>
</div>
<p class="hint">The same list as Multiplayer in game. <strong>Join</strong> starts Minecraft and connects straight away.</p>

<ul class="servers">
  {#each list as s (s.address)}
    {@const v = st(s.address)}
    <li>
      {#if (isOk(v) && v.icon) || s.icon}<img src={(isOk(v) && v.icon) || s.icon} alt="" width="48" height="48" />{:else}<span class="noicon"><Icon name="globe" size={20} /></span>{/if}
      <div class="txt">
        <div class="top"><strong>{s.name}</strong><span class="mono faint addr">{s.address}</span></div>
        {#if v === undefined}
          <span class="faint small">Pinging…</span>
        {:else if isOk(v)}
          <span class="motd">{v.motd || " "}</span>
          <span class="faint small">{v.version}</span>
        {:else}
          <span class="err small">Can't reach it: {v}</span>
        {/if}
      </div>
      {#if isOk(v)}
        <div class="stats">
          <span class="players"><Icon name="users" size={14} /> {v.online.toLocaleString()}<span class="faint">/{v.max.toLocaleString()}</span></span>
          <span class="ping" title="{v.pingMs} ms">
            {#each [1, 2, 3, 4] as b}<i class:on={b <= bars(v.pingMs)} style:height="{b * 3 + 2}px"></i>{/each}
            <span class="faint small">{v.pingMs} ms</span>
          </span>
        </div>
      {/if}
      <div class="acts">
        <button class="ghost danger icon" aria-label="Remove {s.name}" onclick={() => remove(s)}><Icon name="trash" size={15} /></button>
        <button disabled={running} onclick={() => join(s)}><Icon name="play" size={14} filled /> Join</button>
      </div>
    </li>
  {/each}
</ul>

<div class="row add">
  <input placeholder="Name" bind:value={name} aria-label="Server name" style="width: 12rem" />
  <input class="grow mono" placeholder="Address, e.g. play.example.net" bind:value={address} onkeydown={(e) => e.key === "Enter" && address.trim() && add()} aria-label="Server address" />
  <button disabled={!address.trim()} onclick={add}><Icon name="plus" size={15} /> Add server</button>
</div>

<style>
  .bar { margin-bottom: 0.2rem; }
  .bar button, .acts button, .add button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .n { color: var(--faint); font-weight: 500; }
  .hint { font-size: 0.85rem; color: var(--faint); margin-bottom: 0.8rem; }
  .servers { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.45rem; }
  .servers li { display: flex; align-items: center; gap: 0.9rem; padding: 0.65rem 0.8rem; border-radius: var(--r-lg); background: var(--sunk); border: 1px solid var(--line-soft); }
  img, .noicon { width: 48px; height: 48px; border-radius: var(--r-sm); flex: none; image-rendering: pixelated; }
  .noicon { display: grid; place-items: center; color: var(--faint); background: var(--raised); }
  .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.1rem; }
  .top { display: flex; gap: 0.6rem; align-items: baseline; min-width: 0; }
  .addr { font-size: 0.78rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .motd { font-size: 0.85rem; color: var(--muted); white-space: pre-line; line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .small { font-size: 0.78rem; }
  .err { color: var(--rose); }
  .stats { display: flex; flex-direction: column; align-items: flex-end; gap: 0.25rem; font-size: 0.85rem; font-variant-numeric: tabular-nums; }
  .players { display: inline-flex; align-items: center; gap: 0.3rem; font-weight: 600; }
  .ping { display: inline-flex; align-items: flex-end; gap: 2px; }
  .ping i { width: 4px; border-radius: 1px; background: var(--line); }
  .ping i.on { background: var(--mint); }
  .ping .small { margin-left: 0.35rem; }
  .acts { display: flex; gap: 0.3rem; align-items: center; }
  .icon { padding: 0.35rem; }
  .add { margin-top: 0.9rem; }
</style>
