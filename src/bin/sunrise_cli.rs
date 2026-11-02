//! Headless driver for the same engine the window uses: install, prepare, play.
//! Handy for scripts, and the way Sunrise is tested without a display.

use std::path::PathBuf;
use std::sync::mpsc;

use sunrise::config::AccountMode;
use sunrise::{
    auth, builds, detect, launch, microsoft, mojang, paths, prepare, spawn_streamed, Config, Event,
    Job, Settings,
};

const HELP: &str = "\
sunrise-cli - install, prepare and play Horizon without the window

Usage: sunrise-cli [options] <command>

Commands:
  status                 Show what is ready and what is missing
  instances              List instances
  builds [--refresh]     List installable Horizon builds
  install [TAG|latest]   Install a build (default: the newest)
  prepare [--sounds]     Prepare the game files from your own Minecraft 26.3
  play                   Start the game
  account                Show the account Sunrise would use
  account signin         Sign in with Microsoft here in the terminal
  account refresh        Renew the saved sign-in without the browser
  account import FILE    Check a Minecraft session JSON with Mojang and keep it
  account online         Play with the Microsoft account
  account offline        Play as the offline name (the default)

Options:
  --name NAME            Offline player name (default: the saved one)
  --server HOST[:PORT]   Join a server at start
  --session FILE         Use this session JSON for this run only
  --offline              Ignore a saved session for this run
  --data-dir DIR         Where Sunrise keeps builds and instances
  --minecraft-dir DIR    Your official launcher's .minecraft folder
  --java-home PATH       Java 25 to prepare with
  --instance ID          Instance to play (default: the selected one)
  --fullscreen           Start fullscreen
  --world NAME           World to open at start
  -h, --help             This text

The 26.3 server jar the first preparation needs is downloaded from Mojang and
checked against Horizon's pin; there is nothing to choose.

Signing in asks for nothing but a Microsoft account: Sunrise prints a code,
opens microsoft.com/link in the browser, and waits for you to approve it there.
";

fn main() {
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") || args.is_empty() {
        println!("{HELP}");
        return 0;
    }

    let mut data = paths::data_dir();
    let mut overrides = Settings::default();
    let mut command: Option<String> = None;
    let mut positionals: Vec<String> = Vec::new();
    let mut flags: Vec<String> = Vec::new();
    let mut instance_arg: Option<String> = None;
    let mut world_arg: Option<String> = None;
    let mut name_arg: Option<String> = None;
    let mut server_arg: Option<String> = None;
    let mut session_arg: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let take = |i: &mut usize| -> Option<String> {
            *i += 1;
            args.get(*i).cloned()
        };
        match arg {
            "--data-dir" => match take(&mut i) {
                Some(v) => data = PathBuf::from(v),
                None => return usage("--data-dir needs a folder"),
            },
            "--minecraft-dir" => match take(&mut i) {
                Some(v) => overrides.minecraft_dir = Some(PathBuf::from(v)),
                None => return usage("--minecraft-dir needs a folder"),
            },
            "--session" => match take(&mut i) {
                Some(v) => session_arg = Some(PathBuf::from(v)),
                None => return usage("--session needs a session JSON file"),
            },
            "--java-home" => match take(&mut i) {
                Some(v) => overrides.java_home = Some(PathBuf::from(v)),
                None => return usage("--java-home needs a folder"),
            },
            "--instance" => match take(&mut i) {
                Some(v) => instance_arg = Some(v),
                None => return usage("--instance needs an id"),
            },
            "--world" => match take(&mut i) {
                Some(v) => world_arg = Some(v),
                None => return usage("--world needs a name"),
            },
            "--name" => match take(&mut i) {
                Some(v) => name_arg = Some(v),
                None => return usage("--name needs a player name"),
            },
            "--server" => match take(&mut i) {
                Some(v) => server_arg = Some(v),
                None => return usage("--server needs host[:port]"),
            },
            _ if arg.starts_with('-') => flags.push(arg.to_string()),
            _ if command.is_none() => command = Some(arg.to_string()),
            _ => positionals.push(arg.to_string()),
        }
        i += 1;
    }

    let Some(command) = command else {
        println!("{HELP}");
        return 0;
    };

    let mut config = Config::load(&data);
    // Command-line overrides win for this run, without touching the saved file.
    if overrides.minecraft_dir.is_some() {
        config.settings.minecraft_dir = overrides.minecraft_dir;
    }
    if overrides.java_home.is_some() {
        config.settings.java_home = overrides.java_home;
    }
    if let Some(dir) = &config.settings.minecraft_dir {
        if !dir.is_dir() {
            eprintln!("sunrise: warning: {} is not a folder", dir.display());
        }
    }

    // A session for this run: read, but only used by play.
    let mut run_session = config.settings.session.clone();
    if let Some(path) = &session_arg {
        match std::fs::read_to_string(path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))
            .and_then(|text| auth::parse_session(&text))
        {
            Ok(session) => run_session = Some(session),
            Err(err) => {
                eprintln!("sunrise: {err}");
                return 1;
            }
        }
    }
    if flags.iter().any(|f| f == "--offline") {
        run_session = None;
    }

    println!("data: {}", data.display());
    if let Some(name) = name_arg
        .as_deref()
        .or(Some(config.settings.player_name.as_str()))
    {
        if !sunrise::config::valid_player_name(name) {
            eprintln!(
                "sunrise: warning: '{name}' is not a Minecraft name (3-16 letters, digits or _)"
            );
        }
    }

    match command.as_str() {
        "status" => cmd_status(&data, &config),
        "instances" => cmd_instances(&config),
        "builds" => cmd_builds(&data, &config, flags.iter().any(|f| f == "--refresh")),
        "install" => cmd_install(&data, positionals.first().map(String::as_str)),
        "prepare" => cmd_prepare(&data, &config, flags.iter().any(|f| f == "--sounds")),
        "play" => cmd_play(
            &data,
            &mut config,
            instance_arg.as_deref(),
            world_arg.as_deref(),
            name_arg.as_deref(),
            server_arg.as_deref(),
            run_session.as_ref(),
            &flags,
        ),
        "account" => cmd_account(&data, &mut config, &positionals),
        other => usage(&format!("unknown command {other}")),
    }
}

