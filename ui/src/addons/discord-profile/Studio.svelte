<script lang="ts">
  // Profile Studio: a sandbox for a Discord profile. Edits never touch
  // Discord; they live here until you copy them over by hand.
  import { onMount } from "svelte";
  import { addons, openUrl } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import Icon from "../../components/Icon.svelte";
  import Toggle from "../../components/Toggle.svelte";
  import Modal from "../../components/Modal.svelte";
  import Preview from "./Preview.svelte";
  import {
    DEFAULT_BANNER, LIMITS, STATUS_LABEL, blank, fingerprint, fromJson, newId, problems, readImageFile, thumbnail, toJson,
    type DProfile, type PreviewView, type ShelfEntry, type Status,
  } from "./model";
  import { contrast, suggest, type Suggestion } from "./palette";

  const ADDON = "discord-profile";

  // ── the profile being edited ──────────────────────────────
  let profile: DProfile = $state(blank());
  /** What the shelf holds for this profile, to show "unsaved" and compare. */
  let saved: DProfile | null = $state(null);
  /** The previous draft, after an import replaced it. */
  let replaced: DProfile | null = $state(null);
  let loaded = $state(false);

  /** The profile as it was when created, opened or pulled; edits since then
   *  are worth keeping if something replaces it. */
  let baseline = $state("");
  const edited = $derived(baseline !== fingerprint(profile));
  function setProfile(p: DProfile, shelfCopy: DProfile | null) {
    if (loaded && edited && (shelfCopy === null || profile.id !== shelfCopy.id)) replaced = $state.snapshot(profile) as DProfile;
    profile = p;
    saved = shelfCopy;
    baseline = fingerprint(p);
  }
  const savedPrint = $derived(saved ? fingerprint(saved) : null);
  const dirty = $derived(savedPrint === null || fingerprint(profile) !== savedPrint);
  const onShelf = $derived(saved !== null);
  const issues = $derived(problems(profile));
  const len = (s: string) => [...s].length;

  // ── shelf ─────────────────────────────────────────────────
  let shelf: ShelfEntry[] = $state([]);

  async function writeIndex() {
    await addons.set(ADDON, "index", $state.snapshot(shelf));
  }

  async function save(asNew = false) {
    if (asNew || !onShelf) {
      profile.id = newId();
      if (asNew && onShelf) profile.label = `${profile.label} (copy)`;
    }
    profile.updatedAt = Date.now();
    const snap = $state.snapshot(profile) as DProfile;
    try {
      await addons.set(ADDON, `p_${snap.id}`, snap);
      const entry: ShelfEntry = {
        id: snap.id,
        label: snap.label.trim() || "Untitled",
        name: snap.displayName.trim() || snap.username,
        thumb: await thumbnail(snap.avatar),
        bannerColor: snap.bannerColor,
        theme: snap.theme,
        updatedAt: snap.updatedAt,
      };
      const i = shelf.findIndex((s) => s.id === snap.id);
      if (i >= 0) shelf[i] = entry;
      else shelf.unshift(entry);
      await writeIndex();
      saved = structuredClone(snap);
      baseline = fingerprint(snap);
      toast(`Saved “${entry.label}”`);
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function open(id: string) {
    if (id === profile.id && !dirty) return;
    try {
      const p = await addons.get<DProfile>(ADDON, `p_${id}`);
      if (!p) throw new Error("That version is missing; it may have been deleted in another window.");
      setProfile(p, structuredClone(p));
    } catch (e) {
      toast(String(e), "error");
    }
  }

  let deleting: ShelfEntry | null = $state(null);
  let deleteOpen = $state(false);
  async function remove() {
    const e = deleting;
    if (!e) return;
    deleteOpen = false;
    try {
      await addons.set(ADDON, `p_${e.id}`, null);
      shelf = shelf.filter((s) => s.id !== e.id);
      await writeIndex();
      if (profile.id === e.id) saved = null;
      toast(`Deleted “${e.label}”`);
    } catch (err) {
      toast(String(err), "error");
    }
  }

  function startBlank() {
    setProfile(blank(), null);
  }

  function undoReplace() {
    if (!replaced) return;
    const back = replaced;
    replaced = null;
    profile = back;
    saved = null;
    baseline = "";
    // Re-read its shelf copy, if it has one, so "unsaved" is right again.
    if (shelf.some((s) => s.id === back.id)) addons.get<DProfile>(ADDON, `p_${back.id}`).then((p) => (saved = p));
  }

  // ── the working draft survives restarts ───────────────────
  let draftTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const snap = $state.snapshot(profile);
    if (!loaded) return;
    clearTimeout(draftTimer);
    draftTimer = setTimeout(() => addons.set(ADDON, "draft", snap).catch(() => {}), 700);
  });

  // ── import ────────────────────────────────────────────────
  let query = $state("");
  let looking = $state(false);
  let lookupError: string | null = $state(null);
  let notes: string[] = $state([]);
  let settingsOpen = $state(false);
  let tokenSaved = $state(false);
  let tokenInput = $state("");

  async function fetchImg(url: string | null, what: string): Promise<string | null> {
    if (!url) return null;
    try {
      return await addons.discordImage(url);
    } catch {
      notes.push(`The ${what} couldn't be downloaded, so it's left empty.`);
      return null;
    }
  }

  async function pull() {
    if (!query.trim() || looking) return;
    looking = true;
    lookupError = null;
    notes = [];
    try {
      const f = await addons.discordLookup(query.trim());
      const [avatar, banner, decoration] = await Promise.all([
        fetchImg(f.avatarUrl, "avatar"),
        fetchImg(f.bannerUrl, "banner"),
        fetchImg(f.decorationUrl, "avatar decoration"),
      ]);
      setProfile({
        ...blank(),
        label: f.displayName ?? f.username,
        username: f.username,
        displayName: f.displayName ?? "",
        avatar,
        banner,
        decoration,
        bannerColor: f.accentColor ?? DEFAULT_BANNER,
        createdAt: f.createdAt,
        source: { userId: f.id, via: f.source, at: Date.now() },
      }, null);
      notes = [
        ...notes,
        ...f.notes,
        "Discord doesn't share About me, pronouns or Nitro profile colours with apps, so add those yourself.",
      ];
      if (f.bannerAnimated || f.avatarAnimated) notes.push("Animated images keep their animation here.");
      query = "";
    } catch (e) {
      lookupError = String(e);
    } finally {
      looking = false;
    }
  }

  async function saveToken() {
    try {
      await addons.setSecret(ADDON, "bot_token", tokenInput.trim() || null);
      tokenSaved = !!tokenInput.trim();
      tokenInput = "";
      toast(tokenSaved ? "Bot token saved in Windows Credential Manager" : "Bot token forgotten");
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function forgetToken() {
    tokenInput = "";
    await saveToken();
  }

  // ── files ─────────────────────────────────────────────────
  type Slot = "avatar" | "banner" | "decoration";
  const inputs: Partial<Record<Slot | "json", HTMLInputElement>> = $state({});
  async function picked(slot: Slot, e: Event) {
    const el = e.currentTarget as HTMLInputElement;
    const f = el.files?.[0];
    el.value = "";
    if (!f) return;
    try {
      profile[slot] = await readImageFile(f);
    } catch (err) {
      toast(String((err as Error).message ?? err), "error");
    }
  }
  async function pickedJson(e: Event) {
    const el = e.currentTarget as HTMLInputElement;
    const f = el.files?.[0];
    el.value = "";
    if (!f) return;
    try {
      const p = fromJson(await f.text());
      setProfile(p, null);
      toast(`Opened “${p.label}”. Save it to keep it on the shelf.`);
    } catch (err) {
      toast(String((err as Error).message ?? err), "error");
    }
  }
  async function copyJson() {
    try {
      await navigator.clipboard.writeText(toJson($state.snapshot(profile) as DProfile));
      toast("Copied. Paste it into a .json file to share or back it up.");
    } catch {
      toast("The clipboard isn't available here", "error");
    }
  }

  // ── colours ───────────────────────────────────────────────
  let palettes: Suggestion[] = $state([]);
  let paletteBusy = $state(false);
  type Target = "primary" | "accent" | "banner";
  let target: Target = $state("primary");

  $effect(() => {
    const a = profile.avatar;
    const b = profile.banner;
    paletteBusy = true;
    let live = true;
    suggest(a, b)
      .then((p) => { if (live) palettes = p; })
      .catch(() => { if (live) palettes = []; })
      .finally(() => { if (live) paletteBusy = false; });
    return () => { live = false; };
  });

  const fallbackTheme = () => {
    const p = palettes.find((x) => !x.missing);
    return p ? { primary: p.primary, accent: p.accent } : { primary: "#1e1f4b", accent: "#5865f2" };
  };
  function setThemed(on: boolean) {
    profile.theme = on ? (profile.theme ?? fallbackTheme()) : null;
  }
  function applyPalette(p: Suggestion) {
    profile.theme = { primary: p.primary, accent: p.accent };
    if (!profile.banner) profile.bannerColor = p.bannerColor;
    toast(`${p.title} applied`);
  }
  function useSwatch(hex: string) {
    if (target === "banner") profile.bannerColor = hex;
    else profile.theme = { ...(profile.theme ?? fallbackTheme()), [target]: hex };
  }
  const readability = (c: number) => (c >= 4.5 ? "Easy to read" : c >= 3 ? "Readable" : "Low contrast");
  const themeContrast = $derived(profile.theme ? Math.min(contrast("#ffffff", profile.theme.primary), contrast("#ffffff", profile.theme.accent)) : null);

  // ── preview ───────────────────────────────────────────────
  let view: PreviewView = $state("popout");
  let scheme: "dark" | "light" = $state("dark");
  let compare = $state(false);
  const canCompare = $derived(onShelf && dirty);
  $effect(() => {
    if (!canCompare) compare = false;
  });

  onMount(async () => {
    try {
      const [index, draft, secrets] = await Promise.all([
        addons.get<ShelfEntry[]>(ADDON, "index"),
        addons.get<DProfile>(ADDON, "draft"),
        addons.secrets(ADDON),
      ]);
      shelf = index ?? [];
      tokenSaved = secrets.includes("bot_token");
      if (draft) {
        const copy = shelf.some((s) => s.id === draft.id) ? await addons.get<DProfile>(ADDON, `p_${draft.id}`) : null;
        setProfile({ ...blank(), ...draft }, copy);
      } else {
        baseline = fingerprint(profile);
      }
    } catch (e) {
      toast(String(e), "error");
    }
    loaded = true;
  });

  const statuses: Status[] = ["online", "idle", "dnd", "offline"];
  const ago = (t: number) => {
    const m = Math.round((Date.now() - t) / 60000);
    if (m < 1) return "just now";
    if (m < 60) return `${m} min ago`;
    const h = Math.round(m / 60);
    if (h < 24) return `${h} h ago`;
    return new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  };
</script>

{#snippet counter(v: string, max: number)}
  <span class="count" class:over={len(v) > max}>{len(v)}/{max}</span>
{/snippet}

{#snippet imageSlot(slot: Slot, label: string, hint: string)}
  <div class="img-slot">
    <button class="thumb {slot}" onclick={() => inputs[slot]?.click()} aria-label="Choose {label}">
      {#if profile[slot]}
        <img src={profile[slot]} alt="" />
      {:else}
        <Icon name="upload" size={18} />
      {/if}
    </button>
    <div class="img-meta">
      <div class="label">{label}</div>
      <div class="desc">{hint}</div>
      <div class="row">
        <button class="ghost tiny" onclick={() => inputs[slot]?.click()}>{profile[slot] ? "Change" : "Choose"}</button>
        {#if profile[slot]}<button class="ghost tiny danger" onclick={() => (profile[slot] = null)}>Remove</button>{/if}
      </div>
    </div>
    <input type="file" accept="image/png,image/jpeg,image/gif,image/webp" hidden bind:this={inputs[slot]} onchange={(e) => picked(slot, e)} />
  </div>
{/snippet}

<div class="studio">
  <header class="top">
    <div class="title">
      <div class="eyebrow"><Icon name="profile" size={14} /> Profile Studio <span class="pill">Addon</span></div>
      <input class="label-in" bind:value={profile.label} maxlength="40" aria-label="Name of this version" placeholder="Untitled" />
      <div class="state faint">
        {#if !onShelf}Not on the shelf yet{:else if dirty}Unsaved changes{:else}Saved {ago(saved!.updatedAt)}{/if}
        {#if profile.source?.userId}· pulled from Discord ID {profile.source.userId}{/if}
      </div>
    </div>
    <div class="row wrap actions">
      <button class="ghost" onclick={startBlank}><Icon name="plus" size={15} /> Blank</button>
      <button class="ghost" onclick={() => inputs.json?.click()}><Icon name="upload" size={15} /> Open file</button>
      <button class="ghost" onclick={copyJson}><Icon name="copy" size={15} /> Copy JSON</button>
      {#if onShelf}<button class="ghost" onclick={() => save(true)}>Save as copy</button>{/if}
      <button class="save" disabled={onShelf && !dirty} onclick={() => save(false)}><Icon name="save" size={15} /> {onShelf ? "Save" : "Save to shelf"}</button>
      <input type="file" accept="application/json,.json" hidden bind:this={inputs.json} onchange={pickedJson} />
    </div>
  </header>

  {#if replaced}
    <div class="notice">
      <span>Your previous draft was replaced.</span>
      <button class="ghost tiny" onclick={undoReplace}><Icon name="undo" size={14} /> Bring it back</button>
      <button class="ghost tiny" onclick={() => (replaced = null)}>Dismiss</button>
    </div>
  {/if}

  <!-- Saved versions -->
  <div class="shelf" aria-label="Saved profiles">
    {#each shelf as s (s.id)}
      <div class="shelf-item" class:on={s.id === profile.id}>
        <button class="shelf-open" onclick={() => open(s.id)} title="Open “{s.label}”">
          <span class="shelf-art" style:background={s.theme ? `linear-gradient(${s.theme.primary}, ${s.theme.accent})` : s.bannerColor}>
            {#if s.thumb}<img src={s.thumb} alt="" />{:else}<span class="shelf-init">{(s.name[0] ?? "?").toUpperCase()}</span>{/if}
          </span>
          <span class="shelf-text">
            <span class="shelf-label">{s.label}</span>
            <span class="shelf-sub">{s.name} · {ago(s.updatedAt)}</span>
          </span>
        </button>
        <button class="ghost shelf-x" aria-label="Delete “{s.label}”" onclick={() => { deleting = s; deleteOpen = true; }}><Icon name="close" size={13} /></button>
      </div>
    {:else}
      <div class="shelf-empty faint">Saved versions show up here. Try a few looks and flip between them.</div>
    {/each}
  </div>

  <div class="split">
    <!-- ── Editor ── -->
    <div class="editor">
      <section class="section">
        <div class="row spread"><h3>Pull a profile</h3>
          <button class="ghost tiny" onclick={() => (settingsOpen = !settingsOpen)} aria-expanded={settingsOpen}>
            Import settings {#if tokenSaved}<span class="pill live"><span class="dot"></span>Bot token</span>{/if}
          </button>
        </div>
        <form class="pull" onsubmit={(e) => { e.preventDefault(); pull(); }}>
          <input bind:value={query} placeholder="Username, user ID or profile link" aria-label="Discord username or user ID" spellcheck="false" />
          <button type="submit" class="save" disabled={looking || !query.trim()}>{looking ? "Pulling…" : "Pull"}</button>
        </form>
        {#if lookupError}<p class="err">{lookupError}</p>{/if}
        {#each notes as n}<p class="note">{n}</p>{/each}
        {#if settingsOpen}
          <div class="token">
            <p class="desc">
              Discord only lets apps look people up with a bot token. Use one from your own app: in the
              <button class="link" onclick={() => openUrl("https://discord.com/developers/applications")}>Developer Portal</button>
              open your app → Bot → Reset Token. To find people by <em>username</em>, the bot must share a server with them and have
              <strong>Server Members Intent</strong> turned on. A plain user ID works in any case.
            </p>
            <p class="desc">Without a token, IDs are looked up through Lanyard, which only knows people in its own Discord server.</p>
            <div class="row">
              <input type="password" bind:value={tokenInput} placeholder={tokenSaved ? "Saved. Paste a new one to replace it" : "Bot token"} aria-label="Bot token" autocomplete="off" />
              <button disabled={!tokenInput.trim()} onclick={saveToken}>Save</button>
              {#if tokenSaved}<button class="ghost danger" onclick={forgetToken}>Forget</button>{/if}
            </div>
            <p class="desc faint">Kept in Windows Credential Manager and only ever sent to discord.com. Never paste your <em>account</em> token: using it in apps breaks Discord's rules.</p>
          </div>
        {/if}
      </section>

      <section class="section">
        <h3>Identity</h3>
        <label class="field">
          <span class="row spread"><span class="label">Display name</span>{@render counter(profile.displayName, LIMITS.displayName)}</span>
          <input bind:value={profile.displayName} placeholder={profile.username} />
          {#if issues.displayName}<span class="err">{issues.displayName}</span>{/if}
        </label>
        <div class="two">
          <label class="field">
            <span class="row spread"><span class="label">Username</span>{@render counter(profile.username, LIMITS.username)}</span>
            <input bind:value={profile.username} oninput={() => (profile.username = profile.username.toLowerCase())} spellcheck="false" />
            {#if issues.username}<span class="err">{issues.username}</span>{/if}
          </label>
          <label class="field">
            <span class="row spread"><span class="label">Pronouns</span>{@render counter(profile.pronouns, LIMITS.pronouns)}</span>
            <input bind:value={profile.pronouns} placeholder="Add your pronouns" />
            {#if issues.pronouns}<span class="err">{issues.pronouns}</span>{/if}
          </label>
        </div>
        <div class="two">
          <label class="field">
            <span class="label">Status</span>
            <select bind:value={profile.status}>
              {#each statuses as s}<option value={s}>{STATUS_LABEL[s]}</option>{/each}
            </select>
          </label>
          <label class="field">
            <span class="row spread"><span class="label">Custom status</span>{@render counter(profile.customStatus, LIMITS.customStatus)}</span>
            <input bind:value={profile.customStatus} placeholder="What's up?" />
          </label>
        </div>
      </section>

      <section class="section">
        <h3>Images</h3>
        {@render imageSlot("avatar", "Avatar", "Square works best; it's shown as a circle. GIFs stay animated.")}
        {@render imageSlot("banner", "Banner", "Discord crops it to 5 : 2 (e.g. 680 × 240).")}
        {@render imageSlot("decoration", "Avatar decoration", "A transparent PNG or APNG that frames the avatar.")}
        <div class="setting">
          <div><div class="label">Banner colour</div><div class="desc">Shown when there's no banner image.</div></div>
          <div class="row">
            <input class="hex" bind:value={profile.bannerColor} maxlength="7" aria-label="Banner colour hex" />
            <input type="color" bind:value={profile.bannerColor} aria-label="Banner colour" />
          </div>
        </div>
      </section>

      <section class="section">
        <div class="row spread">
          <h3>Colours</h3>
          {#if paletteBusy}<span class="faint small">Reading images…</span>{/if}
        </div>
        <div class="setting">
          <div><div class="label">Profile colours</div><div class="desc">Nitro's two-colour gradient behind the whole profile.</div></div>
          <Toggle label="Profile colours" checked={!!profile.theme} onchange={setThemed} />
        </div>
        {#if profile.theme}
          <div class="theme-row">
            <label class="swatch-in"><span>Top</span><input type="color" bind:value={profile.theme.primary} /><input class="hex" bind:value={profile.theme.primary} maxlength="7" /></label>
            <label class="swatch-in"><span>Bottom</span><input type="color" bind:value={profile.theme.accent} /><input class="hex" bind:value={profile.theme.accent} maxlength="7" /></label>
            <button class="ghost tiny" title="Swap" onclick={() => { const t = profile.theme!; profile.theme = { primary: t.accent, accent: t.primary }; }}>Swap</button>
            {#if themeContrast !== null}<span class="pill" class:warn={themeContrast < 3}>{readability(themeContrast)}</span>{/if}
          </div>
        {/if}

        <div class="pal-head">
          <span class="label">Suggested palettes</span>
          <div class="seg" role="radiogroup" aria-label="Swatch sets">
            <button role="radio" aria-checked={target === "primary"} onclick={() => (target = "primary")}>Top</button>
            <button role="radio" aria-checked={target === "accent"} onclick={() => (target = "accent")}>Bottom</button>
            <button role="radio" aria-checked={target === "banner"} onclick={() => (target = "banner")}>Banner</button>
          </div>
        </div>
        <p class="desc">Apply a whole palette, or click any swatch to use it as the {target === "banner" ? "banner colour" : `${target === "primary" ? "top" : "bottom"} colour`}.</p>
        <div class="palettes">
          {#each palettes as p (p.id)}
            <div class="pal" class:missing={!!p.missing}>
              <div class="row spread">
                <span class="pal-title">{p.title}</span>
                {#if !p.missing}<span class="pill" class:warn={p.contrast < 3}>{readability(p.contrast)}</span>{/if}
              </div>
              {#if p.missing}
                <p class="desc">{p.missing}.</p>
              {:else}
                <div class="sw-row">
                  {#each p.swatches as sw}
                    <button class="sw" style:background={sw.hex} style:flex-grow={0.4 + sw.weight * 4} title="{sw.hex} · {Math.round(sw.weight * 100)}% · use as {target}" aria-label="Use {sw.hex}" onclick={() => useSwatch(sw.hex)}></button>
                  {/each}
                </div>
                <div class="pair">
                  <span class="grad" style:background="linear-gradient(90deg, {p.primary}, {p.accent})"></span>
                  <button class="tiny" onclick={() => applyPalette(p)}>Apply</button>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      </section>

      <section class="section">
        <div class="row spread"><h3>About me</h3>{@render counter(profile.bio, LIMITS.bio)}</div>
        <textarea class="bio-in" rows="5" bind:value={profile.bio} placeholder="Markdown works: **bold**, *italic*, __underline__, ~~strike~~, `code`, ||spoiler||, # headings, > quotes"></textarea>
        {#if issues.bio}<span class="err">{issues.bio}. Discord will cut it off.</span>{/if}
      </section>
    </div>

    <!-- ── Preview ── -->
    <div class="preview-col">
      <div class="pv-bar">
        <div class="seg" role="radiogroup" aria-label="Preview">
          <button role="radio" aria-checked={view === "popout"} onclick={() => (view = "popout")}>Popout</button>
          <button role="radio" aria-checked={view === "profile"} onclick={() => (view = "profile")}>Full profile</button>
          <button role="radio" aria-checked={view === "chat"} onclick={() => (view = "chat")}>Chat</button>
        </div>
        <div class="row">
          {#if canCompare}
            <button class="ghost tiny" class:on={compare} aria-pressed={compare} onclick={() => (compare = !compare)}>Before / after</button>
          {/if}
          <button class="ghost icon-btn" onclick={() => (scheme = scheme === "dark" ? "light" : "dark")} aria-label="Switch to {scheme === 'dark' ? 'light' : 'dark'} theme" title="Discord {scheme === 'dark' ? 'light' : 'dark'} theme">
            <Icon name={scheme === "dark" ? "sun" : "moon"} size={16} />
          </button>
        </div>
      </div>
      <div class="stage {scheme} view-{view}" class:pair={compare && saved}>
        {#if compare && saved}
          <div class="cmp"><span class="cmp-l">Saved</span><Preview profile={saved} {view} {scheme} /></div>
          <div class="cmp"><span class="cmp-l">Now</span><Preview {profile} {view} {scheme} onstatus={(s) => (profile.status = s)} /></div>
        {:else}
          <Preview {profile} {view} {scheme} onstatus={(s) => (profile.status = s)} />
        {/if}
      </div>
      <p class="faint small stage-note">A sandbox: nothing here changes your real Discord profile. Click the status dot to change it.</p>
    </div>
  </div>
</div>

<Modal bind:open={deleteOpen} title={`Delete “${deleting?.label ?? ""}”?`}>
  <p>This removes it from the shelf for good. The profile you're editing stays open.</p>
  {#snippet actions()}
    <button onclick={() => (deleteOpen = false)}>Keep it</button>
    <button class="danger" onclick={remove}>Delete</button>
  {/snippet}
</Modal>

<style>
  .studio { height: 100%; display: flex; flex-direction: column; min-height: 0; padding: 1.4rem 1.8rem 0; gap: 0.8rem; }
  .top { display: flex; align-items: flex-end; justify-content: space-between; gap: 1rem; flex-wrap: wrap; }
  .eyebrow { display: flex; align-items: center; gap: 0.4rem; font-size: 0.78rem; font-weight: 700; color: var(--muted); letter-spacing: 0.04em; }
  .label-in {
    font-family: var(--display); font-size: 1.55rem; font-weight: 700; letter-spacing: -0.01em;
    background: transparent; border: 1px solid transparent; padding: 0.05rem 0.3rem; margin-left: -0.3rem; width: min(28ch, 100%);
  }
  .label-in:hover { border-color: var(--line-soft); }
  .label-in:focus { border-color: var(--lilac); background: var(--sunk); }
  .state { font-size: 0.82rem; }
  .actions { gap: 0.35rem; }
  .save { background: var(--peach); border-color: var(--peach); color: var(--peach-ink); display: inline-flex; align-items: center; gap: 0.35rem; }
  .save:hover { background: color-mix(in srgb, var(--peach) 85%, white); border-color: transparent; }
  .actions .ghost { display: inline-flex; align-items: center; gap: 0.35rem; }

  .notice { display: flex; align-items: center; gap: 0.6rem; padding: 0.45rem 0.8rem; border-radius: var(--r-md); background: color-mix(in srgb, var(--lilac) 12%, var(--sunk)); border: 1px solid color-mix(in srgb, var(--lilac) 30%, transparent); font-size: 0.88rem; }

  .shelf { display: flex; gap: 0.5rem; overflow-x: auto; padding-bottom: 0.3rem; flex-shrink: 0; }
  .shelf-item { position: relative; flex-shrink: 0; }
  .shelf-open { display: flex; align-items: center; gap: 0.55rem; padding: 0.35rem 1.8rem 0.35rem 0.4rem; border-radius: 999px; background: var(--sunk); border-color: var(--line-soft); }
  .shelf-item.on .shelf-open { border-color: var(--peach); box-shadow: 0 0 0 1px var(--peach); }
  .shelf-art { width: 30px; height: 30px; border-radius: 50%; padding: 2px; display: grid; place-items: center; overflow: hidden; }
  .shelf-art img { width: 100%; height: 100%; border-radius: 50%; object-fit: cover; }
  .shelf-init { color: white; font-weight: 700; font-size: 0.85rem; }
  .shelf-text { display: grid; text-align: left; line-height: 1.15; }
  .shelf-label { font-size: 0.85rem; font-weight: 600; max-width: 16ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .shelf-sub { font-size: 0.72rem; color: var(--faint); font-weight: 500; max-width: 20ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .shelf-x { position: absolute; right: 4px; top: 50%; transform: translateY(-50%); padding: 0.2rem; border-radius: 50%; opacity: 0; }
  .shelf-item:hover .shelf-x, .shelf-x:focus-visible { opacity: 1; }
  .shelf-empty { font-size: 0.85rem; padding: 0.4rem 0; }

  .split { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(340px, 1fr) minmax(340px, 1.05fr); gap: 1.4rem; }
  .editor { overflow-y: auto; padding-right: 0.4rem; padding-bottom: 3rem; }
  .preview-col { overflow-y: auto; padding-bottom: 2rem; display: flex; flex-direction: column; gap: 0.7rem; }

  .field { display: grid; gap: 0.3rem; margin-bottom: 0.75rem; }
  .field input, .field select { width: 100%; }
  .label { font-weight: 600; font-size: 0.9rem; }
  .desc { color: var(--muted); font-size: 0.85rem; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
  .count { font-size: 0.75rem; color: var(--faint); font-variant-numeric: tabular-nums; }
  .count.over { color: var(--rose); font-weight: 700; }
  .err { color: var(--rose); font-size: 0.82rem; }
  .note { color: var(--muted); font-size: 0.84rem; margin-top: 0.4rem; padding-left: 0.6rem; border-left: 2px solid var(--line); }
  .small { font-size: 0.8rem; }
  button.tiny { padding: 0.22rem 0.6rem; font-size: 0.8rem; display: inline-flex; gap: 0.3rem; align-items: center; }
  button.link { border: 0; background: none; padding: 0; color: var(--lilac); font-weight: 600; font-size: inherit; }
  button.on { color: var(--text); background: var(--raised); border-color: var(--line); }

  .pull { display: flex; gap: 0.5rem; margin-top: 0.4rem; }
  .pull input { flex: 1; min-width: 0; }
  .token { margin-top: 0.8rem; display: grid; gap: 0.55rem; padding: 0.8rem; border-radius: var(--r-md); background: var(--sunk); border: 1px solid var(--line-soft); }
  .token input { flex: 1; min-width: 0; }

  .img-slot { display: flex; gap: 0.9rem; align-items: center; padding: 0.55rem 0; }
  .img-slot + .img-slot { border-top: 1px solid var(--line-soft); }
  .thumb { padding: 0; overflow: hidden; display: grid; place-items: center; background: var(--sunk); color: var(--faint); flex-shrink: 0; border-style: dashed; }
  .thumb img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .thumb.avatar { width: 56px; height: 56px; border-radius: 50%; }
  .thumb.banner { width: 112px; height: 45px; border-radius: var(--r-sm); }
  .thumb.decoration { width: 56px; height: 56px; border-radius: var(--r-sm); background: repeating-conic-gradient(var(--sunk) 0 25%, var(--raised) 0 50%) 0 0 / 12px 12px; }
  .thumb.decoration img { object-fit: contain; }
  .img-meta { display: grid; gap: 0.15rem; min-width: 0; }
  .img-meta .row { gap: 0.2rem; margin-left: -0.6rem; }
  .hex { width: 6.2rem; font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.82rem; }
  input[type="color"] { width: 2.2rem; height: 2rem; padding: 2px; border-radius: var(--r-sm); cursor: pointer; }

  .theme-row { display: flex; flex-wrap: wrap; align-items: center; gap: 0.8rem; padding: 0.3rem 0 0.8rem; }
  .swatch-in { display: flex; align-items: center; gap: 0.4rem; font-size: 0.85rem; font-weight: 600; }
  .pal-head { display: flex; align-items: center; justify-content: space-between; gap: 0.8rem; margin-top: 0.6rem; flex-wrap: wrap; }
  .palettes { display: grid; gap: 0.6rem; margin: 0.7rem 0 0.8rem; }
  .pal { padding: 0.7rem 0.8rem; border-radius: var(--r-md); background: var(--sunk); border: 1px solid var(--line-soft); display: grid; gap: 0.5rem; }
  .pal.missing { opacity: 0.65; }
  .pal-title { font-weight: 700; font-size: 0.88rem; }
  .sw-row { display: flex; gap: 4px; height: 30px; }
  .sw { flex-basis: 0; min-width: 18px; padding: 0; border: 1px solid rgba(0, 0, 0, 0.18); border-radius: var(--r-sm); transition: transform 0.1s; }
  .sw:hover { transform: translateY(-2px); border-color: var(--text); }
  .pair { display: flex; align-items: center; gap: 0.6rem; }
  .grad { flex: 1; height: 12px; border-radius: 999px; }
  .bio-in { width: 100%; font-family: var(--body); font-size: 0.92rem; min-height: 6.5rem; }

  .pv-bar { display: flex; justify-content: space-between; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  .icon-btn { padding: 0.35rem; display: grid; place-items: center; }
  .stage {
    flex-shrink: 0;
    border-radius: var(--r-lg); padding: 1.6rem 1.2rem;
    display: flex; justify-content: center; align-items: flex-start; gap: 1rem;
    background: #1e1f22; border: 1px solid var(--line-soft);
    min-height: 420px;
  }
  .stage.light { background: #e3e5e8; }
  .stage.pair { justify-content: space-evenly; flex-wrap: wrap; }
  /* Two popouts side by side, a little smaller, so the change is easy to spot. */
  .stage.pair.view-popout { flex-wrap: nowrap; gap: 0.6rem; padding-inline: 0.6rem; }
  .stage.pair.view-popout .cmp { zoom: 0.76; }
  .stage.pair.view-popout .cmp-l { zoom: 1.3; }
  .cmp { display: grid; gap: 0.4rem; justify-items: center; min-width: 0; }
  .cmp-l { font-size: 0.72rem; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; color: #949ba4; }
  .stage :global(.dc) { width: 100%; display: flex; justify-content: center; }
  .stage.pair :global(.dc) { width: auto; }
  .stage-note { text-align: center; }

  @media (max-width: 1100px) {
    .split { grid-template-columns: 1fr; overflow-y: auto; }
    .editor, .preview-col { overflow: visible; }
    .preview-col { order: -1; }
  }
</style>
