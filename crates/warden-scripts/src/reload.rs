//! "Recarregar no jogo" (SPEC T26; ARCHITECTURE §10.2; R5B §3.5): o caminho certo para cada
//! ferramenta, versão e situação do teste.
//!
//! | Situação | Caminho |
//! |---|---|
//! | Sem jogo nem servidor local aberto | nada (o botão fica desabilitado com o motivo) |
//! | CraftTweaker do 1.12.2 (linha 4 ou anterior) | não há recarga oficial: "reinicie o jogo" |
//! | KubeJS 7+, jogo aberto, servidor web configurado, script de servidor ou de startup | `POST /api/reload/<tipo>` ([`crate::web`]) |
//! | Servidor local aberto (L-09) | o comando sem a barra pelo console do servidor ([`ServerConsole`]) |
//! | Os demais | o comando certo para copiar |
//!
//! Comandos: KubeJS até a linha 6, `/kubejs reload <tipo>_scripts`; KubeJS 7+, `/reload` (o
//! mesmo que o servidor web executa) para scripts de servidor e `/kubejs reload startup` ou
//! `client` para os outros; CraftTweaker moderno, `/reload` (o `/ct reload` só manda usar o
//! `/reload`).

use async_trait::async_trait;
use serde::Serialize;

use crate::detect::{KubejsScriptType, ScriptEnv, ScriptFile, ScriptTool};
use crate::error::{Error, Result};
use crate::web::KubejsWeb;

/// Como recarregar o script aberto agora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ReloadRoute {
    /// O arquivo não é script de uma ferramenta do pack.
    NotAScript,
    /// Nem o jogo nem o servidor local estão abertos para este pack.
    NoGame,
    /// O CraftTweaker do 1.12.2 não recarrega: é preciso reiniciar o jogo.
    RestartGame,
    /// Pelo servidor web do KubeJS 7.
    WebServer {
        /// Tipo de script recarregado.
        script_type: KubejsScriptType,
    },
    /// Pelo console do servidor local.
    ServerConsole {
        /// O comando, sem a barra.
        command: String,
    },
    /// O comando para a pessoa copiar e colar no chat do jogo.
    CopyCommand {
        /// O comando, com a barra.
        command: String,
    },
}

/// A situação do teste para decidir o caminho.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReloadContext {
    /// O jogo do teste deste pack está aberto.
    pub game_running: bool,
    /// O servidor local deste pack está aberto (L-09).
    pub server_console: bool,
    /// A instância tem a configuração do servidor web do KubeJS (com token).
    pub web_server: bool,
}

/// O comando de recarga do script, com a barra, ou `None` se não há recarga.
#[must_use]
pub fn reload_command(env: &ScriptEnv, path: &str) -> Option<String> {
    let file = ScriptFile::classify(path)?;
    let tool = env.tool_for(path)?;
    match (tool.tool, file) {
        (ScriptTool::Kubejs, ScriptFile::Kubejs(script_type)) => {
            if tool.major.is_some_and(|major| major >= 7) {
                Some(match script_type {
                    KubejsScriptType::Server => "/reload".to_owned(),
                    other => format!("/kubejs reload {}", other.name()),
                })
            } else {
                Some(format!("/kubejs reload {}_scripts", script_type.name()))
            }
        }
        (ScriptTool::Crafttweaker, ScriptFile::Crafttweaker) => {
            (!classic_crafttweaker(env)).then(|| "/reload".to_owned())
        }
        _ => None,
    }
}

/// CraftTweaker do 1.12.2 ou anterior (sem recarga oficial).
fn classic_crafttweaker(env: &ScriptEnv) -> bool {
    env.crafttweaker
        .as_ref()
        .and_then(|tool| tool.major)
        .is_some_and(|major| major <= 4)
}

/// O caminho de recarga do script `path` (relativo ao pack).
#[must_use]
pub fn plan_reload(env: &ScriptEnv, path: &str, context: ReloadContext) -> ReloadRoute {
    let Some(file) = ScriptFile::classify(path) else {
        return ReloadRoute::NotAScript;
    };
    let Some(tool) = env.tool_for(path) else {
        return ReloadRoute::NotAScript;
    };
    if !context.game_running && !context.server_console {
        return ReloadRoute::NoGame;
    }
    if file == ScriptFile::Crafttweaker && classic_crafttweaker(env) {
        return ReloadRoute::RestartGame;
    }
    if let ScriptFile::Kubejs(script_type) = file {
        let web_type = matches!(
            script_type,
            KubejsScriptType::Server | KubejsScriptType::Startup
        );
        if context.game_running
            && context.web_server
            && web_type
            && tool.major.is_some_and(|major| major >= 7)
        {
            return ReloadRoute::WebServer { script_type };
        }
    }
    let Some(command) = reload_command(env, path) else {
        return ReloadRoute::NotAScript;
    };
    // O servidor não carrega scripts de cliente: esses só pelo chat do jogo.
    let client = file == ScriptFile::Kubejs(KubejsScriptType::Client);
    if context.server_console && !client {
        return ReloadRoute::ServerConsole {
            command: command.trim_start_matches('/').to_owned(),
        };
    }
    if !context.game_running {
        return ReloadRoute::NoGame;
    }
    ReloadRoute::CopyCommand { command }
}

