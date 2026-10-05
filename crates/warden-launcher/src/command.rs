//! A linha de comando final, montada pelo Warden a partir do que o motor instalou
//! (ARCHITECTURE §7.1, §7.2, §7.4 e §7.8). Função pura: os golden tests comparam a saída
//! para cada combinação da matriz L-05.
//!
//! Ordem: `-Xmx`, `-XX:ErrorFile`, codificação da saída, proteção do log4j (≤ 1.18),
//! propriedades pedidas por quem abre, argumentos JVM do perfil, argumentos do usuário (o
//! último vale), classe principal, argumentos do jogo com o jogador offline, entrada direta e
//! `--offlineDeveloperMode` (1.21.9+). Quando a linha passa de ~30 000 caracteres e o Java
//! aceita (9+), tudo vai num `@argfile`.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::offline::WARDEN_CLIENT_ID;
use crate::range::{has_offline_developer_mode, has_quick_play, needs_safe_log4j_config};
use crate::spec::{ArgFile, InstalledGame, LaunchCommand, LaunchOptions, QuickPlay};

/// A partir deste tamanho a linha vai para um `@argfile` (o limite do Windows é 32 767).
pub const ARGFILE_THRESHOLD: usize = 30_000;

/// Limite do `CreateProcess` do Windows.
const WINDOWS_COMMAND_LIMIT: usize = 32_767;

/// Nome do arquivo de argumentos dentro do `state_dir`.
pub const ARGFILE_NAME: &str = "launch-args.txt";

/// Sistema para o qual a linha é montada (os golden tests fixam o Windows em qualquer
/// máquina).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    /// Windows: `\` nos caminhos.
    Windows,
    /// Linux: `/` nos caminhos.
    Linux,
}

impl TargetOs {
    /// O sistema desta máquina.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    const fn separator(self) -> char {
        match self {
            Self::Windows => '\\',
            Self::Linux => '/',
        }
    }
}

/// Junta um nome ao caminho com o separador do sistema alvo.
fn join(dir: &Path, name: &str, os: TargetOs) -> String {
    let base = dir.display().to_string();
    let trimmed = base.trim_end_matches(['/', '\\']);
    format!("{trimmed}{}{name}", os.separator())
}

/// Argumentos de codificação da saída por major do Java (ARCHITECTURE §7.4). No Java 8 fica
/// desligado até ser validado com packs reais (R2 §9 item 7): a saída sai na página de código
/// do Windows e a leitura recua para Windows-1252.
fn encoding_args(java_major: u32) -> &'static [&'static str] {
    match java_major {
        19.. => &["-Dstdout.encoding=UTF-8", "-Dstderr.encoding=UTF-8"],
        17 | 18 => &["-Dfile.encoding=UTF-8"],
        _ => &[],
    }
}

/// Troca o valor depois de cada opção de autenticação do jogo.
fn apply_player(args: &mut [String], opts: &LaunchOptions) {
    let mut index = 0;
    while index + 1 < args.len() {
        let value = match args[index].as_str() {
            "--username" => Some(opts.player.name.as_str().to_owned()),
            "--uuid" => Some(opts.player.uuid.simple()),
            "--accessToken" | "--xuid" => Some("0".to_owned()),
            "--userType" => Some("legacy".to_owned()),
            "--clientId" => Some(WARDEN_CLIENT_ID.to_owned()),
            "--userProperties" => Some("{}".to_owned()),
            _ => None,
        };
        if let Some(value) = value {
            args[index + 1] = value;
            index += 2;
        } else {
            index += 1;
        }
    }
}

/// Argumentos de entrada direta.
fn quick_play_args(game: &InstalledGame, opts: &LaunchOptions) -> Result<Vec<String>> {
    let modern = has_quick_play(&game.release_time);
    let mut args = Vec::new();
    match &opts.quick_play {
        None => {}
        Some(QuickPlay::Singleplayer { world }) => {
            if !modern {
                return Err(Error::QuickPlayUnsupported {
                    minecraft: game.spec.minecraft.as_str().to_owned(),
                });
            }
            args.extend(["--quickPlaySingleplayer".to_owned(), world.clone()]);
        }
        Some(QuickPlay::Multiplayer { host, port }) => {
            if modern {
                args.extend([
                    "--quickPlayMultiplayer".to_owned(),
                    format!("{host}:{port}"),
                ]);
            } else {
                args.extend([
                    "--server".to_owned(),
                    host.clone(),
                    "--port".to_owned(),
                    port.to_string(),
                ]);
            }
        }
    }
    if modern && let Some(path) = &opts.quick_play_path {
        args.extend(["--quickPlayPath".to_owned(), path.clone()]);
    }
    Ok(args)
}