fn usage(message: &str) -> i32 {
    eprintln!("sunrise: {message}");
    eprintln!();
    eprintln!("{HELP}");
    2
}

fn cmd_status(data: &std::path::Path, config: &Config) -> i32 {
    let detection = detect::detect(&config.settings);
    let installed = builds::installed_builds(data);
    let build = select_build(&installed, config);
    let setup = prepare::status(build.as_ref().map(|b| b.dir.as_path()), Some(&detection));

    println!("\nAccount");
    match (&config.settings.account, &config.settings.session) {
        (AccountMode::Microsoft, Some(session)) => println!(
            "  {}  Microsoft session, checked {}",
            session.name,
            ago(session.checked_at)
        ),
        (AccountMode::Microsoft, None) => println!(
            "  {}  offline name (the Microsoft account is selected but nobody is signed in)",
            config.settings.player_name
        ),
        _ => println!("  {}  offline name", config.settings.player_name),
    }

    println!("\nBuilds");
    if installed.is_empty() {
        println!("  none installed (run: sunrise-cli install)");
    } else {
        for item in &installed {
            let mark = if Some(&item.tag) == build.as_ref().map(|b| &b.tag) {
                "*"
            } else {
                " "
            };
            println!(
                " {mark} {}  Minecraft {}  {}",
                item.tag,
                item.version,
                item.dir.display()
            );
        }
    }

    println!("\nYour Minecraft");
    match &detection.chosen {
        Some(install) if install.has_game_version => {
            println!("  ok 26.3 found at {}", install.dir.display());
        }
        _ => println!(
            "  missing: no versions/{}/{} {{json,jar}} under the folders Sunrise looked in",
            sunrise::detect::GAME_VERSION,
            sunrise::detect::GAME_VERSION
        ),
    }
    for install in detection.installs.iter().filter(|i| !i.has_game_version) {
        println!("        (other install: {})", install.dir.display());
    }

    println!("\nJava");
    match &detection.java {
        Some(java) => println!(
            "  {} Java {} at {}",
            if java.is_usable() { "ok" } else { "old" },
            java.version,
            java.home.display()
        ),
        None => println!("  missing: no Java runtime found"),
    }

    println!("\nServer jar (fetched from Mojang on the first prepare)");
    let version = build
        .as_ref()
        .map(|b| b.version.clone())
        .unwrap_or_else(|| sunrise::detect::GAME_VERSION.to_string());
    let pinned = build
        .as_ref()
        .and_then(|b| builds::pinned_server_sha1(&b.dir))
        .unwrap_or_default();
    match mojang::cached(data, &version, &pinned) {
        Some(path) => println!("  ok {}", path.display()),
        None => println!(
            "  not downloaded yet (Horizon pins sha1 {})",
            if pinned.is_empty() {
                "unknown"
            } else {
                &pinned
            }
        ),
    }

    if let Some(build) = &build {
        println!("\nGame files for {}", build.tag);
        let resources = prepare::resources_dir(&build.dir);
        println!(
            "  {} {}",
            if prepare::is_prepared(&build.dir) {
                "ok"
            } else {
                "needs preparing:"
            },
            resources.display()
        );
        if let Ok(caps) = launch::probe_caps(&build.dir) {
            println!(
                "  launch api {}  game {}  ({})",
                caps.launch_api, caps.minecraft, caps.horizon_label
            );
        }
    }

    let (done, total) = setup.score();
    println!("\n{}/{} ready", done, total);
    if let Some(reason) =
        prepare::blocker(build.as_ref().map(|b| b.dir.as_path()), Some(&detection))
    {
        println!("next: {reason}");
    }
    0
}

