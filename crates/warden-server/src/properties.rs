//! O `server.properties` controlado e a porta livre (ARCHITECTURE §7.6; ADR-0032).
//!
//! O que o pack traz no `server.properties` (dificuldade, distância de visão…) é mantido, na
//! ordem; as chaves de segurança são sempre do Warden: só `127.0.0.1`, porta livre testada,
//! `online-mode=false` (o jogador do teste é offline), RCON e query desligados, sem exigir
//! perfil assinado e o `motd` com o nome do pack.

use std::fmt::Write as _;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::Path;

use warden_core::atomic_write;

use crate::error::{Error, Result};

/// O arquivo de propriedades do servidor.
pub const PROPERTIES_FILE: &str = "server.properties";

/// O endereço em que o servidor local escuta (só este computador).
pub const LOCAL_HOST: &str = "127.0.0.1";

/// Primeira porta tentada (a padrão do Minecraft).
pub const FIRST_PORT: u16 = 25_565;

/// Quantas portas seguidas são tentadas antes de pedir uma qualquer ao sistema.
const PORT_ATTEMPTS: u16 = 100;

/// As chaves que o Warden sempre escreve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlledProperties {
    /// Porta livre em `127.0.0.1`.
    pub port: u16,
    /// Mensagem do servidor (o nome do pack).
    pub motd: String,
}

impl ControlledProperties {
    /// As chaves e os valores, na ordem em que entram num arquivo novo.
    #[must_use]
    pub fn entries(&self) -> Vec<(&'static str, String)> {
        vec![
            ("server-ip", LOCAL_HOST.to_owned()),
            ("server-port", self.port.to_string()),
            ("online-mode", "false".to_owned()),
            ("enable-rcon", "false".to_owned()),
            ("enable-query", "false".to_owned()),
            ("enforce-secure-profile", "false".to_owned()),
            ("motd", escape_value(&self.motd)),
        ]
    }
}

/// `motd` em uma linha, no formato do `java.util.Properties` (sem quebras, `\` escapado e o
/// que não é ASCII como `\uXXXX`, porque o servidor lê o arquivo em ISO-8859-1).
fn escape_value(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            ' '..='~' => out.push(character),
            other => {
                let mut units = [0u16; 2];
                for unit in other.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out
}

/// A chave de uma linha do `server.properties` (`None` em comentários e linhas vazias).
fn key_of(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
        return None;
    }
    let end = trimmed.find(['=', ':', ' ', '\t']).unwrap_or(trimmed.len());
    Some(trimmed[..end].trim())
}

/// O `server.properties` final: o do pack (`existing`) com as chaves controladas trocadas no
/// lugar e as que faltam acrescentadas no fim.
#[must_use]
pub fn render(existing: Option<&str>, controlled: &ControlledProperties) -> String {
    let entries = controlled.entries();
    let mut written = vec![false; entries.len()];
    let mut out = String::new();
    for line in existing.unwrap_or_default().lines() {
        let position = key_of(line).and_then(|key| entries.iter().position(|(k, _)| *k == key));
        if let Some(index) = position {
            if !written[index] {
                let (key, value) = &entries[index];
                let _ = writeln!(out, "{key}={value}");
                written[index] = true;
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if existing.is_none_or(str::is_empty) {
        out.push_str("# Servidor local do Warden: só este computador (127.0.0.1).\n");
    }
    for (index, (key, value)) in entries.iter().enumerate() {
        if !written[index] {
            let _ = writeln!(out, "{key}={value}");
        }
    }
    out
}

/// Grava o `server.properties` da pasta do servidor (mantendo o que já estava, que é o do pack
/// quando a materialização o trouxe).
pub fn write_properties(server_dir: &Path, controlled: &ControlledProperties) -> Result<()> {
    let path = server_dir.join(PROPERTIES_FILE);
    let existing = match std::fs::read(&path) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(Error::io("ler o server.properties", &path, error)),
    };
    atomic_write(&path, render(existing.as_deref(), controlled).as_bytes())?;
    Ok(())
}

/// Se a porta está livre em `127.0.0.1` agora.
#[must_use]
pub fn port_is_free(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).is_ok()
}

/// Uma porta livre em `127.0.0.1`: a primeira livre a partir de 25565 (fácil de reconhecer) e,
/// se as 100 estiverem ocupadas, uma escolhida pelo sistema.
pub fn free_port() -> Result<u16> {
    free_port_from(FIRST_PORT)
}

/// [`free_port`] a partir de `first`.
pub fn free_port_from(first: u16) -> Result<u16> {
    for offset in 0..PORT_ATTEMPTS {
        let Some(port) = first.checked_add(offset) else {
            break;
        };
        if port_is_free(port) {
            return Ok(port);
        }
    }
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|_| Error::NoFreePort)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controlled() -> ControlledProperties {
        ControlledProperties {
            port: 25_570,
            motd: "Vale Sereno".into(),
        }
    }

    #[test]
    fn ca_t27_01_arquivo_novo_tem_as_chaves_de_seguranca() {
        let text = render(None, &controlled());
        for line in [
            "server-ip=127.0.0.1",
            "server-port=25570",
            "online-mode=false",
            "enable-rcon=false",
            "enable-query=false",
            "motd=Vale Sereno",
        ] {
            assert!(text.lines().any(|l| l == line), "{line} em {text}");
        }
    }

    #[test]
    fn mantem_o_do_pack_e_sobrescreve_as_controladas() {
        let pack = "#Minecraft server properties\ndifficulty=hard\nserver-ip=0.0.0.0\nonline-mode = true\nenable-rcon=true\nrcon.password=segredo\nview-distance=6\nserver-ip=1.2.3.4\n";
        let text = render(Some(pack), &controlled());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "#Minecraft server properties");
        assert_eq!(lines[1], "difficulty=hard");
        assert_eq!(lines[2], "server-ip=127.0.0.1");
        assert_eq!(lines[3], "online-mode=false");
        assert_eq!(lines[4], "enable-rcon=false");
        assert!(text.contains("view-distance=6"));
        // A chave repetida some; nada aponta para fora do computador.
        assert_eq!(text.matches("server-ip=").count(), 1);
        assert!(!text.contains("0.0.0.0") && !text.contains("1.2.3.4"));
        assert!(text.contains("enable-query=false") && text.contains("server-port=25570"));
    }

    #[test]
    fn motd_com_acentos_vira_escape_do_properties() {
        let text = render(
            None,
            &ControlledProperties {
                port: 1,
                motd: "Vale Sereno: ação\\n".into(),
            },
        );
        assert!(
            text.contains("motd=Vale Sereno: a\\u00e7\\u00e3o\\\\n"),
            "{text}"
        );
    }

    #[test]
    fn grava_por_cima_do_arquivo_do_pack() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(PROPERTIES_FILE),
            "pvp=false\nserver-port=1\n",
        )
        .unwrap();
        write_properties(dir.path(), &controlled()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(PROPERTIES_FILE)).unwrap();
        assert!(text.starts_with("pvp=false\nserver-port=25570\n"), "{text}");
    }

    #[test]
    fn ca_t27_01_porta_livre_pula_a_ocupada() {
        let busy = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let taken = busy.local_addr().unwrap().port();
        assert!(!port_is_free(taken));
        let port = free_port_from(taken).unwrap();
        assert_ne!(port, taken);
        assert!(port_is_free(port));
        drop(busy);
        assert!(port_is_free(taken));
    }
}
