//! Log4j seguro no servidor local (ARCHITECTURE §7.4: nunca abrir versão até a 1.18 sem
//! configuração log4j segura).
//!
//! O servidor vanilla e o Fabric até a 1.18.2 trazem log4j vulnerável (`Log4Shell`). Mesmo só em
//! `127.0.0.1`, o Warden não abre esses servidores sem proteção: grava na pasta do servidor
//! [`LOG4J_CONFIG_FILE`], uma configuração própria com o mesmo remédio que a Mojang publicou
//! para os servidores antigos — um `RegexFilter` que descarta qualquer mensagem com `${…}`
//! (funciona do log4j 2.0-beta9, do 1.7.10, ao 2.16) — e passa
//! `-Dlog4j.configurationFile=` a ela. O NeoForge e o Forge 1.19+ já vêm corrigidos e mantêm a
//! configuração do loader (o FML perde o `debug.log` sem ela). Todas as faixas recebem também
//! `-Dlog4j2.formatMsgNoLookups=true` (log4j 2.10+).

use std::path::Path;

use warden_core::atomic_write;
use warden_launcher::{LoaderSpec, needs_safe_log4j_config};

use crate::error::Result;

/// A configuração do log4j do Warden, na pasta do servidor.
pub const LOG4J_CONFIG_FILE: &str = "warden-log4j2.xml";

/// Mensagens com `${…}` não são gravadas nem mostradas (o remédio da Mojang para 1.7–1.16).
const CONFIG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- Servidor local do Warden: log4j sem lookups nas mensagens (`Log4Shell`). -->
<Configuration status="WARN">
    <Appenders>
        <Console name="SysOut" target="SYSTEM_OUT">
            <PatternLayout pattern="[%d{HH:mm:ss}] [%t/%level]: %msg%n" />
        </Console>
        <RollingRandomAccessFile name="File" fileName="logs/latest.log" filePattern="logs/%d{yyyy-MM-dd}-%i.log.gz">
            <PatternLayout pattern="[%d{HH:mm:ss}] [%t/%level]: %msg%n" />
            <Policies>
                <TimeBasedTriggeringPolicy />
                <OnStartupTriggeringPolicy />
            </Policies>
        </RollingRandomAccessFile>
    </Appenders>
    <Loggers>
        <Root level="info">
            <filters>
                <RegexFilter regex="(?s).*\$\{[^}]*\}.*" onMatch="DENY" onMismatch="NEUTRAL" />
            </filters>
            <AppenderRef ref="SysOut" />
            <AppenderRef ref="File" />
        </Root>
    </Loggers>
</Configuration>
"#;

/// Grava a configuração na pasta do servidor.
pub(crate) fn write_config(server_dir: &Path) -> Result<()> {
    atomic_write(&server_dir.join(LOG4J_CONFIG_FILE), CONFIG.as_bytes())?;
    Ok(())
}

/// Os argumentos de log4j do servidor.
#[must_use]
pub fn log4j_args(loader: &LoaderSpec, release_time: &str) -> Vec<String> {
    let mut args = vec!["-Dlog4j2.formatMsgNoLookups=true".to_owned()];
    let own_config = !matches!(loader, LoaderSpec::NeoForge { .. })
        && (release_time.is_empty() || needs_safe_log4j_config(release_time));
    if own_config {
        args.push(format!("-Dlog4j.configurationFile={LOG4J_CONFIG_FILE}"));
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versoes_antigas_recebem_a_configuracao_segura() {
        let forge = LoaderSpec::Forge {
            version: "14.23.5.2860".into(),
        };
        let args = log4j_args(&forge, "2017-09-18T08:39:46+00:00");
        assert!(args.contains(&"-Dlog4j.configurationFile=warden-log4j2.xml".to_owned()));
        assert!(args.contains(&"-Dlog4j2.formatMsgNoLookups=true".to_owned()));
        // 1.20.1: o log4j já vem corrigido; fica a configuração do jogo.
        let modern = log4j_args(&LoaderSpec::Vanilla, "2023-06-12T13:25:51+00:00");
        assert_eq!(modern, ["-Dlog4j2.formatMsgNoLookups=true"]);
        // NeoForge nunca recebe configuração própria.
        let neo = LoaderSpec::NeoForge {
            version: "47.1.106".into(),
        };
        assert_eq!(log4j_args(&neo, "").len(), 1);
        // Sem a data, por segurança, a configuração do Warden.
        assert_eq!(log4j_args(&LoaderSpec::Vanilla, "").len(), 2);
    }

    #[test]
    fn configuracao_descarta_lookups() {
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(LOG4J_CONFIG_FILE)).unwrap();
        assert!(text.contains(r#"<RegexFilter regex="(?s).*\$\{[^}]*\}.*" onMatch="DENY""#));
    }
}
