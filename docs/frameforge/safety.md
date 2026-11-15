# Safety and rollback

## What FrameForge will never do

These are not settings. No code path exists for them.

- Set a process to REALTIME priority.
- Overclock a GPU or change its voltage by itself.
- Disable Windows Defender, VBS / Core Isolation, other security features, Windows Update or Windows services.
- Edit boot configuration (BCD), HPET or timer settings.
- Touch anti-cheat components, read or write game memory, inject code, or modify game executables.
- Apply registry packs from the internet. Every registry value it writes is a documented, per-user Windows setting named in the action's "source" text.
- Send a GPU a value outside the range its driver reports.
- Promise a frame-rate gain. Actions state what they change and their trade-off; measured results come from benchmarks.
- Run as administrator. The app runs as the signed-in user; the changes it makes are per-user settings, the user's own power plan and the user's own processes.
- Run a shell command, or anything named in a profile or preset file. Process launches (PresentMon console application, Profile Inspector) use validated paths and fixed arguments.

## Levels

| Level | Meaning | Examples |
|---|---|---|
| Safe (1) | Standard Windows options, no known downside | Switch to the existing High performance plan; ask a background app to close |
| Advanced (2) | Real trade-offs, explained per action | Arcadia Gaming power plan and its settings; Game Mode; turn off background recording and Game Bar capture; stop a startup app; per-game GPU preference; end a process; process priority; vendor 3D driver controls |
| Expert (3) | Changes hardware limits inside the driver's range | Intel sustained power limit and temperature limit |

The Optimizer level caps what can be applied. Expert additionally needs an explicit acknowledgement stored in the settings. Nothing is applied without the user choosing it and confirming a list that shows current value, new value, risk, trade-off and whether a restart is needed.

## How a change is applied

Every change, from any page, goes through `optimizer::Engine` as part of a **session**:

1. The action checks it applies to this PC (`applicable`). If not, it is refused with the reason.
2. The current value is read and the session is written to disk **before** anything changes (write-ahead record).
3. The change is applied.
4. The value is read back and compared with what was intended.
5. If any step fails, every change already made in the session is rolled back and the session is marked failed.

A session file holds, per change: action id, parameters, value before, value intended, value after, status, timestamps and any error.

## Revert

- **One change or a whole session** from Optimizer, History. Revert writes the recorded "before" value and reads it back.
- **Temporary changes** (process priority) end with the process. When a process exits, its change record is closed, so a reused process id is never mistaken for the old process.
- **Power plan**: the Arcadia Gaming plan is a copy of an existing plan. Reverting switches back to the plan that was active and deletes the copy.
- **Startup apps**: the exact command line is saved and written back.
- **NVIDIA driver profiles**: each apply or import takes a `.nip` snapshot first; restore it from NVIDIA Profiles, Backups.
- **AMD / Intel controls and presets**: recorded as optimiser sessions like any other change.

If a revert cannot restore a value (for example the driver refuses it), the change is marked "revert failed" with the error and stays visible in History.

## Protected processes

The process manager never offers close, end or priority for: Windows core processes, anything running as a system account or from the Windows system folders, security software, anti-cheat services, GPU and audio driver services, PresentMon's own service, and Arcadia itself. The list and rules are in `processes.rs` (`PROTECTED_NAMES`, `protection_reason`). A game process cannot be closed from the background-apps list. Ending a process is a separate, confirmed action that warns about unsaved work; closing first asks the app's windows to close.

## Game profiles

A profile belongs to one executable file name (plus the library game id when both the profile and the running process have one, so two games with the same file name are not confused). Matching by name means a game update or a moved install folder does not orphan the profile. Protected executables can never have a profile.

A profile can hold only:

- optimiser action ids from the registry (never `process.*` or `gpu.control`, which carry a process or device id),
- a priority class (Normal, Above normal, High; never Realtime),
- an Arcadia GPU preset,
- switches for Live Monitor and benchmark recording.

Applying a profile hands these to the optimiser and the preset layer, so the rules above apply unchanged.

**Automatic activation is opt-in twice**: the global setting and the profile's own switch, both off by default. An automatic run never goes above the level approved when the profile was saved, nor above the Optimizer level. Anything left out is reported in Recent activity. Imported profiles arrive switched off and unapproved; nothing in them runs until the user opens and saves them.

**When the game exits**: process priority goes back by itself; system changes are restored if "Put system changes back when the game exits" is on; GPU driver settings from a preset are kept, because they are intentional and persistent. Revert on the profile undoes everything, the driver settings included.

Known limit: the list of applied profiles is kept in memory. If Arcadia is closed while a game runs, the exit-time restore does not happen; the changes remain recorded and can be reverted from Optimizer, History.

## Logging

Optimiser sessions (`history\`), NVIDIA snapshots with their index (`nvidia\backups\`) and the lifecycle activity list record every system and driver change FrameForge makes.
