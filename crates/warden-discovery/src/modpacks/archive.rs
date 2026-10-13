//! Um arquivo de dentro do zip de um modpack, lido por partes ([`warden_http::RemoteFile`]).
//!
//! O crate `zip` lê o fim do diretório central, o diretório central e, depois, só a entrada
//! pedida; cada leitura que falta vira um pedido `Range`. Tudo fica em memória: nada do modpack
//! (nem o manifesto da CurseForge) vai para o disco. Servidor sem `Range`: o arquivo inteiro,
//! até 200 MB, com aviso ([`ArchiveEntry::whole_download`]).

use std::io::Read as _;

use warden_core::CancellationToken;
use warden_http::{RangeOptions, RemoteFile, Url};
use warden_project::search::SourceId;

use super::{ModpackError, ModpackFailure, Result, unavailable};
use crate::DiscoveryErrorCode;

/// Maior entrada aceita (um `modrinth.index.json` de mil mods tem ~700 KB).
const MAX_ENTRY: u64 = 16 * 1024 * 1024;

/// O que foi lido do zip.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// O conteúdo da entrada pedida.
    pub bytes: Vec<u8>,
    /// Os nomes de todas as entradas (diretório central).
    pub names: Vec<String>,
    /// O servidor não aceitou `Range`: o arquivo veio inteiro (para a memória).
    pub whole_download: bool,
    /// Pedidos `GET` feitos ao servidor do arquivo.
    pub requests: u64,
    /// Bytes recebidos.
    pub bytes_received: u64,
}

/// Lê `entry` do zip em `url`. Erros: fonte sem resposta (`SEARCH_SOURCE_UNAVAILABLE` de
/// `source`), arquivo grande demais sem `Range` (`MODPACK_TOO_LARGE`), zip inválido ou sem a
/// entrada (`MODPACK_UNREADABLE`).
pub async fn read_entry(
    http: &warden_http::HttpClient,
    source: SourceId,
    url: &str,
    entry: &str,
    options: RangeOptions,
    cancel: &CancellationToken,
) -> Result<ArchiveEntry> {
    let url = Url::parse(url)
        .map_err(|e| ModpackError::unreadable(format!("endereço do arquivo inválido: {e}")))?;
    let file = RemoteFile::open(http, url, options, Some(cancel))
        .await
        .map_err(|error| http_failure(source, error))?;
    let handle = tokio::runtime::Handle::current();
    let reader_file = file.clone();
    let entry_name = entry.to_owned();
    let read = tokio::task::spawn_blocking(move || {
        let mut reader = reader_file.reader(handle);
        let result = read_zip(&mut reader, &entry_name);
        (result, reader.take_error())
    })
    .await
    .map_err(|e| ModpackError::new(DiscoveryErrorCode::Internal, format!("leitura do zip: {e}")))?;
    let (bytes, names) = match read {
        (Ok(found), _) => found,
        (Err(_), Some(network)) => return Err(http_failure(source, network)),
        (Err(error), None) => return Err(ModpackError::unreadable(error).into()),
    };
    tracing::debug!(
        requests = file.requests(),
        bytes = file.bytes_received(),
        total = file.len(),
        inteiro = file.whole_download(),
        "entrada do modpack lida por partes"
    );
    Ok(ArchiveEntry {
        bytes,
        names,
        whole_download: file.whole_download(),
        requests: file.requests(),
        bytes_received: file.bytes_received(),
    })
}

fn read_zip(
    reader: &mut warden_http::RangeReader,
    entry: &str,
) -> std::result::Result<(Vec<u8>, Vec<String>), String> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("zip inválido: {e}"))?;
    let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
    let file = archive
        .by_name(entry)
        .map_err(|e| format!("{entry} não encontrado no modpack: {e}"))?;
    if file.size() > MAX_ENTRY {
        return Err(format!("{entry} tem {} bytes", file.size()));
    }
    let mut bytes = Vec::new();
    file.take(MAX_ENTRY)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{entry} ilegível: {e}"))?;
    Ok((bytes, names))
}

/// Erro de rede ao ler o arquivo do modpack.
fn http_failure(source: SourceId, error: warden_http::Error) -> ModpackFailure {
    match error {
        warden_http::Error::ResponseTooLarge { limit, .. } => ModpackError::new(
            DiscoveryErrorCode::ModpackTooLarge,
            "o servidor não aceita Range e o arquivo passa do limite",
        )
        .param("size", (limit / (1024 * 1024)).to_string())
        .into(),
        warden_http::Error::NotFound { .. } | warden_http::Error::InvalidResponse { .. } => {
            ModpackError::unreadable(error.to_string()).into()
        }
        other => unavailable(source, other),
    }
}
