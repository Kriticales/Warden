//! Envio ao GitHub pelo protocolo "smart HTTP" do git (`git-receive-pack`), sobre a
//! `warden-http` (ADR-0059).
//!
//! Por que não o `push` da libgit2: com o `https` da `git2` ligado, o Linux compilaria a
//! libgit2 com OpenSSL e o Windows com o `WinHTTP` — duas pilhas de TLS fora da `warden-http`
//! (rustls, proxy, User-Agent e limites do Warden) e um caminho de credencial por callback.
//! Aqui a libgit2 só monta o pacote de objetos ([`git2::PackBuilder`]); o pedido HTTP é o
//! mesmo do git oficial:
//!
//! 1. `GET <repo>.git/info/refs?service=git-receive-pack` → as referências do servidor;
//! 2. `POST <repo>.git/git-receive-pack` com `<antigo> <novo> <ref>` de cada referência, o
//!    pacote e a capacidade `report-status`, e a resposta diz `ok`/`ng` por referência.
//!
//! O valor antigo de cada referência vai no pedido, e o servidor só aceita se ele ainda for o
//! atual: é a troca protegida ("lease") da ARCHITECTURE §11.1 item 5, feita pelo próprio
//! servidor, sem janela de corrida. A credencial vai só no cabeçalho `Authorization`,
//! marcado como sensível (CA-T18-02).

use std::collections::BTreeMap;
use std::time::Duration;

use git2::Oid;
use url::Url;
use warden_core::CancellationToken;
use warden_http::{HeaderName, HeaderValue};

use crate::error::{Error, Result};
use crate::github::{GithubClient, GithubPermission, RepoRef, http_error};

/// Id "nenhum objeto" do git (referência que ainda não existe).
const ZERO: &str = "0000000000000000000000000000000000000000";

/// Maior resposta aceita na lista de referências (repositórios do Warden têm poucas).
const MAX_ADVERTISEMENT: u64 = 8 * 1024 * 1024;

/// O envio do pacote pode levar mais que os 30 s das chamadas de API.
const PUSH_TIMEOUT: Duration = Duration::from_mins(15);

/// Uma referência a atualizar no servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefUpdate {
    /// Nome completo (`refs/heads/main`).
    pub(crate) name: String,
    /// Valor que o servidor precisa ter agora (`None`: a referência não pode existir).
    pub(crate) old: Option<Oid>,
    /// Valor novo.
    pub(crate) new: Oid,
}

/// Uma linha do protocolo (`pkt-line`): 4 dígitos hexadecimais com o tamanho total.
pub(crate) fn pkt_line(out: &mut Vec<u8>, payload: &[u8]) {
    let len = payload.len() + 4;
    out.extend_from_slice(format!("{len:04x}").as_bytes());
    out.extend_from_slice(payload);
}

/// Lê as linhas `pkt-line` de `data`: `None` = `flush` (`0000`).
///
/// Erros: [`Error::Internal`] com o trecho malformado.
pub(crate) fn read_pkt_lines(mut data: &[u8]) -> Result<Vec<Option<Vec<u8>>>> {
    let mut lines = Vec::new();
    while !data.is_empty() {
        let head = data
            .get(..4)
            .and_then(|head| std::str::from_utf8(head).ok())
            .and_then(|head| usize::from_str_radix(head, 16).ok())
            .ok_or_else(|| Error::Internal("resposta do servidor git malformada".into()))?;
        if head == 0 {
            lines.push(None);
            data = &data[4..];
            continue;
        }
        if head < 4 || head > data.len() {
            return Err(Error::Internal("linha do servidor git malformada".into()));
        }
        lines.push(Some(data[4..head].to_vec()));
        data = &data[head..];
    }
    Ok(lines)
}

fn protocol_error(text: impl Into<String>) -> Error {
    Error::Internal(format!("protocolo git: {}", text.into()))
}

/// O anúncio de `info/refs`: as referências do servidor e se ele aceita o envio atômico.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Advertisement {
    /// Nome → id.
    pub(crate) refs: BTreeMap<String, Oid>,
    /// O servidor anuncia `atomic`: todas as referências do envio mudam juntas, ou nenhuma.
    pub(crate) atomic: bool,
}

/// Lê o anúncio de referências de `info/refs`.
pub(crate) fn parse_advertisement(body: &[u8]) -> Result<Advertisement> {
    let mut advertisement = Advertisement::default();
    let mut started = false;
    for line in read_pkt_lines(body)? {
        let Some(line) = line else {
            started = true;
            continue;
        };
        if line.starts_with(b"# service=") {
            continue;
        }
        if !started && line.starts_with(b"#") {
            continue;
        }
        let text = String::from_utf8_lossy(&line);
        let mut parts = text.split('\0');
        let text = parts.next().unwrap_or_default().trim_end();
        if let Some(capabilities) = parts.next() {
            advertisement.atomic |= capabilities.split_whitespace().any(|cap| cap == "atomic");
        }
        let Some((id, name)) = text.split_once(' ') else {
            return Err(protocol_error(format!("referência malformada: {text}")));
        };
        if name == "capabilities^{}" || id == ZERO {
            continue;
        }
        let oid = Oid::from_str(id).map_err(|_| protocol_error(format!("id inválido: {id}")))?;
        advertisement.refs.insert(name.to_owned(), oid);
    }
    Ok(advertisement)
}

