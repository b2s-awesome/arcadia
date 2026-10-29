# Arcadia

A game hub for Windows that does three jobs in one small native app:

1. **Universal library.** Finds installed games from Steam, Epic Games, Battle.net, GOG, EA app and Ubisoft Connect, plus anything you add by hand, and launches them through their own launcher.
2. **Roblox bootstrapper** in the style of Bloxstrap / Fishstrap / Froststrap: installs and updates Roblox itself, FastFlags, mods, custom font, FPS cap, multi-instance, process priority, channels and version pinning.
3. **Asset proxy** in the style of Fleasion 2.4: swaps or removes Roblox assets by ID before they reach the client, serves your own files as replacements, overrides FastFlags live, and shows every request in an Inspector.

Built with Tauri 2: Rust backend, Svelte 5 UI. The installer is about 10 MB and the app idles at a few MB of RAM.

## Build it (Windows)

Prerequisites, once:

- [Rust](https://rustup.rs) (stable, MSVC toolchain)
- [Node.js 20+](https://nodejs.org)
- Microsoft C++ Build Tools ("Desktop development with C++")
- WebView2 is already on Windows 10/11

Then:

```powershell
npm install
npm install --prefix ui
npm run tauri build
```

The installer lands in `target\release\bundle\nsis\Arcadia_0.1.0_x64-setup.exe`. For live-reload development use `npm run tauri dev`.

Pushing a tag like `v0.1.0` to GitHub runs `.github/workflows/release.yml`, which builds the installer on a Windows runner and attaches it to a draft release.

## Using it

- **Library:** click a cover to select it, double-click or press Play to launch. "Add a game" accepts an `.exe` or any launcher link (`steam://…`, `uplay://…`).
- **Roblox join buttons:** on first launch Arcadia offers to take over `roblox-player:` links, so Join on roblox.com opens through Arcadia (with your FastFlags, mods and proxy applied). It re-claims the links each launch, since Roblox's own updater tends to take them back.
- **Mods:** drop files into the mods folder using Roblox's own layout, e.g. `content\sounds\ouch.ogg`. They're copied over Roblox's files on launch and the originals restored when you remove them.
- **Asset proxy:** turn on "Use the proxy when Roblox starts", add rules in a profile (paste asset IDs, pick Swap / Use a file / Remove), then launch Roblox from Arcadia. The cache is cleared on launch and exit so changes take effect. Profiles can be shared as JSON.
- **Coming from Fleasion:** Import → "Import everything from Fleasion" brings over every config in `%LOCALAPPDATA%\FleasionNT\configs` as its own profile, keeping which ones were on. You can also open or paste a single Fleasion config. Swap, remove, local-file and CDN-link rules all carry over, with their folders; replacement files are copied into Arcadia's own folder. Asset-type wildcards (such as `"Image"`) aren't supported yet and are skipped.

## Minecraft launcher

A Prism-style launcher built into Arcadia. Open it from the sidebar, or pop it out into its own window; **Accounts and setup** can add a desktop shortcut that opens only the launcher (`Arcadia.exe --minecraft`).

- **Instances** with Vanilla, Fabric, Forge or NeoForge. Each has its own mods, worlds, memory and Java arguments. Arcadia downloads the right Java for each Minecraft version.
- **Modpacks** from Modrinth and CurseForge, installed directly. No other launcher needed.
- **Import** `.mrpack` files, CurseForge `.zip` packs, Prism Launcher and MultiMC instances, and CurseForge app instances. Instances from Prism and the CurseForge app are found automatically.
- **Mods:** search and install mods, resource packs and shaders (dependencies included), turn mods on and off, and check that each file matches a Modrinth or CurseForge upload.
- **Security:** checks installed mods against known incidents (fractureiser, BleedingPipe, Log4Shell), looks for files fractureiser leaves behind, and recommends PipeBlocker for old Forge packs. Log4Shell is always patched.

Two keys are needed and are entered under **Accounts and setup**:

- **Azure client ID** for Microsoft sign-in. Register an app in Azure, then request Minecraft API access through Mojang's form. Everything except signing in and playing works without it.
- **CurseForge API key** for CurseForge search and downloads, free from the CurseForge developer console. Modrinth needs no key.

Some CurseForge mods don't allow other launchers to download them. Arcadia lists those on the instance with a link to download each by hand, as Prism does.

## Appearance

Four built-in looks, switchable from **Appearance** or the launcher:

- **Obsidian:** black floating panel over a light desk, grouped settings, white controls.
- **Gruvbox:** warm browns and orange, monospace throughout.
- **Frost:** frosted translucent glass over a wallpaper.
- **Dusk:** plum and peach.

On top of any look you can set a wallpaper (five built in, or your own image), its blur and dim, panel opacity, corners, font and accent color, and save the result as a named look. The window is frameless; the top bar holds the page dots, clock, proxy status and window buttons, and can be an island or a flush strip, or hidden.

The startup splash can be Short, Normal, Long or off (**Appearance → Startup**), and any click or key skips it. It's timed from when the logo first appears, so a fast start doesn't cut it short. If animation effects are off in Windows, the logo shows without moving for the same length of time; a switch there plays the animation anyway.

The sidebar is yours too: under **Appearance → Sidebar**, drag entries into any order, hide the ones you don't use (they stay one Ctrl+K away), or switch to icons only. The page dots in the top bar follow the same order.

## Marketplace and addons

Bigger optional tools ship as addons, so the sidebar only holds what you actually use. Open **Marketplace**, add one, and it gets its own sidebar entry; remove it the same way, optionally deleting what it saved. Each addon only loads once it's installed, keeps its data in its own folder (`%LOCALAPPDATA%\Arcadia\addons\<id>\`) and its secrets in Windows Credential Manager, and can't reach another addon's data.

### Translucent window

Makes Arcadia's background see-through and lets Windows blur what's behind it, like a riced Linux desktop. Pick **Acrylic** (real blur of the windows behind; can stutter while dragging on some Windows builds), **Mica**, **Mica dark** or **Mica Alt** (wallpaper-tinted, very light; Windows 11), or **Blur**. Tint colour and strength, panel opacity and "keep the wallpaper" are under Appearance. A window's transparency is fixed when it's created, so switching the addon on or off asks for a restart; with it off, Arcadia's windows are created solid exactly as before.

### Profile Studio (first official addon)

A sandbox for your Discord profile. Nothing in it changes your real profile.

- **Preview** the profile popout, the full profile, and a chat message with the member list, in Discord's dark or light theme, with Nitro profile colours, banner, avatar decoration, status and custom status. About me renders Discord markdown, spoilers included.
- **Pull** a profile from a username, user ID or `discord.com/users/…` link: avatar, banner, banner colour, decoration, names and account age. Discord only allows this with a bot token, so add your own app's token under *Import settings* (it goes in Credential Manager). Username lookups search servers your bot shares with that person and need **Server Members Intent** turned on. Without a token, user IDs are looked up through [Lanyard](https://github.com/Phineas/lanyard), which only knows people in its server. Bio, pronouns and profile colours aren't available to apps, so you add those yourself. Account tokens are never used: that's a self-bot, which Discord bans.
- **Suggested palettes**: three of them, worked out from the banner, the avatar, and both together. Colours are clustered in OKLab from only the pixels Discord actually shows (the avatar's circle, not its corners), the gradient pair is tuned for legibility, and each palette shows how readable white text is on it. Apply one, or click any swatch to use it for the top, bottom or banner colour.
- **Shelf**: save as many versions as you like, flip between them, compare a version with its saved copy side by side, and share one as JSON.
- Discord's own limits are enforced: 32-character names, 40 for pronouns, 190 for About me.

Press **Ctrl+K** anywhere for the launcher: a ring of quick actions, or type to search every installed game and action.

## How the proxy works

Same approach as Fleasion's "Roblox Env Proxy" mode:

1. Arcadia generates a local certificate authority once (`%LOCALAPPDATA%\Arcadia\proxy\ca.pem`).
2. Before launch it appends that CA to the Roblox copy's `ssl\cacert.pem`, and makes Roblox use that bundle by setting `DFFlagHttpUseEmbeddedCaCerts=False` in Roblox's cached server flags (`%TEMP%\Roblox\cache\flag_cache.dat`). Roblox ignores this flag in `ClientAppSettings.json` since its FastFlag allowlist, but reads its server-flag cache before going online. The proxy keeps the flag in every ClientSettings response, so it survives Roblox's refreshes. On a PC where Roblox has never run, that cache doesn't exist yet: the first launch passes traffic straight through, and replacements apply from the next one.
3. Roblox is started with `HTTPS_PROXY=http://127.0.0.1:58443`. No hosts-file edits, no admin rights, nothing else on the PC goes through it.
4. Only `assetdelivery.roblox.com`, `clientsettingscdn.roblox.com` and `clientsettings.roblox.com` are decrypted. Every other connection (login, chat, purchases) is a blind tunnel that is never read.
5. Asset rules rewrite `/v1/assets/batch` requests (swap the ID, drop it, or point its `location` at a local file Arcadia serves) and single-asset `/v1/asset?id=` requests. Live FastFlags are merged into the `PCDesktopClient` settings response.

The CA is never added to the Windows certificate store.

## Risk notes

- Asset swaps are client-side; other players don't see them.
- **Live FastFlags** (proxy route) bypass Roblox's local FastFlag allowlist. Roblox has said accounts doing this can be banned. They stay off until you accept that in the app.
- Modifying the Roblox client may break Roblox's Terms of Use; use at your own risk.

## Status

Verified in this build:

- 24 automated tests pass, including an end-to-end proxy test (client → proxy with CONNECT and TLS interception → fake Roblox upstream) covering swap, remove, local file, single-asset requests, live FastFlags, and proof that non-Roblox-asset hosts are tunneled without decryption.
- End-to-end installer test against a local fake Roblox CDN: manifest parsing, MD5 verification, retry, cache reuse, corrupt-package rejection, zip-slip protection, Roblox's backslash zip paths.
- The desktop app builds and runs with the real backend.

Written for Windows but **not yet run on Windows** (the build machine was Linux): registry-based scanners (Steam path, GOG, Ubisoft, Battle.net, EA), `roblox-player:` protocol registration, the multi-instance mutex, priority classes, and launching through each store. Expect a round of small fixes on first Windows run.

Minecraft: an end-to-end test against fake Mojang, Fabric and Modrinth servers creates a Fabric instance, installs it, builds the launch command, and imports a Modrinth pack, including rejecting tampered and path-escaping files. Forge and NeoForge run their official installers, which couldn't be exercised offline, and Microsoft sign-in needs a real approved client ID, so both still need a first run on Windows.

Not in this version yet (planned):

- Froststrap side: Discord Rich Presence, account manager, server region picker, game history, community mod downloads, custom bootstrapper themes, Studio support.
- Fleasion side: hold/edit/replay in the Inspector, cache viewer and asset browser, mesh/RBXM exports and converters, hosts-file mode, macOS and Linux (Sober).
- Library: Xbox / Microsoft Store games, playtime tracking for non-Roblox games.

## Project layout

```
crates/core/          all logic, no GUI deps (testable headless)
  src/library/        Steam (VDF parser), Epic, Battle.net, GOG, EA, Ubisoft, custom
  src/roblox/         deploy (CDN install), fflags, mods, cache, activity (log watcher)
  src/proxy/          ca, rules, hooks (pure rewrites), server (MITM proxy), inspector
  tests/              proxy_e2e.rs, install_e2e.rs
src-tauri/            desktop shell: commands, tray, single-instance, protocol handling
ui/                   Svelte 5 UI (runs in a browser with a mock backend: npm run dev)
```

Run tests with `cargo test -p arcadia-core`.
