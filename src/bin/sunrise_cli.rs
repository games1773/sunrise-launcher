//! Headless driver for the same engine the window uses: install, prepare, play.
//! Handy for scripts, and the way Sunrise is tested without a display.

use std::path::PathBuf;
use std::sync::mpsc;

use sunrise::{
    builds, detect, launch, paths, prepare, spawn_streamed, Config, Event, Job, Settings,
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
  play                   Start the game as your offline account

Options:
  --name NAME            Offline player name (default: the saved one)
  --server HOST[:PORT]   Join an offline-mode server at start
  --data-dir DIR         Where Sunrise keeps builds and instances
  --minecraft-dir DIR    Your official launcher's .minecraft folder
  --server-jar PATH      The 26.3 server jar the first preparation needs
  --java-home PATH       Java 25 to prepare with
  --instance ID          Instance to play (default: the selected one)
  --fullscreen           Start fullscreen
  --world NAME           World to open at start
  -h, --help             This text
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
            "--server-jar" => match take(&mut i) {
                Some(v) => overrides.server_jar = Some(PathBuf::from(v)),
                None => return usage("--server-jar needs a file"),
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
    if overrides.server_jar.is_some() {
        config.settings.server_jar = overrides.server_jar;
    }
    if overrides.java_home.is_some() {
        config.settings.java_home = overrides.java_home;
    }
    if let Some(dir) = &config.settings.minecraft_dir {
        if !dir.is_dir() {
            eprintln!("sunrise: warning: {} is not a folder", dir.display());
        }
    }

    println!("data: {}", data.display());
    if let Some(name) = name_arg.as_deref().or(Some(config.settings.player_name.as_str())) {
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
            &flags,
        ),
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
    let setup = prepare::status(
        build.as_ref().map(|b| b.dir.as_path()),
        Some(&detection),
        config.settings.server_jar.as_deref(),
    );

    println!("\nOffline account");
    println!("  {}", config.settings.player_name);

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
            println!("  {} 26.3 found at {}", "ok", install.dir.display());
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

    println!("\nServer jar (needed once, to prepare)");
    match &detection.server_jar {
        Some(jar) => println!("  ok {}", jar.display()),
        None => println!("  not set (--server-jar PATH, or Settings in the window)"),
    }

    if let Some(build) = &build {
        println!("\nGame files for {}", build.tag);
        let resources = prepare::resources_dir(&build.dir);
        println!(
            "  {} {}",
            if prepare::is_prepared(&build.dir) { "ok" } else { "needs preparing:" },
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
    if let Some(reason) = prepare::blocker(
        build.as_ref().map(|b| b.dir.as_path()),
        Some(&detection),
        config.settings.server_jar.as_deref(),
    ) {
        println!("next: {reason}");
    }
    0
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

fn select_build(installed: &[builds::InstalledBuild], config: &Config) -> Option<builds::InstalledBuild> {
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
        if p.total > 0 {
            let percent = (p.done * 100 / p.total).min(100);
            if percent >= last_percent.saturating_add(5) || percent == 100 {
                last_percent = percent;
                print!("\r  {} {percent}% ({} / {} MB)   ", p.message, p.done / 1_048_576, p.total / 1_048_576);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        }
    };
    match builds::install(data, &build, &mut progress) {
        Ok(installed) => {
            println!("\ninstalled {} at {}", installed.tag, installed.dir.display());
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
    match spawn_streamed(cmd, job, tx) {
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
    if let Some(reason) = prepare::blocker(
        Some(&build.dir),
        Some(&detection),
        config.settings.server_jar.as_deref(),
    ) {
        eprintln!("sunrise: {reason}");
        return 1;
    }
    let minecraft = detection.chosen.as_ref().map(|c| c.dir.as_path());
    let cmd = match prepare::prepare_command(
        &build.dir,
        minecraft,
        config.settings.server_jar.as_deref(),
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
        if let Ok(cmd) = prepare::sounds_command(
            &build.dir,
            minecraft,
            config.settings.java_home.as_deref(),
        ) {
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
    flags: &[String],
) -> i32 {
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
        name: name
            .map(str::to_string)
            .or_else(|| Some(config.settings.player_name.clone())),
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
    println!("started {} as {}", caps.horizon_label, config.settings.player_name);
    println!("game folder: {} (pid {pid})", spec.game_dir.display());
    if let Some(server) = &spec.server {
        println!("joining {server}");
    }

    let started = std::time::Instant::now();
    let mut code = None;
    for event in rx {
        match event {
            // Horizon writes its normal log to stderr, so both streams read alike.
            Event::Log { line, err: true, .. } => eprintln!("  {line}"),
            Event::Log { line, .. } => println!("  {line}"),
            Event::Finished { code: exit, .. } => {
                code = exit;
                break;
            }
            _ => {}
        }
    }

    if let Some(instance) = config
        .instances
        .iter_mut()
        .find(|i| i.id == instance.id)
    {
        instance.last_played = Some(sunrise::now_secs());
        instance.play_seconds += started.elapsed().as_secs();
        let _ = config.save(data);
    }

    println!("\n{}", launch::exit_hint(code));
    0
}