/// O console (stdin) do servidor local aberto. **Ponto de extensão da L-09**: o servidor local
/// implementa este trait e o comando `scripts_reload` o usa quando ele está aberto.
#[async_trait]
pub trait ServerConsole: Send + Sync {
    /// Manda um comando (sem a barra) e devolve as linhas que o servidor respondeu.
    async fn send(&self, command: &str) -> std::result::Result<Vec<String>, String>;
}

/// O que a recarga fez.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReloadOutcome {
    /// O caminho usado (ou a explicar, quando nada pôde ser feito sozinho).
    pub route: ReloadRoute,
    /// O que o servidor local respondeu (vazio nos outros caminhos).
    pub response: Vec<String>,
}

/// Recarrega pelo caminho dado. Os caminhos que dependem da pessoa (copiar, reiniciar) voltam
/// como estão.
///
/// # Errors
///
/// Os do servidor web ([`KubejsWeb::reload`]) e [`Error::ServerConsole`].
pub async fn execute(
    route: ReloadRoute,
    web: Option<&KubejsWeb>,
    console: Option<&dyn ServerConsole>,
) -> Result<ReloadOutcome> {
    let response = match &route {
        ReloadRoute::WebServer { script_type } => {
            let web = web.ok_or_else(|| Error::Internal("servidor web sem cliente".into()))?;
            web.reload(*script_type).await?;
            Vec::new()
        }
        ReloadRoute::ServerConsole { command } => {
            let console =
                console.ok_or_else(|| Error::Internal("servidor local sem console".into()))?;
            console
                .send(command)
                .await
                .map_err(|detail| Error::ServerConsole { detail })?
        }
        _ => Vec::new(),
    };
    Ok(ReloadOutcome { route, response })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::detect::ToolVersion;

    fn env(kubejs: Option<u32>, crafttweaker: Option<u32>) -> ScriptEnv {
        ScriptEnv {
            kubejs: kubejs.map(|major| ToolVersion {
                tool: ScriptTool::Kubejs,
                major: Some(major),
                version: None,
            }),
            crafttweaker: crafttweaker.map(|major| ToolVersion {
                tool: ScriptTool::Crafttweaker,
                major: Some(major),
                version: None,
            }),
        }
    }

    const GAME: ReloadContext = ReloadContext {
        game_running: true,
        server_console: false,
        web_server: false,
    };

    fn copy(command: &str) -> ReloadRoute {
        ReloadRoute::CopyCommand {
            command: command.into(),
        }
    }

    #[test]
    fn kubejs_6_so_com_o_cliente_mostra_o_comando() {
        let six = env(Some(6), None);
        assert_eq!(
            plan_reload(&six, "kubejs/server_scripts/receitas.js", GAME),
            copy("/kubejs reload server_scripts")
        );
        assert_eq!(
            plan_reload(&six, "kubejs/startup_scripts/itens.js", GAME),
            copy("/kubejs reload startup_scripts")
        );
        // Mesmo com o arquivo do servidor web (não existe no 6), continua o comando.
        let with_file = ReloadContext {
            web_server: true,
            ..GAME
        };
        assert_eq!(
            plan_reload(&six, "kubejs/server_scripts/receitas.js", with_file),
            copy("/kubejs reload server_scripts")
        );
    }

    #[test]
    fn com_o_servidor_local_vai_pelo_console() {
        let server = ReloadContext {
            game_running: true,
            server_console: true,
            web_server: false,
        };
        assert_eq!(
            plan_reload(&env(Some(6), None), "kubejs/server_scripts/a.js", server),
            ReloadRoute::ServerConsole {
                command: "kubejs reload server_scripts".into()
            }
        );
        assert_eq!(
            plan_reload(&env(Some(7), None), "kubejs/server_scripts/a.js", server),
            ReloadRoute::ServerConsole {
                command: "reload".into()
            }
        );
        assert_eq!(
            plan_reload(&env(None, Some(14)), "scripts/a.zs", server),
            ReloadRoute::ServerConsole {
                command: "reload".into()
            }
        );
        // Só o servidor aberto (sem o cliente): ainda vai pelo console.
        let only_server = ReloadContext {
            game_running: false,
            ..server
        };
        assert!(matches!(
            plan_reload(
                &env(Some(6), None),
                "kubejs/server_scripts/a.js",
                only_server
            ),
            ReloadRoute::ServerConsole { .. }
        ));
        // Script de cliente: só pelo chat; sem o cliente aberto, nada.
        assert_eq!(
            plan_reload(&env(Some(6), None), "kubejs/client_scripts/a.js", server),
            copy("/kubejs reload client_scripts")
        );
        assert_eq!(
            plan_reload(
                &env(Some(6), None),
                "kubejs/client_scripts/a.js",
                only_server
            ),
            ReloadRoute::NoGame
        );
    }

    #[test]
    fn kubejs_7_com_o_jogo_aberto_usa_o_servidor_web() {
        let seven = env(Some(7), None);
        let web = ReloadContext {
            web_server: true,
            ..GAME
        };
        assert_eq!(
            plan_reload(&seven, "kubejs/server_scripts/a.js", web),
            ReloadRoute::WebServer {
                script_type: KubejsScriptType::Server
            }
        );
        assert_eq!(
            plan_reload(&seven, "kubejs/startup_scripts/a.js", web),
            ReloadRoute::WebServer {
                script_type: KubejsScriptType::Startup
            }
        );
        assert_eq!(
            plan_reload(&seven, "kubejs/client_scripts/a.js", web),
            copy("/kubejs reload client")
        );
        // Sem o arquivo do token, o comando.
        assert_eq!(
            plan_reload(&seven, "kubejs/server_scripts/a.js", GAME),
            copy("/reload")
        );
    }

    #[test]
    fn crafttweaker_e_casos_sem_recarga() {
        assert_eq!(
            plan_reload(&env(None, Some(14)), "scripts/a.zs", GAME),
            copy("/reload")
        );
        assert_eq!(
            plan_reload(&env(None, Some(4)), "scripts/a.zs", GAME),
            ReloadRoute::RestartGame
        );
        assert_eq!(
            plan_reload(
                &env(Some(6), None),
                "kubejs/server_scripts/a.js",
                ReloadContext::default()
            ),
            ReloadRoute::NoGame
        );
        // Pack sem a ferramenta do arquivo, ou arquivo que não é script.
        assert_eq!(
            plan_reload(&env(Some(6), None), "scripts/a.zs", GAME),
            ReloadRoute::NotAScript
        );
        assert_eq!(
            plan_reload(&env(Some(6), Some(14)), "config/a.toml", GAME),
            ReloadRoute::NotAScript
        );
        assert_eq!(reload_command(&env(None, Some(4)), "scripts/a.zs"), None);
    }

    struct FakeConsole(Mutex<Vec<String>>);

    #[async_trait]
    impl ServerConsole for FakeConsole {
        async fn send(&self, command: &str) -> std::result::Result<Vec<String>, String> {
            self.0.lock().unwrap().push(command.to_owned());
            if command == "quebra" {
                return Err("o servidor fechou".into());
            }
            Ok(vec!["[15:12:03] [Server thread/INFO]: Reloading!".into()])
        }
    }

    #[tokio::test]
    async fn executa_pelo_console_do_servidor() {
        let console = FakeConsole(Mutex::new(Vec::new()));
        let outcome = execute(
            ReloadRoute::ServerConsole {
                command: "kubejs reload server_scripts".into(),
            },
            None,
            Some(&console),
        )
        .await
        .unwrap();
        assert_eq!(
            outcome.response,
            ["[15:12:03] [Server thread/INFO]: Reloading!"]
        );
        assert_eq!(
            console.0.lock().unwrap().as_slice(),
            ["kubejs reload server_scripts"]
        );
        let error = execute(
            ReloadRoute::ServerConsole {
                command: "quebra".into(),
            },
            None,
            Some(&console),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, Error::ServerConsole { .. }));
        // Caminhos que dependem da pessoa voltam como estão.
        let outcome = execute(copy("/reload"), None, None).await.unwrap();
        assert_eq!(outcome.route, copy("/reload"));
        assert!(
            execute(
                ReloadRoute::WebServer {
                    script_type: KubejsScriptType::Server
                },
                None,
                None
            )
            .await
            .is_err()
        );
    }
}
