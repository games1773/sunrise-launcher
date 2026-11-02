# Sunrise Launcher

A small desktop launcher for **Horizon** — Minecraft: Java Edition **26.3**, rewritten
from scratch in Rust by the
[horizon-rust/horizon](https://github.com/horizon-rust/horizon) project.

Sunrise plays as an **offline account** by default: you choose a name and Sunrise hands it
to the game. You can also give it a **Microsoft session** (optional) so online-mode servers
accept you. It installs Horizon builds, prepares the game files from **your own** copy of
Minecraft: Java Edition 26.3, and starts the game with the arguments Horizon's
[Launch API](https://github.com/horizon-rust/horizon/blob/main/docs/api/launch.md)
describes. It ships no Minecraft code, assets or data.

> Unofficial. Not affiliated with, endorsed by or connected to Mojang Studios, Microsoft
> or the Horizon project. Minecraft is a trademark of Mojang Synergies AB.

## What it does

- **Offline account** — a player name (3–16 letters, digits or `_`, vanilla's rule) is
  handed to the game as `--name`. It is saved, so the next launch uses it again.
- **Microsoft account, optional** — press **Sign in**, type the code Microsoft shows you at
  `microsoft.com/link`, and you are done: Sunrise walks the Microsoft → Xbox Live → Minecraft
  chain itself, checks ownership, and keeps a session that renews from its refresh token. A
  session JSON can still be imported instead. Either way it reaches the game on standard
  input with `--session-stdin`, never on the command line. Switch back to Offline at any
  time; Play never requires a session.
- **Join a server** — an optional `host[:port]` is passed as `--quickPlayMultiplayer`, so
  the game connects at start. Offline-mode servers and LAN worlds accept an offline name;
  online-mode servers accept a Microsoft account.
- **Builds** — lists the published builds of `horizon-rust/horizon` from GitHub releases,
  downloads the archive for your platform, checks it against the release's published
  SHA-256, and unpacks it into its own folder.
- **Prepare** — runs the build's own `horizon-prepare` against your official launcher's
  `.minecraft` (or the folder you pick) with Java 25. There is no server jar to find: the
  first preparation needs the 26.3 server jar, so Sunrise downloads it from Mojang once,
  checks it against the SHA-1 Horizon pins in the build's `research-lock.json`, and keeps it
  in its cache. Sounds can be copied too; without them the game is silent.
- **Play** — starts `horizon-client` with `--resources`, `--game-dir`, `--home`, window
  size and fullscreen. Optional flags (`--world`/`--seed`, `--render-distance`,
  `--creative`) are only passed when that build's own `--help` lists them.
- **One window** — a sunrise hero banner with your offline profile and playtime, the
  account and server fields, one big Play button, and a tabbed panel (Setup, Launch,
  Console) for everything else. There are no JVM or mod tabs because Horizon is a
  native Rust client: it has no JVM to tune and no Java mods to load.

## Requirements

| What | Why |
| --- | --- |
| Minecraft: Java Edition **26.3** played once in the official launcher | Horizon prepares its resources from your own game files; it never downloads them. Sunrise fetches only the public 26.3 **server jar** (62 MB, once) after your own client jar checks out against Horizon's pin |
| **Java 25** (JAVA_HOME, PATH, or the launcher's own runtimes) | `horizon-prepare` runs Mojang's own exporters |
| A GPU with Vulkan or OpenGL 3.3 / DX12 | Horizon's renderer |

Sunrise supports Windows x64, Linux x86-64, macOS arm64 and macOS x64 — the platforms
Horizon publishes builds for.

## Run it

```sh
cargo run --release --bin sunrise        # the window
cargo run --release --bin sunrise-cli -- status
```

The **Setup** tab is where a new install starts: install a build, point at your
Minecraft, press **Prepare game files** once. Then type a name and press **Play**. The
server jar downloads itself; the Setup tab shows it arriving and remembers it.

## Offline accounts, and what they can join

Minecraft's offline mode is what a name and no session give you:

- **Singleplayer and LAN worlds**: work.
- **Offline-mode servers** (`online-mode=false`) and most LAN-hosted games: the server
  looks the player up by name, so they join.
- **Online-mode servers**: still refuse, because they check the session with Mojang. That
  is the server's authentication, not a Horizon restriction — Sunrise cannot and does not
  forge sessions.

Preparing the game still needs your own Minecraft 26.3 install. Sunrise never downloads
Minecraft itself — only the public server jar the preparer insists on, and only after your
own pinned client jar is present.

## The optional Microsoft account

Switch the account to **Online** and press **Sign in** — there is nothing to set up first.
Sunrise asks Microsoft for a code and shows it in the launch card, in the same place the Play
button lives, with one big button: **Open microsoft.com/link**, which also copies the code.
Paste it on the page, approve, and the launcher keeps a session: no JSON files, no
registration, no other launcher involved. The browser is deliberately left for you to open,
so the code is on screen before anything can take your attention. The same flow runs headless
with `sunrise-cli account signin`.

That sign-in goes Microsoft → Xbox Live → Minecraft, and afterwards `microsoft::refresh` mints
fresh tokens from the stored refresh token, so the session renews itself instead of dying after
a day. Play renews before it starts when the token is over 20 hours old. **Sign out** forgets
both the session and the refresh token.

**Nothing to register.** The sign-in runs on Microsoft's consumer service and uses
Minecraft's own public title id, `00000000402b5328`, as the application: that id is
Minecraft's own registration, already allowed to reach Xbox Live and Minecraft's services, so
there is no app to create, no secret to keep and no approval to wait for. Sunrise never
borrows another launcher's id, and it never sees your password — you approve the code in your
own browser and the launcher only receives the token.

If you already have a session, importing still works: the account panel's **Import file** and
**Paste** buttons, or `sunrise-cli account import FILE`. The shape is the one Horizon's Launch
API documents:

```json
{"name": "Alex", "uuid": "069a79f444e94726a5befca90e38aaf5", "access_token": "…", "user_type": "msa", "xuid": "…"}
```

Sunrise asks `api.minecraftservices.com` who the session belongs to, whether it is still
valid, and whether the account owns Java Edition. A refused token is reported as expired,
not saved. An imported session cannot be renewed — it has no refresh token — so it lasts the
24 hours of its Minecraft token and then has to be imported again. Once signed in, Play hands
the session to the game on standard input (`--session-stdin`) and stops sending `--name`,
because the session carries its own name. The session lives in `sunrise.json` (written `0600`
on macOS and Linux; under your `%APPDATA%` on Windows), and **Sign out** deletes it.

## Command line

The same engine without a display — handy for scripts and for checking a setup:

```sh
sunrise-cli status                     # what is ready, what is missing
sunrise-cli builds [--refresh]         # installable builds for this platform
sunrise-cli install [TAG|latest]       # download, verify, unpack
sunrise-cli prepare [--sounds]         # prepare the selected build (fetches the server jar)
sunrise-cli play --name Alex --server 192.168.1.20:25565
sunrise-cli account                    # which account would play, and when it was checked
sunrise-cli account signin [APP_ID]    # sign in with Microsoft, code typed in the browser
sunrise-cli account refresh            # renew the saved sign-in from its refresh token
sunrise-cli account import FILE        # check a session JSON with Mojang and keep it
sunrise-cli account online|offline     # switch, without deleting the session
sunrise-cli instances                  # list game folders
```

Options: `--name NAME`, `--server HOST[:PORT]`, `--session FILE` (use a session for this
run), `--offline` (ignore the saved one), `--data-dir DIR`, `--minecraft-dir DIR`,
`--java-home PATH`, `--instance ID`, `--fullscreen`, `--world NAME`.

## Where things live

`SUNRISE_DATA_DIR`, else the platform data folder (`%APPDATA%\Sunrise` on Windows,
`~/Library/Application Support/Sunrise` on macOS, `~/.local/share/Sunrise` on Linux):

```
sunrise.json              settings (offline name, paths, account) and game folders
cache/26.3-server.jar     the server jar, downloaded once from Mojang and verified
builds/<tag>/             one Horizon build (horizon-client, horizon-prepare, ...)
builds/<tag>/.local/      its prepared resources (horizon-client/26.3/)
instances/<id>/           game folder: saves/, options.txt, logs/, mods/
downloads/                in-flight downloads
builds-cache.json         the build list, cached for ten minutes
```

The window works in the first game folder; `sunrise-cli --instance` can address others.

## How it talks to Horizon

The published Launch API draft and the shipped Nightly do not fully agree, so Sunrise
follows what the build actually accepts:

- `horizon-client --launch-api` is read at install time to show the Launch API version and
  the game version.
- `horizon-client --help` is the gate for optional flags. `--name`, `--render-distance`,
  `--world`, `--seed` and `--creative` work in the current build even though
  `--launch-api` does not list them, so Sunrise asks the build rather than assuming.
- `horizon-prepare` in the current build accepts `--prepare`, `--root`, `--minecraft-dir`,
  `--server-jar` and the `sounds` subcommand — not the `--check`/`--output`/`--progress`
  flags the API draft describes. Sunrise therefore passes `--root <build>` and decides
  "prepared" by looking for `horizon-resources.json` in
  `<build>/.local/horizon-client/<version>/`. That build also only takes a subcommand as the
  first argument, so `sounds` is passed before `--root`.
- Exit codes are reported as the API defines them: 0 done, 1 failed or crash, 2 a refused
  command line, 101 a panic (the crash report is named on the last stderr line).

## Known limits

- The Microsoft account needs an Xbox profile (free at xbox.com); without one Xbox Live
  refuses the sign-in, and a child account has to be allowed by its Microsoft family. Offline
  names can't join online-mode servers, by the server's own check.
- No build update checker: **Builds** shows what is published; nothing updates behind you.
- No mod/resource-pack browser and no server hosting.
- No "stop the game" button; the game is a normal child process the launcher follows.
- Installing the moving `nightly` tag replaces that folder, so its resources have to be
  prepared again.
- One game folder in the window; the CLI can pick another instance.

## Layout

```
src/lib.rs        events, process spawning with captured output, argument splitting
src/builds.rs     GitHub releases, download, SHA-256 check, unpack, the pinned hashes
src/detect.rs     Minecraft 26.3 install and Java 25 discovery
src/mojang.rs     the pinned server jar: Mojang's manifest, SHA-1 check, cache
src/auth.rs       the optional session: parse, check with Mojang, handover JSON
src/prepare.rs    horizon-prepare invocations and readiness
src/launch.rs     Launch API probing, --name/--session-stdin/--quickPlayMultiplayer, spawning
src/config.rs     settings, account mode, session, game folders
src/paths.rs      data folder layout
src/bin/sunrise_cli.rs   the headless driver
```

The window (`src/bin/sunrise/`) is split by what each file is responsible for.
State has one owner; nothing else writes it.

```
main.rs            the viewport, the app icon, and the module list
app.rs             App: the state, its accessors, and the frame loop
events.rs          what the background jobs report, applied to that state
work.rs            starting the jobs: detect, builds, install, prepare, play, sessiontheme.rs           the palette, the egui style, the pixel font, the hero artwork
widgets.rs         the painted controls: cards, inputs, buttons, toggles, icons
logo.rs            the mark, drawn from the pixel grid the .exe icon is built from
logo_grid.rs       that 16x16 grid, shared by the window and build.rs
panels/mod.rs      the two columns and the tab bar
panels/hero.rs     the banner: the scene, the chips, the account, the playtime
panels/account.rs  the account and server row
panels/play.rs     the play card and the newest-log strip
panels/setup.rs    the Setup tab
panels/launch.rs   the Launch tab
panels/console.rs  the Console tab
panels/builds.rs   the Builds window
panels/title_bar.rs  brand, build badge, window buttons
panels/toast.rs    the transient notice
```

Data flows one way: a panel reads the state and returns an `Action`; `app`
decides what it means, `work` starts the job, and `events` writes the answer back
into the state that the next frame draws. Panels never call the engine directly,
and the engine modules never know a window exists.

## Credits

The interface uses [Monocraft](https://github.com/IdreesInc/Monocraft) by Idrees
Hassan, a pixel font in the spirit of the game's own lettering, under the SIL
Open Font License 1.1 (`assets/fonts/OFL.txt`).

The Sunrise mark is 16x16 pixel art defined once in `logo_grid.rs`. `build.rs`
rasterises it into `assets/sunrise.ico`, links that into the executables with
`windres`, and the window draws the same grid for its title bar and icon.
