<script lang="ts">
  // Saved Roblox accounts. Logins live in Windows Credential Manager.
  import Icon from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";
  import { api } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { RobloxAccount } from "../../lib/types";
  import Thumb from "./Thumb.svelte";
  import { rbx, loadAccounts, signIn, play, ago } from "./rbx.svelte";

  let status = $state<Record<number, "checking" | "ok" | "out">>({});
  async function check(a: RobloxAccount) {
    status[a.id] = "checking";
    const ok = await api.robloxAccountCheck(a.id).catch(() => false);
    status[a.id] = ok ? "ok" : "out";
    if (ok) loadAccounts();
  }
  async function checkAll() {
    await Promise.all(rbx.accounts.list.map(check));
  }

  async function makeDefault(id: number | null) {
    await api.robloxAccountDefault(id);
    await loadAccounts();
  }
  async function note(a: RobloxAccount, v: string) {
    a.note = v;
    await api.robloxAccountNote(a.id, v);
  }
  async function move(i: number, by: number) {
    const ids = rbx.accounts.list.map((a) => a.id);
    const j = i + by;
    if (j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    await api.robloxAccountReorder(ids);
    await loadAccounts();
  }

  let removing = $state<RobloxAccount | null>(null);
  let confirming = $state(false);
  async function remove() {
    if (!removing) return;
    await api.robloxAccountRemove(removing.id);
    toast(`Removed ${removing.displayName} and deleted its saved login`);
    confirming = false;
    await loadAccounts();
  }
</script>

<section class="section">
  <div class="row spread">
    <h3>Accounts</h3>
    <div class="row">
      {#if rbx.accounts.list.length}<button class="ghost" onclick={checkAll}><Icon name="refresh" size={15} /> Check sign-ins</button>{/if}
      <button onclick={signIn} disabled={rbx.signingIn}><Icon name="plus" size={15} /> {rbx.signingIn ? "Waiting for sign-in…" : "Add account"}</button>
    </div>
  </div>
  <p class="hint">
    Sign in on Roblox's own page, in a private window. Arcadia keeps each login in Windows Credential Manager and never writes it to a file.
    Play as any of them, or run several at once with <em>Multiple Roblox windows</em> on.
  </p>

  {#if rbx.accounts.list.length === 0}
    <div class="empty">
      <Icon name="users" size={28} />
      <strong>No accounts yet</strong>
      <span class="muted">Add your main and your alts to switch between them in one click.</span>
    </div>
  {:else}
    <ul class="list">
      {#each rbx.accounts.list as a, i (a.id)}
        <li class="acct" class:def={rbx.accounts.default === a.id}>
          <div class="order">
            <button class="ghost tiny" aria-label="Move up" disabled={i === 0} onclick={() => move(i, -1)}><Icon name="up" size={14} /></button>
            <button class="ghost tiny" aria-label="Move down" disabled={i === rbx.accounts.list.length - 1} onclick={() => move(i, 1)}><Icon name="down" size={14} /></button>
          </div>
          <Thumb src={a.avatar} seed={a.id} label={a.displayName} size={52} round />
          <div class="who">
            <div class="name">
              <strong>{a.displayName}</strong>
              <span class="muted">@{a.name}</span>
              {#if status[a.id] === "ok"}<span class="pill live"><span class="dot"></span>Signed in</span>
              {:else if status[a.id] === "out"}<span class="pill warn">Signed out</span>
              {:else if status[a.id] === "checking"}<span class="pill">Checking…</span>{/if}
            </div>
            <input class="note" placeholder="Add a note (main, trading, AFK…)" value={a.note} onchange={(e) => note(a, e.currentTarget.value)} aria-label="Note for {a.displayName}" />
            <div class="faint small">Last played {ago(a.lastUsed)} · added {ago(a.added)}</div>
          </div>
          <div class="acts">
            <button class="ghost star" class:on={rbx.accounts.default === a.id} aria-pressed={rbx.accounts.default === a.id}
              title={rbx.accounts.default === a.id ? "Default account" : "Make default"}
              onclick={() => makeDefault(rbx.accounts.default === a.id ? null : a.id)}>
              <Icon name="star" size={17} filled={rbx.accounts.default === a.id} />
            </button>
            {#if status[a.id] === "out"}
              <button onclick={signIn}>Sign in again</button>
            {:else}
              <button onclick={() => play(null, null, a.id)}><Icon name="play" size={14} filled /> Play</button>
            {/if}
            <button class="ghost danger" aria-label="Remove {a.displayName}" onclick={() => { removing = a; confirming = true; }}><Icon name="trash" size={16} /></button>
          </div>
        </li>
      {/each}
    </ul>
    <p class="hint">The <Icon name="star" size={12} filled /> default account is used by Play, join links from roblox.com and game shortcuts.</p>
  {/if}
</section>

<Modal bind:open={confirming} title="Remove {removing?.displayName ?? ''}?">
  <p>Its saved login is deleted from Credential Manager. Your play history stays.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (confirming = false)}>Keep</button>
    <button class="danger" onclick={remove}>Remove account</button>
  {/snippet}
</Modal>

<style>
  .row button { display: inline-flex; align-items: center; gap: 0.4rem; }
  .empty {
    display: flex; flex-direction: column; align-items: center; gap: 0.35rem; text-align: center;
    padding: 2.2rem 1rem; border: 1px dashed var(--line); border-radius: var(--r-lg); color: var(--muted);
  }
  .empty strong { color: var(--text); }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.55rem; }
  .acct {
    display: flex; align-items: center; gap: 0.9rem;
    padding: 0.75rem 0.9rem 0.75rem 0.4rem; border-radius: var(--r-lg);
    background: var(--sunk); border: 1px solid var(--line-soft);
  }
  .acct.def { border-color: color-mix(in srgb, #ffd66b 35%, var(--line-soft)); }
  .order { display: flex; flex-direction: column; }
  .tiny { padding: 0.1rem 0.25rem; line-height: 0; }
  .who { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.15rem; }
  .name { display: flex; align-items: baseline; gap: 0.5rem; flex-wrap: wrap; }
  .note {
    background: transparent; border-color: transparent; padding: 0.15rem 0.35rem; margin-left: -0.35rem;
    font-size: 0.88rem; color: var(--muted); max-width: 22rem;
  }
  .note:hover { border-color: var(--line-soft); }
  .note:focus { color: var(--text); background: var(--raised); }
  .small { font-size: 0.78rem; }
  .acts { display: flex; align-items: center; gap: 0.35rem; }
  .acts button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .star { color: var(--faint); }
  .star.on { color: #ffd66b; }
  .hint :global(svg) { vertical-align: -1px; color: #ffd66b; }
</style>