/// Um argumento no formato do `@argfile` do Java: sempre entre aspas, com `\` e `"`
/// escapados (dentro de aspas a barra invertida é escape).
fn argfile_quote(arg: &str) -> String {
    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    for character in arg.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

/// Tamanho aproximado da linha no Windows (cada argumento entre aspas e um espaço).
fn command_length(program: &Path, args: &[String]) -> usize {
    program.as_os_str().len() + 3 + args.iter().map(|arg| arg.len() + 3).sum::<usize>()
}

/// Monta a linha de comando.
///
/// Erros: [`Error::QuickPlayUnsupported`] (mundo direto antes da 1.20) e
/// [`Error::CommandTooLong`] (Java 8 no Windows com linha acima de 32 767 caracteres).
pub fn build(game: &InstalledGame, opts: &LaunchOptions, os: TargetOs) -> Result<LaunchCommand> {
    let game_dir = opts.game_dir.display().to_string();
    let relocate = |arg: &String| arg.replace(&game.game_dir_placeholder, &game_dir);

    let mut args = vec![
        format!("-Xmx{}M", opts.memory_mb),
        format!(
            "-XX:ErrorFile={}",
            join(&opts.game_dir, "hs_err_pid%p.log", os)
        ),
    ];
    args.extend(
        encoding_args(opts.java.major)
            .iter()
            .map(|arg| (*arg).to_owned()),
    );
    if needs_safe_log4j_config(&game.release_time) {
        // Proteção extra para log4j 2.10+ (1.17 e 1.18); nas versões anteriores quem protege
        // é a configuração escolhida na instalação (`InstalledGame::logging`).
        args.push("-Dlog4j2.formatMsgNoLookups=true".to_owned());
    }
    args.extend(
        opts.system_props
            .iter()
            .map(|(key, value)| format!("-D{key}={value}")),
    );
    args.extend(game.jvm_args.iter().map(relocate));
    args.extend(opts.extra_jvm_args.iter().cloned());
    args.push(game.main_class.clone());

    let mut game_args: Vec<String> = game.game_args.iter().map(relocate).collect();
    apply_player(&mut game_args, opts);
    game_args.extend(quick_play_args(game, opts)?);
    if has_offline_developer_mode(&game.release_time)
        && !game_args.iter().any(|arg| arg == "--offlineDeveloperMode")
    {
        game_args.push("--offlineDeveloperMode".to_owned());
    }
    args.extend(game_args);

    let program = opts.java.launcher.clone();
    let length = command_length(&program, &args);
    let mut argfile = None;
    if length > ARGFILE_THRESHOLD {
        if opts.java.major >= 9 {
            let path: PathBuf = join(&opts.state_dir, ARGFILE_NAME, os).into();
            let mut contents = String::with_capacity(length + args.len() * 2);
            for arg in &args {
                contents.push_str(&argfile_quote(arg));
                contents.push('\n');
            }
            args = vec![format!("@{}", path.display())];
            argfile = Some(ArgFile { path, contents });
        } else if os == TargetOs::Windows && length > WINDOWS_COMMAND_LIMIT {
            return Err(Error::CommandTooLong {
                length,
                java_major: opts.java.major,
            });
        }
    }

    Ok(LaunchCommand {
        program,
        args,
        cwd: opts.game_dir.clone(),
        env: Vec::new(),
        argfile,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::OfflineProfile;
    use crate::spec::{GameSpec, JavaRuntime, LoaderSpec, LoggingConfig};

    fn game(release_time: &str, game_args: &[&str]) -> InstalledGame {
        InstalledGame {
            spec: GameSpec::new(
                "1.20.1",
                LoaderSpec::Fabric {
                    version: "0.19.5".into(),
                },
            )
            .unwrap(),
            version_id: "fabric-loader-0.19.5-1.20.1".into(),
            hierarchy: vec!["fabric-loader-0.19.5-1.20.1".into(), "1.20.1".into()],
            release_time: release_time.into(),
            main_class: "net.fabricmc.loader.impl.launch.knot.KnotClient".into(),
            jvm_args: vec![
                "-Djava.library.path=C:\\shared\\natives\\abc".into(),
                "-cp".into(),
                "C:\\shared\\libraries\\a.jar;C:\\shared\\versions\\1.20.1\\1.20.1.jar".into(),
            ],
            game_args: game_args.iter().map(|arg| (*arg).to_owned()).collect(),
            game_dir_placeholder: "C:\\shared\\engine-gamedir".into(),
            logging: LoggingConfig::Mojang {
                file: "client-1.12.xml".into(),
            },
        }
    }

    fn options(major: u32) -> LaunchOptions {
        LaunchOptions {
            game_dir: PathBuf::from("C:\\dados\\instances\\X\\minecraft"),
            state_dir: PathBuf::from("C:\\dados\\instances\\X\\state"),
            java: JavaRuntime {
                java: PathBuf::from("C:\\java\\bin\\java.exe"),
                launcher: PathBuf::from("C:\\java\\bin\\javaw.exe"),
                major,
            },
            memory_mb: 4096,
            extra_jvm_args: vec!["-XX:+UseG1GC".into()],
            system_props: vec![("fabric.noGui".into(), "true".into())],
            quick_play: None,
            quick_play_path: None,
            player: OfflineProfile::from_name("WardenTest").unwrap(),
        }
    }

    const MODERN_ARGS: [&str; 16] = [
        "--username",
        "WardenPlayer",
        "--version",
        "fabric-loader-0.19.5-1.20.1",
        "--gameDir",
        "C:\\shared\\engine-gamedir",
        "--uuid",
        "ffffffffffffffffffffffffffffffff",
        "--accessToken",
        "",
        "--clientId",
        "",
        "--xuid",
        "",
        "--userType",
        "",
    ];

    #[test]
    fn jogador_offline_e_pasta_do_jogo() {
        let command = build(
            &game("2023-06-12T13:25:51+00:00", &MODERN_ARGS),
            &options(17),
            TargetOs::Windows,
        )
        .unwrap();
        let args = &command.args;
        let after = |flag: &str| {
            let index = args.iter().position(|arg| arg == flag).unwrap();
            args[index + 1].clone()
        };
        assert_eq!(after("--username"), "WardenTest");
        assert_eq!(after("--uuid"), "6c5aa2b1c08439d799a8edc5b372e5f9");
        assert_eq!(after("--accessToken"), "0");
        assert_eq!(after("--clientId"), WARDEN_CLIENT_ID);
        assert_eq!(after("--xuid"), "0");
        assert_eq!(after("--userType"), "legacy");
        assert_eq!(after("--gameDir"), "C:\\dados\\instances\\X\\minecraft");
        assert_eq!(args[0], "-Xmx4096M");
        assert_eq!(
            args[1],
            "-XX:ErrorFile=C:\\dados\\instances\\X\\minecraft\\hs_err_pid%p.log"
        );
        assert!(args.contains(&"-Dfile.encoding=UTF-8".to_owned()));
        assert!(args.contains(&"-Dfabric.noGui=true".to_owned()));
        assert!(!args.iter().any(|arg| arg == "--offlineDeveloperMode"));
        // O argumento do usuário vem depois dos do perfil e antes da classe principal.
        let user = args.iter().position(|arg| arg == "-XX:+UseG1GC").unwrap();
        let main = args
            .iter()
            .position(|arg| arg == "net.fabricmc.loader.impl.launch.knot.KnotClient")
            .unwrap();
        let classpath = args.iter().position(|arg| arg == "-cp").unwrap();
        assert!(classpath < user && user < main);
        assert_eq!(command.program, PathBuf::from("C:\\java\\bin\\javaw.exe"));
        assert_eq!(
            command.cwd,
            PathBuf::from("C:\\dados\\instances\\X\\minecraft")
        );
        assert!(command.argfile.is_none());
    }

    #[test]
    fn codificacao_por_java() {
        assert!(encoding_args(8).is_empty());
        assert_eq!(encoding_args(17), ["-Dfile.encoding=UTF-8"]);
        assert_eq!(
            encoding_args(21),
            ["-Dstdout.encoding=UTF-8", "-Dstderr.encoding=UTF-8"]
        );
        assert_eq!(encoding_args(25).len(), 2);
    }

    #[test]
    fn modo_offline_da_1_21_9() {
        let command = build(
            &game("2026-09-15T11:23:02+00:00", &MODERN_ARGS[..14]),
            &options(25),
            TargetOs::Windows,
        )
        .unwrap();
        assert_eq!(command.args.last().unwrap(), "--offlineDeveloperMode");
        assert!(!command.args.iter().any(|arg| arg == "--userType"));
    }

    #[test]
    fn log4j_protegido_ate_a_1_18() {
        let old = build(
            &game("2017-09-18T08:39:46+00:00", &MODERN_ARGS),
            &options(8),
            TargetOs::Windows,
        )
        .unwrap();
        assert!(
            old.args
                .contains(&"-Dlog4j2.formatMsgNoLookups=true".to_owned())
        );
        let new = build(
            &game("2023-06-12T13:25:51+00:00", &MODERN_ARGS),
            &options(17),
            TargetOs::Windows,
        )
        .unwrap();
        assert!(
            !new.args
                .iter()
                .any(|arg| arg.contains("formatMsgNoLookups"))
        );
    }

    #[test]
    fn entrada_direta_por_faixa() {
        let mut opts = options(17);
        opts.quick_play = Some(QuickPlay::Multiplayer {
            host: "127.0.0.1".into(),
            port: 25_570,
        });
        opts.quick_play_path = Some("quickplay/log.json".into());
        let modern = build(
            &game("2023-06-12T13:25:51+00:00", &MODERN_ARGS),
            &opts,
            TargetOs::Windows,
        )
        .unwrap();
        let tail: Vec<&str> = modern
            .args
            .iter()
            .rev()
            .take(4)
            .map(String::as_str)
            .collect();
        assert_eq!(
            tail,
            [
                "quickplay/log.json",
                "--quickPlayPath",
                "127.0.0.1:25570",
                "--quickPlayMultiplayer"
            ]
        );
        let legacy = build(
            &game("2017-09-18T08:39:46+00:00", &MODERN_ARGS),
            &opts,
            TargetOs::Windows,
        )
        .unwrap();
        let tail: Vec<&str> = legacy
            .args
            .iter()
            .rev()
            .take(4)
            .map(String::as_str)
            .collect();
        assert_eq!(tail, ["25570", "--port", "127.0.0.1", "--server"]);
        assert!(!legacy.args.iter().any(|arg| arg == "--quickPlayPath"));

        opts.quick_play = Some(QuickPlay::Singleplayer {
            world: "mundo".into(),
        });
        let error = build(
            &game("2017-09-18T08:39:46+00:00", &MODERN_ARGS),
            &opts,
            TargetOs::Windows,
        )
        .unwrap_err();
        assert!(matches!(error, Error::QuickPlayUnsupported { .. }));
        let modern = build(
            &game("2023-04-05T12:05:17+00:00", &MODERN_ARGS),
            &opts,
            TargetOs::Windows,
        )
        .unwrap();
        assert!(
            modern
                .args
                .iter()
                .any(|arg| arg == "--quickPlaySingleplayer")
        );
    }

    #[test]
    fn linha_longa_vai_para_argfile_no_java_9_ou_mais() {
        let mut long = game("2023-06-12T13:25:51+00:00", &MODERN_ARGS);
        long.jvm_args[2] = format!("C:\\{}", "x".repeat(WINDOWS_COMMAND_LIMIT));
        let command = build(&long, &options(17), TargetOs::Windows).unwrap();
        let argfile = command.argfile.unwrap();
        assert_eq!(
            argfile.path,
            PathBuf::from("C:\\dados\\instances\\X\\state\\launch-args.txt")
        );
        assert_eq!(
            command.args,
            ["@C:\\dados\\instances\\X\\state\\launch-args.txt"]
        );
        assert!(argfile.contents.starts_with("\"-Xmx4096M\"\n"));
        assert!(argfile.contents.contains("\"C:\\\\xxx"));

        let error = build(&long, &options(8), TargetOs::Windows).unwrap_err();
        assert!(matches!(error, Error::CommandTooLong { java_major: 8, .. }));
        // No Linux não há o limite de 32 767.
        assert!(build(&long, &options(8), TargetOs::Linux).is_ok());
    }

    #[test]
    fn aspas_do_argfile() {
        assert_eq!(argfile_quote("a b"), "\"a b\"");
        assert_eq!(argfile_quote("C:\\x\\y"), "\"C:\\\\x\\\\y\"");
        assert_eq!(argfile_quote("diz \"oi\""), "\"diz \\\"oi\\\"\"");
        assert_eq!(argfile_quote(""), "\"\"");
    }
}