/// Corpo do `POST git-receive-pack`: os comandos, o `flush` e o pacote. Com `atomic`, o
/// servidor aplica todas as referências ou nenhuma (a tag nunca entra sem a `main`).
pub(crate) fn receive_pack_request(updates: &[RefUpdate], pack: &[u8], atomic: bool) -> Vec<u8> {
    let mut body = Vec::new();
    for (i, update) in updates.iter().enumerate() {
        let old = update
            .old
            .map_or_else(|| ZERO.to_owned(), |oid| oid.to_string());
        let mut line = format!("{old} {} {}", update.new, update.name);
        if i == 0 {
            line.push_str(if atomic {
                "\0report-status atomic agent=warden"
            } else {
                "\0report-status agent=warden"
            });
        }
        line.push('\n');
        pkt_line(&mut body, line.as_bytes());
    }
    body.extend_from_slice(b"0000");
    body.extend_from_slice(pack);
    body
}

/// Resultado do envio, por referência: `Ok(())` ou o motivo da recusa.
pub(crate) fn parse_report(
    body: &[u8],
) -> Result<BTreeMap<String, std::result::Result<(), String>>> {
    let mut report = BTreeMap::new();
    let mut unpack_ok = false;
    for line in read_pkt_lines(body)?.into_iter().flatten() {
        let text = String::from_utf8_lossy(&line).trim_end().to_owned();
        if let Some(status) = text.strip_prefix("unpack ") {
            if status != "ok" {
                return Err(Error::PushRejected {
                    reason: format!("unpack {status}"),
                });
            }
            unpack_ok = true;
        } else if let Some(name) = text.strip_prefix("ok ") {
            report.insert(name.to_owned(), Ok(()));
        } else if let Some(rest) = text.strip_prefix("ng ") {
            let (name, reason) = rest.split_once(' ').unwrap_or((rest, ""));
            report.insert(name.to_owned(), Err(reason.to_owned()));
        }
    }
    if !unpack_ok {
        return Err(protocol_error("o servidor não confirmou o recebimento"));
    }
    Ok(report)
}

/// O servidor git do GitHub para um repositório.
pub(crate) struct GitRemote<'a> {
    client: &'a GithubClient,
    repo: &'a RepoRef,
    url: Url,
}

