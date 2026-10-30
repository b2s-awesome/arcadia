# Arcadia

A game hub for Windows that does three jobs in one small native app, plus a music player:

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
  - **Sections** in collapsible tabs: all games, favourites, recently played, your collections (with an emoji each), each launcher, and hidden games. Sort by name, recent or most played, and resize the covers.
  - **Playtime for every game:** Arcadia watches for the game's process to close after launching it. Steam's own playtime, and Roblox's and Minecraft's trackers, are shown too.
  - **Per game** (the brush button):
    - **Art:** covers, banners and logos from SteamGridDB (needs a free key in Settings → Library), or your own pictures.
    - **Launch:** launch options (passed through Steam for Steam games), plus scripts that run before the game starts and after it closes.
    - **Notes.**
    - **Details:** a rename, how long to beat (shown as progress against your playtime, with a link to HowLongToBeat), collections, and hiding the game.
  - **Big Picture:** a full-screen, sofa-friendly view with shelves. Use the arrow keys or a controller: A plays, Y favourites, LB/RB switch shelves, B leaves.
- **Roblox join buttons:** on first launch Arcadia offers to take over `roblox-player:` links, so Join on roblox.com opens through Arcadia (with your FastFlags, mods and proxy applied). It re-claims the links each launch, since Roblox's own updater tends to take them back.
- **Mods:** drop files into the mods folder using Roblox's own layout, e.g. `content\sounds\ouch.ogg`. They're copied over Roblox's files on launch and the originals restored when you remove them.
- **Asset proxy:** turn on "Use the proxy when Roblox starts", add rules in a profile (paste asset IDs, pick Swap / Use a file / Remove), then launch Roblox from Arcadia. The cache is cleared on launch and exit so changes take effect. Profiles can be shared as JSON.
- **Coming from Fleasion:** Import → "Import everything from Fleasion" brings over every config in `%LOCALAPPDATA%\FleasionNT\configs` as its own profile, keeping which ones were on. You can also open or paste a single Fleasion config. Swap, remove, local-file and CDN-link rules all carry over, with their folders; replacement files are copied into Arcadia's own folder. Asset-type wildcards (such as `"Image"`) aren't supported yet and are skipped.

## Roblox+

The Roblox page is a set of collapsible vertical tabs:

- **Play:** pick who to play as, paste any game link or place ID, or jump back into a recent game.
- **Accounts:** sign in on Roblox's own page in a private window. Arcadia only keeps each login in Windows Credential Manager, split across a few entries because one entry is too small for it. Logins are never written to a file or a log, and only ever sent to roblox.com. The starred default account is used by Play, join links from roblox.com and game shortcuts. Several accounts can run at once with *Multiple Roblox windows*.
- **Games:**
  - A library of every game you've played through Arcadia, with total playtime, sessions, favourites and desktop shortcuts.
  - A day-by-day list of visits, showing server region and account. You can rejoin the exact server or copy a join link for it.
- **Servers:** a game's public servers with player count and ping. *Find regions* asks Roblox for each server's address and looks up the city, which needs a signed-in account. You can then filter by region and join one.
- **Rejoin & windows:** auto-rejoin after an idle kick or a lost connection. There is a countdown you can cancel, an option to go back to the same server, and it stops after five rejoins in half an hour.
- **Client & updates:** pause updates on the current version, pick a channel, or reinstall.
- **FastFlags:**
  - Presets that only use flags on Roblox's allowlist.
  - Profiles you can save and swap between.
  - Your own flags, each one marked *ignored* when Roblox won't read it.
- **Mods:**
  - **Mod studio:** Arcadia-drawn cursor and shift-lock crosshair styles in any colour and size, or your own pictures, plus death sound and font. It has a live preview and presets.
  - **Files & cache:** the mods folder and cache clearing.

## Minecraft launcher

A Prism-style launcher built into Arcadia. Open it from the sidebar, or pop it out into its own window; **Accounts and setup** can add a desktop shortcut that opens only the launcher (`Arcadia.exe --minecraft`).

