//! Construtores de argv puros (ARCHITECTURE §6.1 e §6.3; QUALITY §11).
//!
//! Só os comandos que o Warden usa existem aqui; os que a ARCHITECTURE §6.1 proíbe (`init`,
//! `modrinth add`, `curseforge add` por busca ou com `-y`, `update`, `remove`…) não têm
//! construtor. Toda chamada leva:
//!
//! - `--pack-file pack.toml` **relativo**, com a pasta do pack como diretório de trabalho.
//!   Com `--pack-file` absoluto, o packwiz ignora os padrões ancorados do `.packwizignore`
//!   (`/logs/` etc.; evidência em `crates/warden-packwiz/tests/fixtures/ignore-100`) e o `add`
//!   exige `--meta-folder-base` absoluto (`third_party/packwiz/README.md`). A forma relativa
//!   evita os dois defeitos;
//! - `--cache` e `--config` do Warden (isolam o cache e as configurações globais do usuário);
//! - nada de segredo: a chave da CurseForge vai só pela variável de ambiente
//!   [`CURSEFORGE_KEY_ENV`], definida apenas nos comandos que falam com a CurseForge
//!   ([`PackwizCommand::needs_curseforge_key`]).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use warden_packwiz::PACK_FILE;

/// Variável de ambiente lida pelo sidecar com o patch 0001.
pub const CURSEFORGE_KEY_ENV: &str = "WARDEN_CURSEFORGE_API_KEY";

/// Variáveis do ambiente do Warden repassadas ao packwiz; todas as outras são apagadas
/// (ARCHITECTURE §6.3). Inclui `PACKWIZ_*`, que o `viper` do packwiz leria como opções.
pub const INHERITED_ENV: [&str; 8] = [
    "PATH",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "HOME",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
];

/// Lado de uma exportação da CurseForge (`curseforge export --side`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportSide {
    /// Só o que vai no cliente (padrão do packwiz).
    Client,
    /// Só o que vai no servidor.
    Server,
    /// Tudo.
    Both,
}

impl ExportSide {
    /// Texto aceito pelo packwiz.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
            Self::Both => "both",
        }
    }
}

/// Um comando do packwiz que o Warden executa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackwizCommand {
    /// `refresh`, ou `refresh --build` (gera os hashes mesmo com `no-internal-hashes`).
    Refresh {
        /// `--build`.
        build: bool,
    },
    /// `curseforge add <link>`: link de **arquivo** da CurseForge (ARCHITECTURE §6.1).
    CurseForgeAddUrl {
        /// O link.
        url: String,
    },
    /// `curseforge add --addon-id <projeto> --file-id <arquivo>`: versão escolhida no Warden.
    CurseForgeAddIds {
        /// ID do projeto.
        addon_id: u32,
        /// ID do arquivo.
        file_id: u32,
    },
    /// `list`.
    List,
    /// `modrinth export --output <arquivo>`.
    ModrinthExport {
        /// Arquivo `.mrpack` gerado (relativo à pasta do pack ou absoluto).
        output: PathBuf,
    },
    /// `curseforge export --side <lado> --output <arquivo>`.
    CurseForgeExport {
        /// Lado exportado.
        side: ExportSide,
        /// Arquivo `.zip` gerado (relativo à pasta do pack ou absoluto).
        output: PathBuf,
    },
}

impl PackwizCommand {
    /// Nome curto do comando, usado em registros e erros.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Refresh { build: false } => "refresh",
            Self::Refresh { build: true } => "refresh --build",
            Self::CurseForgeAddUrl { .. } | Self::CurseForgeAddIds { .. } => "curseforge add",
            Self::List => "list",
            Self::ModrinthExport { .. } => "modrinth export",
            Self::CurseForgeExport { .. } => "curseforge export",
        }
    }

    /// Argumentos do subcomando (depois das opções globais).
    #[must_use]
    pub fn subcommand_args(&self) -> Vec<OsString> {
        let strings = |items: &[&str]| items.iter().map(OsString::from).collect::<Vec<_>>();
        match self {
            Self::Refresh { build } => {
                let mut args = strings(&["refresh"]);
                if *build {
                    args.push("--build".into());
                }
                args
            }
            Self::CurseForgeAddUrl { url } => {
                let mut args = strings(&["curseforge", "add"]);
                // `--` impede que um link começando por `-` vire opção.
                args.push("--".into());
                args.push(url.into());
                args
            }
            Self::CurseForgeAddIds { addon_id, file_id } => strings(&[
                "curseforge",
                "add",
                "--addon-id",
                &addon_id.to_string(),
                "--file-id",
                &file_id.to_string(),
            ]),
            Self::List => strings(&["list"]),
            Self::ModrinthExport { output } => {
                let mut args = strings(&["modrinth", "export", "--output"]);
                args.push(output.into());
                args
            }
            Self::CurseForgeExport { side, output } => {
                let mut args = strings(&["curseforge", "export", "--side", side.as_str()]);
                args.push("--output".into());
                args.push(output.into());
                args
            }
        }
    }

    /// Se o comando pode falar com a CurseForge e, portanto, recebe a chave: o `add` sempre;
    /// as exportações baixam os mods da CurseForge pela API (`metadata:curseforge`).
    #[must_use]
    pub fn needs_curseforge_key(&self) -> bool {
        matches!(
            self,
            Self::CurseForgeAddUrl { .. }
                | Self::CurseForgeAddIds { .. }
                | Self::ModrinthExport { .. }
                | Self::CurseForgeExport { .. }
        )
    }

    /// O que vai para a entrada padrão. O `curseforge add` responde `n` à pergunta "Would
    /// you like to add them? [Y/n]" (dependências): o Warden resolve dependências pela API
    /// (ARCHITECTURE §6.1). Uma resposta só: o packwiz cria um leitor novo por pergunta e o
    /// primeiro engoliria as demais.
    #[must_use]
    pub fn stdin(&self) -> Stdin {
        match self {
            Self::CurseForgeAddUrl { .. } | Self::CurseForgeAddIds { .. } => Stdin::Answers(b"n\n"),
            _ => Stdin::Closed,
        }
    }

    /// Tempo-limite do comando.
    #[must_use]
    pub fn timeout(&self, timeouts: &Timeouts) -> Duration {
        match self {
            Self::Refresh { .. } => timeouts.refresh,
            Self::CurseForgeAddUrl { .. } | Self::CurseForgeAddIds { .. } => {
                timeouts.curseforge_add
            }
            Self::List => timeouts.list,
            Self::ModrinthExport { .. } | Self::CurseForgeExport { .. } => timeouts.export,
        }
    }
}