fn ago(secs: u64) -> String {
    if secs == 0 {
        return "never".to_string();
    }
    let minutes = sunrise::now_secs().saturating_sub(secs) / 60;
    match minutes {
        0 => "just now".to_string(),
        m if m < 60 => format!("{m} min ago"),
        m if m < 2880 => format!("{} h ago", m / 60),
        m => format!("{} days ago", m / 1440),
    }
}

fn cmd_instances(config: &Config) -> i32 {
    for instance in &config.instances {
        let selected = if config.selected_instance.as_deref() == Some(instance.id.as_str()) {
            "*"
        } else {
            " "
        };
        println!(
            " {selected} {:<18} {:<24} {}",
            instance.id,
            instance.name,
            instance.game_dir.display()
        );
    }
    0
}

fn select_build(
    installed: &[builds::InstalledBuild],
    config: &Config,
) -> Option<builds::InstalledBuild> {
    let wanted = config
        .settings
        .last_build
        .clone()
        .or_else(|| config.selected().build_tag.clone());
    match wanted {
        Some(tag) => installed
            .iter()
            .find(|b| b.tag == tag)
            .cloned()
            .or_else(|| installed.last().cloned()),
        None => installed.last().cloned(),
    }
}

fn cmd_builds(data: &std::path::Path, config: &Config, refresh: bool) -> i32 {
    match builds::fetch_builds(data, refresh) {
        Ok(list) => {
            let installed = builds::installed_builds(data);
            println!("\n{} builds for this platform:\n", list.len());
            for (index, build) in list.iter().enumerate() {
                let have = installed.iter().any(|b| b.tag == build.tag);
                println!(
                    " {} {:<26} {:>7.1} MB  {}  {}",
                    if index == 0 { ">" } else { " " },
                    build.tag,
                    build.size as f64 / 1_048_576.0,
                    if have { "installed" } else { "         " },
                    build.published_at
                );
            }
            println!("\ninstall with: sunrise-cli install <tag>");
            if config.settings.last_build.is_some() {
                println!("selected: {}", config.settings.last_build.clone().unwrap());
            }
            0
        }
        Err(err) => {
            eprintln!("sunrise: {err}");
            1
        }
    }
}