- **Instances** with Vanilla, Fabric, Forge or NeoForge. Each has its own mods, worlds, memory and Java arguments. Arcadia downloads the right Java for each Minecraft version.
- **Modpacks** from Modrinth and CurseForge, installed directly. No other launcher needed.
- **Import** `.mrpack` files, CurseForge `.zip` packs, Prism Launcher and MultiMC instances, and CurseForge app instances. Instances from Prism and the CurseForge app are found automatically.
- **Mods:** search and install mods, resource packs and shaders (dependencies included), turn mods on and off, and check that each file matches a Modrinth or CurseForge upload.
- **Each instance** has collapsible tabs:
  - **Content:** mods, updates, resource packs, shaders and add content.
  - **Play:** worlds, servers and screenshots.
  - **Logs & crashes.**
  - **Settings:** general, Java & launch, and export & share.
- **Updates:** finds newer Modrinth versions of mods, resource packs and shaders for the instance's Minecraft version and loader. Files are matched by SHA-1, so this works whoever installed them. Update one, or all.
- **Worlds:** game mode, version, size and seed, with one-click zip backups, restore (the current world is backed up first) and delete-with-backup.
- **Servers:** the instance's Multiplayer list, with live player counts, message of the day and ping. **Join** starts the game and connects straight away, using Quick Play on 1.20+ and `--server` before that. An instance can also join a server every time it starts.
- **Screenshots** in a grid with a viewer, and **resource and shader packs** you can turn on and off, add or remove.
- **Logs & crashes:** live output, saved logs and crash reports, with search and filters. When the game crashes, Arcadia explains it in plain words and gives a fix. It recognises:
  - missing or wrong dependencies (Fabric and Forge/NeoForge)
  - incompatible or duplicate mods
  - running out of memory
  - the wrong Java
  - graphics drivers
  - mixin failures
  - suspected mods
  - missing libraries
- **Java:** automatic by default, or pick Java 8, 16, 17 or 21 from Mojang, or your own `javaw.exe`.
- **Export:** a `.mrpack` that Arcadia, Prism and the Modrinth app import. Modrinth files are linked rather than packed in, or everything goes inside for an offline copy. You choose whether to include config, packs, options and worlds.
- **Skins:**
  - A skin library with a 3D preview you can drag to turn, outer-layer and walking toggles, and classic or slim arms.
  - Add skin files, or copy any player's skin by name.
  - **Wear** a skin and pick your **cape** through Mojang's own profile API.
- **Security:** checks installed mods against known incidents (fractureiser, BleedingPipe, Log4Shell), looks for files fractureiser leaves behind, and recommends PipeBlocker for old Forge packs. Log4Shell is always patched.

Two keys are needed and are entered under **Accounts and setup**:

- **Azure client ID** for Microsoft sign-in. Register an app in Azure, then request Minecraft API access through Mojang's form. Everything except signing in and playing works without it.
- **CurseForge API key** for CurseForge search and downloads, free from the CurseForge developer console. Modrinth needs no key.

Some CurseForge mods don't allow other launchers to download them. Arcadia lists those on the instance with a link to download each by hand, as Prism does.

## Music

A music player for the songs on your PC, built into Arcadia (inspired by Riff and MyRiff).

