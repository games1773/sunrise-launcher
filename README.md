# Sunrise Launcher

A small desktop launcher for **Horizon** — Minecraft: Java Edition **26.3**, rewritten
from scratch in Rust by the
[horizon-rust/horizon](https://github.com/horizon-rust/horizon) project.

Sunrise plays as an **offline account**: you choose a name, Sunrise launches Horizon with
it, and there is no Microsoft sign-in anywhere. It installs Horizon builds, prepares the
game files from **your own** copy of Minecraft: Java Edition 26.3, and starts the game with
the arguments Horizon's [Launch API](https://github.com/horizon-rust/horizon/blob/main/docs/api/launch.md)
describes. It ships no Minecraft code, assets or data.

> Unofficial. Not affiliated with, endorsed by or connected to Mojang Studios, Microsoft
> or the Horizon project. Minecraft is a trademark of Mojang Synergies AB.

## What it does

- **Offline account** — a player name (3–16 letters, digits or `_`, vanilla's rule) is
  handed to the game as `--name`. It is saved, so the next launch uses it again.
- **Join a server** — an optional `host[:port]` is passed as `--quickPlayMultiplayer`, so
  the game connects at start. Offline-mode servers and LAN worlds accept an offline name.
- **Builds** — lists the published builds of `horizon-rust/horizon` from GitHub releases,
  downloads the archive for your platform, checks it against the release's published
  SHA-256, and unpacks it into its own folder.
- **Prepare** — runs the build's own `horizon-prepare` against your official launcher's
  `.minecraft` (or the folder you pick), with your 26.3 server jar and Java 25. Sounds can
  be copied too; without them the game is silent.
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
| Minecraft: Java Edition **26.3** played once in the official launcher | Horizon prepares its resources from your own game files; it never downloads them |
| **Java 25** (JAVA_HOME, PATH, or the launcher's own runtimes) | `horizon-prepare` runs Mojang's own exporters |
| The 26.3 **server jar**, once | The first preparation needs it. It is `downloads.server.url` in your launcher's `versions/26.3/26.3.json` |
| A GPU with Vulkan or OpenGL 3.3 / DX12 | Horizon's renderer |

Sunrise supports Windows x64, Linux x86-64, macOS arm64 and macOS x64 — the platforms
Horizon publishes builds for.

## Run it

```sh
cargo run --release --bin sunrise        # the window
cargo run --release --bin sunrise-cli -- status
```

The **Setup** tab is where a new install starts: install a build, point at your
Minecraft and the server jar, press **Prepare game files** once. Then type a name and
press **Play**.

## Offline accounts, and what they can join

Minecraft's offline mode is what a name and no session give you:

- **Singleplayer and LAN worlds**: work.
- **Offline-mode servers** (`online-mode=false`) and most LAN-hosted games: the server
  looks the player up by name, so they join.
- **Online-mode servers**: still refuse, because they check the session with Mojang. That
  is the server's authentication, not a Horizon restriction — Sunrise cannot and does not
  forge sessions.

Preparing the game still needs your own Minecraft 26.3 install. Sunrise never downloads
Minecraft itself.

## Command line

The same engine without a display — handy for scripts and for checking a setup:

```sh
sunrise-cli status                     # what is ready, what is missing
sunrise-cli builds [--refresh]         # installable builds for this platform
sunrise-cli install [TAG|latest]       # download, verify, unpack
sunrise-cli prepare [--sounds]         # prepare the selected build
sunrise-cli play --name Alex --server 192.168.1.20:25565
sunrise-cli instances                  # list game folders
```

Options: `--name NAME`, `--server HOST[:PORT]`, `--data-dir DIR`, `--minecraft-dir DIR`,
`--server-jar PATH`, `--java-home PATH`, `--instance ID`, `--fullscreen`, `--world NAME`.

## Where things live

`SUNRISE_DATA_DIR`, else the platform data folder (`%APPDATA%\Sunrise` on Windows,
`~/Library/Application Support/Sunrise` on macOS, `~/.local/share/Sunrise` on Linux):

```
sunrise.json              settings (offline name, paths) and game folders
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
  `<build>/.local/horizon-client/<version>/`.
- Exit codes are reported as the API defines them: 0 done, 1 failed or crash, 2 a refused
  command line, 101 a panic (the crash report is named on the last stderr line).

## Known limits

- No Microsoft sign-in, by design; online-mode servers will refuse offline players.
- No build update checker: **Builds** shows what is published; nothing updates behind you.
- No mod/resource-pack browser and no server hosting.
- No "stop the game" button; the game is a normal child process the launcher follows.
- Installing the moving `nightly` tag replaces that folder, so its resources have to be
  prepared again.
- One game folder in the window; the CLI can pick another instance.

## Layout

```
src/lib.rs        events, process spawning with captured output, argument splitting
src/builds.rs     GitHub releases, download, SHA-256 check, unpack
src/detect.rs     Minecraft 26.3 install and Java 25 discovery
src/prepare.rs    horizon-prepare invocations and readiness
src/launch.rs     Launch API probing, --name/--quickPlayMultiplayer, spawning
src/config.rs     settings, offline name, game folders
src/paths.rs      data folder layout
src/bin/sunrise.rs       the window (egui/eframe)
src/bin/sunrise_cli.rs   the headless driver
```