fn cmd_install(data: &std::path::Path, wanted: Option<&str>) -> i32 {
    let list = match builds::fetch_builds(data, false) {
        Ok(list) => list,
        Err(err) => {
            eprintln!("sunrise: {err}");
            return 1;
        }
    };
    let wanted = wanted.unwrap_or("latest");
    let build = if wanted == "latest" {
        list.first().cloned()
    } else {
        list.iter().find(|b| b.tag == wanted).cloned()
    };
    let Some(build) = build else {
        eprintln!("sunrise: no build called {wanted}; sunrise-cli builds lists them");
        return 1;
    };

    println!("installing {} ...", build.tag);
    let mut last_percent = 0u64;
    let mut progress = |p: builds::Progress| {
        if let Some(percent) = p.done.saturating_mul(100).checked_div(p.total) {
            let percent = percent.min(100);
            if percent >= last_percent.saturating_add(5) || percent == 100 {
                last_percent = percent;
                print!(
                    "\r  {} {percent}% ({} / {} MB)   ",
                    p.message,
                    p.done / 1_048_576,
                    p.total / 1_048_576
                );
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        }
    };
    match builds::install(data, &build, &mut progress) {
        Ok(installed) => {
            println!(
                "\ninstalled {} at {}",
                installed.tag,
                installed.dir.display()
            );
            0
        }
        Err(err) => {
            println!();
            eprintln!("sunrise: {err}");
            1
        }
    }
}

fn run_job(cmd: std::process::Command, job: Job, label: &str) -> Option<i32> {
    let (tx, rx) = mpsc::channel();
    println!("\n{label}");
    match spawn_streamed(cmd, job, tx, None) {
        Ok(_) => {}
        Err(err) => {
            eprintln!("sunrise: {err}");
            return None;
        }
    }
    for event in rx {
        match event {
            Event::Log { line, .. } => println!("  {line}"),
            Event::Finished { code, seconds, .. } => {
                println!("\nfinished in {seconds:.1}s with exit code {code:?}");
                return Some(code.unwrap_or(-1));
            }
            _ => {}
        }
    }
    None
}

fn cmd_prepare(data: &std::path::Path, config: &Config, force_sounds: bool) -> i32 {
    let installed = builds::installed_builds(data);
    let Some(build) = select_build(&installed, config) else {
        eprintln!("sunrise: no build installed; run: sunrise-cli install");
        return 1;
    };
    let detection = detect::detect(&config.settings);
    if let Some(reason) = prepare::blocker(Some(&build.dir), Some(&detection)) {
        eprintln!("sunrise: {reason}");
        return 1;
    }
    let minecraft = detection.chosen.as_ref().map(|c| c.dir.as_path());

    // The one file Sunrise has to bring: downloaded once, checked against the
    // SHA-1 Horizon pins, then handed to the preparer like any other argument.
    let pinned = builds::pinned_server_sha1(&build.dir).unwrap_or_default();
    let mut last_percent = 0u64;
    let mut progress = |p: mojang::Progress| {
        if let Some(percent) = p.done.saturating_mul(100).checked_div(p.total) {
            let percent = percent.min(100);
            if percent >= last_percent.saturating_add(5) || percent == 100 {
                last_percent = percent;
                print!(
                    "\r  {} {percent}% ({} / {} MB)   ",
                    p.message,
                    p.done / 1_048_576,
                    p.total / 1_048_576
                );
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        } else {
            println!("  {}", p.message);
        }
    };
    let server_jar = match mojang::ensure(data, &build.version, &pinned, &mut progress) {
        Ok(path) => path,
        Err(err) => {
            println!();
            eprintln!("sunrise: {err}");
            return 1;
        }
    };
    println!();

    let cmd = match prepare::prepare_command(
        &build.dir,
        minecraft,
        Some(&server_jar),
        config.settings.java_home.as_deref(),
    ) {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("sunrise: {err}");
            return 1;
        }
    };
    let code = run_job(cmd, Job::Prepare, "Preparing game files");
    if code != Some(0) {
        return 1;
    }
    if config.settings.copy_sounds || force_sounds {
        if let Ok(cmd) =
            prepare::sounds_command(&build.dir, minecraft, config.settings.java_home.as_deref())
        {
            let _ = run_job(cmd, Job::Sounds, "Copying sounds");
        }
    }
    if prepare::is_prepared(&build.dir) {
        println!("\nready: {}", prepare::resources_dir(&build.dir).display());
        0
    } else {
        eprintln!("\nsunrise: preparation finished but the resources are not complete");
        1
    }
}

fn cmd_play(
    data: &std::path::Path,
    config: &mut Config,
    instance_id: Option<&str>,
    world: Option<&str>,
    name: Option<&str>,
    server: Option<&str>,
    session: Option<&auth::Session>,
    flags: &[String],
) -> i32 {
    // A renewable sign-in renews itself first, so a day-old token never reaches
    // the server as an expired one.
    let renewed = match session {
        Some(session) if session.needs_renewal() => {
            println!("renewing the Microsoft sign-in ...");
            match microsoft::refresh(session) {
                Ok(checked) => {
                    let fresh = checked.session;
                    config.settings.session = Some(fresh.clone());
                    if config.settings.account == AccountMode::Microsoft {
                        let _ = config.save(data);
                    }
                    println!("sign-in renewed for {}", fresh.name);
                    Some(fresh)
                }
                Err(err) => {
                    eprintln!("sunrise: warning: {err}");
                    None
                }
            }
        }
        _ => None,
    };
    let session = renewed.as_ref().or(session);

    let installed = builds::installed_builds(data);
    let Some(build) = select_build(&installed, config) else {
        eprintln!("sunrise: no build installed; run: sunrise-cli install");
        return 1;
    };
    if !prepare::is_prepared(&build.dir) {
        eprintln!(
            "sunrise: {} is not prepared yet; run: sunrise-cli prepare",
            build.tag
        );
        return 1;
    }

    let instance = match instance_id {
        Some(id) => config
            .instances
            .iter()
            .find(|i| i.id == id)
            .cloned()
            .or_else(|| {
                config
                    .instances
                    .iter()
                    .find(|i| paths::slug(&i.name) == paths::slug(id))
                    .cloned()
            }),
        None => Some(config.selected().clone()),
    };
    let Some(instance) = instance else {
        eprintln!("sunrise: no such instance");
        return 1;
    };

    let caps = match launch::probe_caps(&build.dir) {
        Ok(caps) => caps,
        Err(err) => {
            eprintln!("sunrise: {err}");
            return 1;
        }
    };

    let spec = launch::GameSpec {
        build_dir: build.dir.clone(),
        game_dir: instance.game_dir.clone(),
        home: instance.game_dir.clone(),
        width: instance.width,
        height: instance.height,
        fullscreen: flags.iter().any(|f| f == "--fullscreen") || instance.fullscreen,
        render_distance: instance.render_distance,
        world: world.map(str::to_string).or_else(|| instance.world.clone()),
        seed: instance.seed.clone(),
        creative: instance.creative,
        extra_args: instance.extra_args.clone(),
        name: if session.is_some() {
            None
        } else {
            name.map(str::to_string)
                .or_else(|| Some(config.settings.player_name.clone()))
        },
        session: session.cloned(),
        server: server
            .map(str::to_string)
            .or_else(|| instance.connect.clone()),
    };

    let (tx, rx) = mpsc::channel();
    let pid = match launch::spawn(&spec, &caps, tx) {
        Ok(pid) => pid,
        Err(err) => {
            eprintln!("sunrise: {err}");
            return 1;
        }
    };
    match session {
        Some(session) => println!(
            "started {} as {} (Microsoft session)",
            caps.horizon_label, session.name
        ),
        None => println!(
            "started {} as {} (offline name)",
            caps.horizon_label, config.settings.player_name
        ),
    }
    println!("game folder: {} (pid {pid})", spec.game_dir.display());
    if let Some(server) = &spec.server {
        println!("joining {server}");
    }

    let started = std::time::Instant::now();
    let mut code = None;
    for event in rx {
        match event {
            // Horizon writes its normal log to stderr, so both streams read alike.
            Event::Log {
                line, err: true, ..
            } => eprintln!("  {line}"),
            Event::Log { line, .. } => println!("  {line}"),
            Event::Finished { code: exit, .. } => {
                code = exit;
                break;
            }
            _ => {}
        }
    }

    if let Some(instance) = config.instances.iter_mut().find(|i| i.id == instance.id) {
        instance.last_played = Some(sunrise::now_secs());
        instance.play_seconds += started.elapsed().as_secs();
        let _ = config.save(data);
    }

    println!("\n{}", launch::exit_hint(code));
    0
}

/// The optional Microsoft account. Sunrise takes a session a launcher already
/// gave the player and checks it with Mojang; it does not sign anyone in, because
/// Minecraft only accepts sessions from an application Mojang approved.
fn cmd_account(data: &std::path::Path, config: &mut Config, args: &[String]) -> i32 {
    let action = args.first().map(String::as_str);
    match action {
        None => {
            println!("\nMode: {:?}", config.settings.account);
            match &config.settings.session {
                Some(session) => {
                    println!("  name   {}", session.name);
                    println!("  uuid   {}", session.short_uuid());
                    println!(
                        "  token  held in sunrise.json, checked {}",
                        ago(session.checked_at)
                    );
                    if session.looks_stale() {
                        println!("  note   Minecraft tokens last 24 h; import a fresh session if it stopped working");
                    }
                    println!(
                        "  renew  {}",
                        if session.renewable() {
                            "yes, from its refresh token"
                        } else {
                            "no; this session was imported"
                        }
                    );
                }
                None => println!("  not signed in (offline name only)"),
            }
            println!("\nsign in: sunrise-cli account signin");
            println!("import: sunrise-cli account import session.json");
            0
        }
        Some("offline") => {
            config.settings.account = AccountMode::Offline;
            match config.save(data) {
                Ok(()) => {
                    println!(
                        "now playing as the offline name '{}'",
                        config.settings.player_name
                    );
                    0
                }
                Err(err) => {
                    eprintln!("sunrise: {err}");
                    1
                }
            }
        }
        Some("online") => {
            if config.settings.session.is_none() {
                eprintln!("sunrise: not signed in yet; run: sunrise-cli account signin");
                return 1;
            }
            config.settings.account = AccountMode::Microsoft;
            match config.save(data) {
                Ok(()) => {
                    println!("now playing with the Microsoft account");
                    0
                }
                Err(err) => {
                    eprintln!("sunrise: {err}");
                    1
                }
            }
        }
        Some("signin") => {
            let code = match microsoft::begin() {
                Ok(code) => code,
                Err(err) => {
                    eprintln!("sunrise: {err}");
                    return 1;
                }
            };
            println!("\n  open  {}", code.verification_uri);
            println!("  code  {}\n", code.user_code);
            println!("waiting for the browser (Ctrl+C cancels) ...");

            let tokens = match microsoft::wait(&code, || true) {
                Ok(Some(tokens)) => tokens,
                Ok(None) => {
                    eprintln!("sunrise: cancelled");
                    return 1;
                }
                Err(err) => {
                    eprintln!("sunrise: {err}");
                    return 1;
                }
            };
            let checked =
                match microsoft::session_from_microsoft(&tokens.access_token, tokens.refresh_token)
                {
                    Ok(checked) => checked,
                    Err(err) => {
                        eprintln!("sunrise: {err}");
                        return 1;
                    }
                };
            if let Some(warning) = &checked.warning {
                eprintln!("sunrise: warning: {warning}");
            }
            let name = checked.session.name.clone();
            config.settings.session = Some(checked.session);
            config.settings.account = AccountMode::Microsoft;
            match config.save(data) {
                Ok(()) => {
                    println!("signed in as {name}; online-mode servers will accept this account");
                    0
                }
                Err(err) => {
                    eprintln!("sunrise: {err}");
                    1
                }
            }
        }
        Some("refresh") => {
            let Some(session) = config.settings.session.clone() else {
                eprintln!("sunrise: no session to renew; run: sunrise-cli account signin");
                return 1;
            };
            println!("renewing the Microsoft sign-in ...");
            match microsoft::refresh(&session) {
                Ok(checked) => {
                    if let Some(warning) = &checked.warning {
                        eprintln!("sunrise: warning: {warning}");
                    }
                    let name = checked.session.name.clone();
                    config.settings.session = Some(checked.session);
                    config.settings.account = AccountMode::Microsoft;
                    match config.save(data) {
                        Ok(()) => {
                            println!("renewed; still signed in as {name}");
                            0
                        }
                        Err(err) => {
                            eprintln!("sunrise: {err}");
                            1
                        }
                    }
                }
                Err(err) => {
                    eprintln!("sunrise: {err}");
                    1
                }
            }
        }
        Some("import") => {
            let Some(path) = args.get(1) else {
                eprintln!("sunrise: account import needs a session JSON file");
                return 1;
            };
            let text = match std::fs::read_to_string(path) {
                Ok(text) => text,
                Err(err) => {
                    eprintln!("sunrise: could not read {path}: {err}");
                    return 1;
                }
            };
            let session = match auth::parse_session(&text) {
                Ok(session) => session,
                Err(err) => {
                    eprintln!("sunrise: {err}");
                    return 1;
                }
            };
            println!("checking the session with Mojang ...");
            match auth::check(&session) {
                Ok(checked) => {
                    if let Some(warning) = &checked.warning {
                        eprintln!("sunrise: warning: {warning}");
                    }
                    let name = checked.session.name.clone();
                    config.settings.session = Some(checked.session);
                    config.settings.account = AccountMode::Microsoft;
                    match config.save(data) {
                        Ok(()) => {
                            println!(
                                "signed in as {name}; online-mode servers will accept this account"
                            );
                            0
                        }
                        Err(err) => {
                            eprintln!("sunrise: {err}");
                            1
                        }
                    }
                }
                Err(err) => {
                    eprintln!("sunrise: {err}");
                    1
                }
            }
        }
        Some(other) => usage(&format!("account {other} is not a command")),
    }
}