- **Native audio engine** in Rust: MP3, AAC/M4A, ALAC, FLAC, WAV, Ogg Vorbis and Opus (a pure-Rust Opus decoder). Playback is **gapless**: the next song is decoded straight onto the end of the current one, and MP3/AAC encoder padding is trimmed, so live albums and DJ mixes don't click between tracks. Songs that match the device's sample rate pass through without resampling; others are resampled with a high-quality FFT resampler. It follows the Windows default output (plug in headphones and it moves), or stays on a device you pick.
- **Library:** add folders, and Arcadia reads tags and cover art (embedded, or `cover.jpg`/`folder.jpg` beside the files), keeps a small database and only rereads files that changed. An unplugged drive doesn't wipe its songs. Albums, artists, songs, favourites, play counts and search.
- **Playlists and queue:** create, rename, reorder by dragging; Play next / Add to queue from any song; shuffle keeps the current song playing; repeat all or one. The queue and position are restored next time.
- **Two layouts**, switched under Customise: **Tonal** (covers, colour taken from what's playing, player bar top or bottom) and **Tiled** (panes with gaps, keyboard first: space, n/p, s, r, q, /). Customise also sets cover size, compact rows, and which Home sections show, in what order.
- **Everywhere:** the top bar shows what's playing with play/pause and next, and Ctrl+K can play/pause or skip.
- Keys: space play/pause, ←/→ seek 5 s, Shift+←/→ previous/next, ↑/↓ volume, / search.
- **Media keys and the Windows media overlay:** the keyboard's play/pause, next and previous keys work from anywhere, and the volume flyout and lock screen show the song, artist and cover with working controls.

Discord presence comes in a later update. Lyrics, stats and the other Spicetify-style extras are addons (below).

### Streaming & downloads (addon)

Add it from the Marketplace and Music gets **Online** and **Downloads** tabs.

- **Find music:** search YouTube Music or SoundCloud, or paste a link to a song, album, playlist or set from anywhere [yt-dlp](https://github.com/yt-dlp/yt-dlp) supports (Bandcamp and more).
- **Play (fetch-then-play):** a song is fetched in a second or two, then played by Arcadia's own engine, so it's gapless and you can seek anywhere. The next song in the queue is fetched ahead. Streamed songs live in a cache (1 GB by default, least recently played cleared first) and don't clutter the library until you download them.
- **Download:** M4A (default: exactly what the site sends, untouched), MP3 320 (re-encoded so it plays anywhere; tiny loss, bigger files) or FLAC (same sound as the source, 3–5× bigger). Files land in `Music\Arcadia Downloads\Artist\Album`, tagged, with covers, and appear in your library. On SoundCloud, the artist's original upload is used when they allow downloads.
- **Linked playlists:** save a playlist/album/set link as an Arcadia playlist, stream or download it, and check it for new songs later.
- **Tools:** yt-dlp (~18 MB) is fetched from its official GitHub release and checked against the release's published SHA-256 before use; ffmpeg (~87 MB, from yt-dlp's FFmpeg builds, also checksum-checked) only if you want MP3/FLAC or embedded covers. Copies you already have are found on PATH, or you can point to them.

Only fetch what you're allowed to. Subscriber-only audio (e.g. SoundCloud Go+ tracks, which give non-subscribers a 30-second preview) and DRM-protected audio aren't fetched.

## Appearance

Four built-in looks, switchable from **Appearance** or the launcher:

- **Obsidian:** black floating panel over a light desk, grouped settings, white controls.
- **Gruvbox:** warm browns and orange, monospace throughout.
- **Frost:** frosted translucent glass over a wallpaper.
- **Dusk:** plum and peach.

On top of any look you can set a wallpaper (five built in, or your own image), its blur and dim, panel opacity, corners, font and accent color, and save the result as a named look. The window is frameless; the top bar holds the page dots, clock, proxy status and window buttons, and can be an island or a flush strip, or hidden.

The startup splash can be Short, Normal, Long or off (**Appearance → Startup**), and any click or key skips it. It's timed from when the logo first appears, so a fast start doesn't cut it short. If animation effects are off in Windows, the logo shows without moving for the same length of time; a switch there plays the animation anyway.

The sidebar is yours too: under **Appearance → Sidebar**, drag entries into any order, hide the ones you don't use (they stay one Ctrl+K away), or switch to icons only. The page dots in the top bar follow the same order.

Appearance is split into collapsible sections: Look, Feel, Layout, Keyboard, Profiles & backup, Name & startup, and Addons.

- **Home:** a dashboard of widgets. Press **Customise** to add, remove, resize (S/M/L) and drag them around. Arcadia can open on Home. The widgets are:
  - Greeting
  - Continue playing
  - Now playing
  - Roblox
  - Minecraft
  - Most played
  - Pinned games
  - Listening
  - Notes
  - Clock
- **Motion & sound:**
  - Motion can be full, calm (slower, and looping animations hold still) or off, with an animation speed control.
  - Interface sounds come in soft, clicky or retro sets, made on the fly with Web Audio, with a volume control.
- **Fonts & icons:**
  - Text size (85–130%).
  - Any installed font, with an install check and a preview.
  - Icon line weight and round or square line ends.
- **Keyboard:** change every shortcut by pressing the new keys. It warns when two actions share a combo. Shortcuts marked **Anywhere** work as global hotkeys while you're in a game or another app. They cover:
  - search and launch, Big Picture and bringing Arcadia to the front
  - each page
  - music play/pause, next, previous and volume
  - Play Roblox, and your last game
- **Profiles:** save your whole setup (look, layout, Home, keys, feel, addon settings) under a name and switch between setups in a click.
- **Backup & sync:**
  - Back up to one `.arcadia` file and restore it, choosing which parts. What you had before is backed up first.
  - Or pick a sync folder (OneDrive, Dropbox…). Arcadia keeps the latest copy there and offers it on your other PCs.
  - Logins, addon secrets and, unless you ask, API keys never leave the PC.

## Marketplace and addons

Bigger optional tools ship as addons, so the sidebar only holds what you actually use. Open **Marketplace**, add one, and it gets its own sidebar entry; remove it the same way, optionally deleting what it saved. Each addon only loads once it's installed, keeps its data in its own folder (`%LOCALAPPDATA%\Arcadia\addons\<id>\`) and its secrets in Windows Credential Manager, and can't reach another addon's data.

### Spicetify-style addons

The things people install Spicetify for, rebuilt natively. Add them one by one, or all at once with the **Spice rack** bundle. The visual ones follow your Music layout (Tonal or Tiled), or you can pick a look in their settings, including Liquid Glass.

- **Full App Display:** what's playing, full screen. *Ambient* (blurred cover, huge title), *Tiled* (panes with a live spectrum visualiser from the audio engine, which only runs while it's on screen) or *Liquid Glass* (big clock and a glass card). Shows the current lyric line with Lyrics Plus. Press **F** in Music; Esc leaves, double-click toggles full screen.
- **Lyrics Plus:** a Lyrics tab in Music and in Now Playing. Lyrics come from a `.lrc` beside the song, the song's own tags, your own timing, or [LRCLIB](https://lrclib.net) (free and open; can be turned off), and scroll in time, word by word for enhanced LRC. Click a line to jump there, nudge the timing per song, or **Fix timing** by tapping along. **Pop out** opens a small always-on-top glass window; the **Desktop bar** floats one line over everything, games included, and lets clicks through. Optionally saves found lyrics as `.lrc` beside your songs. **Romanise** shows Korean, Japanese kana, Chinese (pinyin), Cyrillic and Greek in Latin letters (offline); **Translate** adds a translation under each line from [MyMemory](https://mymemory.translated.net) (free, no account, cached per song).
- **Stats:** listening time, top artists/songs/albums, when in the day you listen, a 26-week heatmap, sources and formats, skips, streaks and new finds, for 4 weeks, 6 months, a year or all time. Arcadia keeps a play log in the music library (what, when, how long, skipped or not); nothing leaves the PC. Recap cards, terminal tables or glass widgets with a daily goal ring.
- **Trash Bin + Auto Skip:** trash a song or an artist from its menu and Arcadia passes over them when it moves on by itself (gapless included). Auto skip can also pass over explicit songs (iTunes advisory tag, or "(Explicit)" in the title), anything under 30 seconds, live versions and remixes. Picking a song yourself always plays it.
- **Shuffle+:** right-click the shuffle button for *Spread artists*, *Fewer repeats* (recently played songs drift later), *Favour less played* or *True random*. The current song keeps playing when you switch.
- **Loopy Loop + Bookmarks:** sample-accurate A–B loops done in the audio engine (`[` sets A, `]` sets B, `\` stops, or right-click the seek bar), and bookmarks (`M`) that show on the seek bar and play from the moment.
- **Sort & Play:** a Sort & Play button on songs, albums, artists and playlists (most/least played, recently added/played, longest/shortest, title, artist, favourites first, album shuffle), plus queue tools: remove duplicates, clear what's played, shuffle what's left.
- **Liquid Glass:** frosted or smoked glass panels with lit edges and a sheen, iOS 27 style, over your wallpaper (or your real desktop with Translucent window), for the whole app or just Music. Blur, tint and edge light are adjustable; text colours follow the glass so it stays readable.
- **Themes & CSS snippets:** six theme packs on top of your Look (Peach Rice, Gruvbox Hard, Nord Frost, Rosé Night, Paper, OLED), one-click snippets, and your own CSS for the whole app or Music only, applied as you type. Save your CSS as a snippet. Hold **Shift** while Arcadia starts to skip snippets and CSS for that run. Also in the Marketplace's Themes tab.

### Bundles

**Marketplace → Bundles** has official bundles (Spice rack, Listening tools, Eye candy, Rice kit), each installable in one click with anything you don't want unticked. Make your own from the addons you have, optionally with their settings and your theme and CSS, and share it as a code (`arc-bundle:v1:…`) or a `.arcbundle` file. Adding someone's bundle shows every addon and setting it changes first. Bundles only name addons from the Marketplace and carry settings; they can't carry code.

### Translucent window

Makes Arcadia's background see-through and lets Windows blur what's behind it, like a riced Linux desktop. Pick **Acrylic** (real blur of the windows behind; can stutter while dragging on some Windows builds), **Mica**, **Mica dark** or **Mica Alt** (wallpaper-tinted, very light; Windows 11), or **Blur**. Tint colour and strength, panel opacity and "keep the wallpaper" are under Appearance. A window's transparency is fixed when it's created, so switching the addon on or off asks for a restart; with it off, Arcadia's windows are created solid exactly as before.

### Discord status (Rich Presence)

Shows what you're doing in Arcadia on your Discord profile:

- **Roblox:** the game, its icon and the server's region, plus a "View game" button and an optional "Join my server" button.
- **Minecraft:** the instance, version and server.
- **Library games:** the game and its cover.
- **Music:** "Listening to", with a progress bar.
- **Browsing:** an optional "Browsing Arcadia" when nothing else is going on.

Games take priority over music.

**Setup:**
1. Create a free app called **Arcadia** at the [Discord Developer Portal](https://discord.com/developers/applications).
2. Under **Rich Presence → Art Assets**, upload `arcadia`, `game`, `cube`, `music` and `paused`.
3. Paste the app's **Application ID** into the addon's settings and click **Connect**.

It talks only to the Discord app on your PC over its local pipe: no login, token or bot. You choose what's shared, and can hide game names.

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
- Auto-rejoin only reconnects you. It doesn't move your character, so it isn't an anti-AFK tool, and some games still kick idle players by design.

## Status

Verified in this build:

- 24 automated tests pass, including an end-to-end proxy test (client → proxy with CONNECT and TLS interception → fake Roblox upstream) covering swap, remove, local file, single-asset requests, live FastFlags, and proof that non-Roblox-asset hosts are tunneled without decryption.
- End-to-end installer test against a local fake Roblox CDN: manifest parsing, MD5 verification, retry, cache reuse, corrupt-package rejection, zip-slip protection, Roblox's backslash zip paths.
- The desktop app builds and runs with the real backend.

Written for Windows but **not yet run on Windows** (the build machine was Linux): registry-based scanners (Steam path, GOG, Ubisoft, Battle.net, EA), `roblox-player:` protocol registration, the multi-instance mutex, priority classes, and launching through each store. Expect a round of small fixes on first Windows run.

Minecraft: an end-to-end test against fake Mojang, Fabric and Modrinth servers creates a Fabric instance, installs it, builds the launch command, and imports a Modrinth pack, including rejecting tampered and path-escaping files. Forge and NeoForge run their official installers, which couldn't be exercised offline, and Microsoft sign-in needs a real approved client ID, so both still need a first run on Windows.

Not in this version yet (planned):

- Froststrap side: Discord Rich Presence, community mod downloads, custom bootstrapper themes, Studio support.
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

Run tests with `cargo test -p arcadia-core -p arcadia-audio`.