/// Entrada padrão do processo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stdin {
    /// Fechada (o packwiz recebe fim de arquivo se perguntar algo).
    Closed,
    /// Estes bytes e depois fim de arquivo.
    Answers(&'static [u8]),
}

/// Tempos-limite por tipo de comando (ARCHITECTURE §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// `refresh`: 120 s.
    pub refresh: Duration,
    /// `curseforge add`: 120 s.
    pub curseforge_add: Duration,
    /// `list`: 120 s (lê só os metafiles, mas um pack enorme num disco lento demora).
    pub list: Duration,
    /// Exportações: 600 s.
    pub export: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            refresh: Duration::from_secs(120),
            curseforge_add: Duration::from_secs(120),
            list: Duration::from_secs(120),
            export: Duration::from_secs(600),
        }
    }
}

impl Timeouts {
    /// O mesmo limite para todos os comandos (testes).
    #[must_use]
    pub fn all(limit: Duration) -> Self {
        Self {
            refresh: limit,
            curseforge_add: limit,
            list: limit,
            export: limit,
        }
    }
}

/// Tudo o que define uma execução, exceto a chave: argv, ambiente herdado, entrada padrão,
/// pasta de trabalho e tempo-limite. Função pura do comando e das pastas, testável sem
/// processo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// Argumentos completos (sem o executável).
    pub args: Vec<OsString>,
    /// Variáveis repassadas (nome, valor), já filtradas por [`INHERITED_ENV`].
    pub env: Vec<(&'static str, OsString)>,
    /// Se a chave da CurseForge deve ser posta no ambiente (quando o chamador a tiver).
    pub pass_curseforge_key: bool,
    /// Entrada padrão.
    pub stdin: Stdin,
    /// Pasta de trabalho: a pasta do pack.
    pub cwd: PathBuf,
    /// Tempo-limite.
    pub timeout: Duration,
}

impl Invocation {
    /// Monta a execução de `command` no pack em `pack_dir`. `env` lê o ambiente do Warden
    /// (no app, `std::env::var_os`).
    pub fn new(
        command: &PackwizCommand,
        pack_dir: &Path,
        cache_dir: &Path,
        config_file: &Path,
        timeouts: &Timeouts,
        env: impl Fn(&str) -> Option<OsString>,
    ) -> Self {
        let mut args: Vec<OsString> = vec![
            "--config".into(),
            config_file.into(),
            "--cache".into(),
            cache_dir.into(),
            "--pack-file".into(),
            PACK_FILE.into(),
        ];
        args.extend(command.subcommand_args());
        let env = INHERITED_ENV
            .iter()
            .filter_map(|name| {
                env(name)
                    .filter(|value| !value.is_empty())
                    .map(|value| (*name, value))
            })
            .collect();
        Self {
            args,
            env,
            pass_curseforge_key: command.needs_curseforge_key(),
            stdin: command.stdin(),
            cwd: pack_dir.to_path_buf(),
            timeout: command.timeout(timeouts),
        }
    }