impl<'a> GitRemote<'a> {
    pub(crate) fn new(client: &'a GithubClient, repo: &'a RepoRef) -> Result<Self> {
        Ok(Self {
            url: client.endpoints().remote(repo)?,
            client,
            repo,
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url> {
        let base = format!("{}/", self.url.as_str().trim_end_matches('/'));
        Url::parse(&base)
            .and_then(|base| base.join(path))
            .map_err(|error| Error::Internal(format!("endereço git: {error}")))
    }

    fn auth(&self, request: warden_http::Request) -> Result<warden_http::Request> {
        request
            .secret_header(
                HeaderName::from_static("authorization"),
                &self.client.basic_auth(),
            )
            .map_err(|error| Error::GithubFailed {
                action: "montar o envio",
                source: Box::new(error),
            })
    }

    /// As referências do servidor (`refs/heads/main`, `refs/tags/v1.0.0`…) e as capacidades
    /// que importam.
    ///
    /// Erros: [`Error::GithubTokenRejected`], [`Error::GithubPermissionMissing`] (envio),
    /// [`Error::GithubRepoNotFound`], [`Error::GithubOffline`].
    pub(crate) async fn advertisement(&self, cancel: &CancellationToken) -> Result<Advertisement> {
        const ACTION: &str = "ler o repositório no GitHub";
        let url = self.endpoint("info/refs?service=git-receive-pack")?;
        let request = self.auth(self.client.http().get(url))?.cancel(cancel);
        let body = request
            .max_body(MAX_ADVERTISEMENT)
            .bytes()
            .await
            .map_err(|error| http_error(error, ACTION, GithubPermission::Push, Some(self.repo)))?;
        parse_advertisement(&body)
    }

    /// Envia o pacote e atualiza as referências, cada uma só se o valor antigo ainda for o
    /// do servidor (todas juntas quando o servidor aceita `atomic`).
    ///
    /// Erros: [`Error::RemoteDiverged`] se alguma referência mudou no servidor;
    /// [`Error::PushRejected`] para outra recusa; os de [`GitRemote::advertisement`].
    pub(crate) async fn push(
        &self,
        updates: &[RefUpdate],
        pack: &[u8],
        atomic: bool,
        cancel: &CancellationToken,
    ) -> Result<()> {
        const ACTION: &str = "enviar a versão";
        let url = self.endpoint("git-receive-pack")?;
        let body = receive_pack_request(updates, pack, atomic);
        let request = self
            .client
            .http()
            .post_bytes(url, "application/x-git-receive-pack-request", body)
            .header(
                HeaderName::from_static("accept"),
                HeaderValue::from_static("application/x-git-receive-pack-result"),
            )
            .total_timeout(Some(PUSH_TIMEOUT))
            .cancel(cancel);
        let response =
            self.auth(request)?.bytes().await.map_err(|error| {
                http_error(error, ACTION, GithubPermission::Push, Some(self.repo))
            })?;
        let report = parse_report(&response)?;
        let refused = updates
            .iter()
            .find_map(|update| match report.get(&update.name) {
                Some(Ok(())) => None,
                Some(Err(reason)) => Some(format!("{}: {reason}", update.name)),
                None => Some(format!("{}: sem resposta do servidor", update.name)),
            });
        let Some(reason) = refused else {
            return Ok(());
        };
        // O motivo da recusa é texto livre do servidor (o `git-receive-pack` diz só "failed
        // to update ref" ou "atomic transaction failed"): a divergência se confere relendo as
        // referências.
        let now = self.advertisement(cancel).await?.refs;
        if updates
            .iter()
            .any(|update| now.get(&update.name).copied() != update.old)
        {
            return Err(Error::RemoteDiverged {
                repo: self.repo.full_name(),
            });
        }
        Err(Error::PushRejected { reason })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linhas_do_protocolo_ida_e_volta() {
        let mut out = Vec::new();
        pkt_line(&mut out, b"# service=git-receive-pack\n");
        out.extend_from_slice(b"0000");
        pkt_line(&mut out, b"abc\n");
        assert!(out.starts_with(b"001f# service=git-receive-pack\n0000"));
        let lines = read_pkt_lines(&out).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], None);
        assert_eq!(lines[2].as_deref(), Some(&b"abc\n"[..]));
        assert!(read_pkt_lines(b"00zz").is_err());
        assert!(read_pkt_lines(b"0100abc").is_err());
    }

    #[test]
    fn anuncio_vazio_e_com_referencias() {
        let mut body = Vec::new();
        pkt_line(&mut body, b"# service=git-receive-pack\n");
        body.extend_from_slice(b"0000");
        pkt_line(
            &mut body,
            format!("{ZERO} capabilities^{{}}\0report-status ofs-delta\n").as_bytes(),
        );
        body.extend_from_slice(b"0000");
        let empty = parse_advertisement(&body).unwrap();
        assert!(empty.refs.is_empty());
        assert!(!empty.atomic);

        let id = "1111111111111111111111111111111111111111";
        let mut body = Vec::new();
        pkt_line(&mut body, b"# service=git-receive-pack\n");
        body.extend_from_slice(b"0000");
        pkt_line(
            &mut body,
            format!("{id} refs/heads/main\0report-status atomic ofs-delta\n").as_bytes(),
        );
        pkt_line(&mut body, format!("{id} refs/tags/v1.0.0\n").as_bytes());
        body.extend_from_slice(b"0000");
        let advertisement = parse_advertisement(&body).unwrap();
        assert!(advertisement.atomic);
        let refs = advertisement.refs;
        assert_eq!(refs.len(), 2);
        assert_eq!(refs["refs/heads/main"].to_string(), id);
    }

    #[test]
    fn pedido_com_valor_antigo_e_capacidades_so_na_primeira_linha() {
        let new = Oid::from_str("2222222222222222222222222222222222222222").unwrap();
        let old = Oid::from_str("1111111111111111111111111111111111111111").unwrap();
        let body = receive_pack_request(
            &[
                RefUpdate {
                    name: "refs/heads/main".into(),
                    old: Some(old),
                    new,
                },
                RefUpdate {
                    name: "refs/tags/v1.0.0".into(),
                    old: None,
                    new,
                },
            ],
            b"PACK",
            true,
        );
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains(&format!(
            "{old} {new} refs/heads/main\0report-status atomic agent=warden"
        )));
        assert!(text.contains(&format!("{ZERO} {new} refs/tags/v1.0.0\n")));
        assert_eq!(text.matches("report-status").count(), 1);
        assert!(text.ends_with("0000PACK"));
    }

    #[test]
    fn relatorio_do_servidor() {
        let mut body = Vec::new();
        pkt_line(&mut body, b"unpack ok\n");
        pkt_line(&mut body, b"ok refs/heads/main\n");
        pkt_line(&mut body, b"ng refs/tags/v1 stale info\n");
        body.extend_from_slice(b"0000");
        let report = parse_report(&body).unwrap();
        assert_eq!(report["refs/heads/main"], Ok(()));
        assert_eq!(report["refs/tags/v1"], Err("stale info".into()));

        let mut body = Vec::new();
        pkt_line(&mut body, b"unpack index-pack abnormal exit\n");
        body.extend_from_slice(b"0000");
        assert!(matches!(
            parse_report(&body),
            Err(Error::PushRejected { .. })
        ));
        assert!(parse_report(b"0000").is_err());
    }
}