    /// Os argumentos como texto, para o registro (`debug`).
    #[must_use]
    pub fn display_args(&self) -> String {
        self.args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn invocation(command: &PackwizCommand) -> Invocation {
        Invocation::new(
            command,
            Path::new("pack"),
            Path::new("cache"),
            Path::new("cfg/packwiz.toml"),
            &Timeouts::default(),
            |name| match name {
                "PATH" => Some("C:\\Windows".into()),
                "TEMP" => Some("".into()),
                "PACKWIZ_YES" | "WARDEN_CURSEFORGE_API_KEY" => Some("1".into()),
                _ => None,
            },
        )
    }

    #[test]
    fn argv_de_cada_comando() {
        let cases: Vec<(PackwizCommand, Vec<&str>)> = vec![
            (PackwizCommand::Refresh { build: false }, vec!["refresh"]),
            (
                PackwizCommand::Refresh { build: true },
                vec!["refresh", "--build"],
            ),
            (
                PackwizCommand::CurseForgeAddUrl {
                    url: "https://www.curseforge.com/minecraft/mc-mods/jei/files/123".to_owned(),
                },
                vec![
                    "curseforge",
                    "add",
                    "--",
                    "https://www.curseforge.com/minecraft/mc-mods/jei/files/123",
                ],
            ),
            (
                PackwizCommand::CurseForgeAddIds {
                    addon_id: 238_222,
                    file_id: 5_101_366,
                },
                vec![
                    "curseforge",
                    "add",
                    "--addon-id",
                    "238222",
                    "--file-id",
                    "5101366",
                ],
            ),
            (PackwizCommand::List, vec!["list"]),
            (
                PackwizCommand::ModrinthExport {
                    output: PathBuf::from("saida.mrpack"),
                },
                vec!["modrinth", "export", "--output", "saida.mrpack"],
            ),
            (
                PackwizCommand::CurseForgeExport {
                    side: ExportSide::Both,
                    output: PathBuf::from("saida.zip"),
                },
                vec![
                    "curseforge",
                    "export",
                    "--side",
                    "both",
                    "--output",
                    "saida.zip",
                ],
            ),
        ];
        for (command, sub) in cases {
            let inv = invocation(&command);
            let mut expected = vec![
                "--config",
                "cfg/packwiz.toml",
                "--cache",
                "cache",
                "--pack-file",
                "pack.toml",
            ];
            expected.extend(sub);
            assert_eq!(strings(&inv.args), expected, "{command:?}");
            assert_eq!(inv.cwd, Path::new("pack"));
            assert!(!command.label().is_empty());
        }
    }

    #[test]
    fn nenhum_comando_proibido_tem_construtor() {
        // Os subcomandos possíveis são só estes (ARCHITECTURE §6.1, "nunca usados").
        let allowed = ["refresh", "curseforge", "list", "modrinth"];
        let commands = [
            PackwizCommand::Refresh { build: true },
            PackwizCommand::CurseForgeAddUrl { url: "u".into() },
            PackwizCommand::CurseForgeAddIds {
                addon_id: 1,
                file_id: 2,
            },
            PackwizCommand::List,
            PackwizCommand::ModrinthExport { output: "o".into() },
            PackwizCommand::CurseForgeExport {
                side: ExportSide::Client,
                output: "o".into(),
            },
        ];
        for command in commands {
            let args = strings(&command.subcommand_args());
            assert!(allowed.contains(&args[0].as_str()));
            assert!(!args.iter().any(|arg| arg == "-y" || arg == "--yes"));
        }
    }

    #[test]
    fn ambiente_so_com_a_lista_permitida() {
        let inv = invocation(&PackwizCommand::Refresh { build: false });
        // `TEMP` vazio não passa; `PACKWIZ_*` e a chave nunca vêm do ambiente do Warden.
        assert_eq!(inv.env, vec![("PATH", OsString::from("C:\\Windows"))]);
        assert!(!inv.pass_curseforge_key);
    }

    #[test]
    fn chave_so_nos_comandos_da_curseforge() {
        assert!(!PackwizCommand::Refresh { build: true }.needs_curseforge_key());
        assert!(!PackwizCommand::List.needs_curseforge_key());
        assert!(PackwizCommand::CurseForgeAddUrl { url: "u".into() }.needs_curseforge_key());
        assert!(PackwizCommand::ModrinthExport { output: "o".into() }.needs_curseforge_key());
    }

    #[test]
    fn stdin_e_tempos() {
        let timeouts = Timeouts::default();
        let add = PackwizCommand::CurseForgeAddIds {
            addon_id: 1,
            file_id: 2,
        };
        assert_eq!(add.stdin(), Stdin::Answers(b"n\n"));
        assert_eq!(add.timeout(&timeouts), Duration::from_secs(120));
        assert_eq!(PackwizCommand::List.stdin(), Stdin::Closed);
        assert_eq!(
            PackwizCommand::CurseForgeExport {
                side: ExportSide::Server,
                output: "o".into()
            }
            .timeout(&timeouts),
            Duration::from_secs(600)
        );
        assert_eq!(
            PackwizCommand::Refresh { build: false }
                .timeout(&Timeouts::all(Duration::from_secs(3))),
            Duration::from_secs(3)
        );
        assert_eq!(ExportSide::Server.as_str(), "server");
        assert_eq!(ExportSide::Client.as_str(), "client");
    }

    #[test]
    fn exibicao_dos_argumentos() {
        let inv = invocation(&PackwizCommand::List);
        assert_eq!(
            inv.display_args(),
            "--config cfg/packwiz.toml --cache cache --pack-file pack.toml list"
        );
    }
}
