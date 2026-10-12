//! Prints do relato (UI-03): o rascunho guarda as imagens no Rust enquanto o diálogo está
//! aberto, e a interface só conhece um token, o nome, o tamanho e a miniatura.
//!
//! - Só PNG e JPEG, reconhecidos pelo conteúdo (os primeiros bytes), não pela extensão.
//! - Limites: [`MAX_IMAGE_BYTES`] por imagem, [`MAX_IMAGES`] por relato e
//!   [`MAX_TOTAL_BYTES`] somados. 10 MB cobre um print PNG de uma tela 4K; 5 prints bastam
//!   para mostrar um problema; 25 MB somados mantêm o envio (base64, +33%) curto numa conexão
//!   lenta e a branch dos anexos pequena.
//! - O identificador de cada imagem é o começo do SHA-256 do conteúdo: o mesmo print colado
//!   duas vezes entra uma vez, e um envio repetido depois de uma falha cai no mesmo arquivo.
//! - Abrir ou fechar o diálogo esvazia o rascunho; enquanto ele está aberto, arquivos soltos
//!   sobre a janela vão para os prints (e não para "Adicionar arquivo local").

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use base64::Engine as _;
use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::error::{AppError, AppErrorCode};

/// Maior imagem aceita.
pub(crate) const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

/// Mais prints num relato.
pub(crate) const MAX_IMAGES: usize = 5;

/// Maior soma dos prints de um relato.
pub(crate) const MAX_TOTAL_BYTES: usize = 25 * 1024 * 1024;

/// Maior nome mostrado (o resto é cortado).
const MAX_NAME_CHARS: usize = 60;

/// Tipos aceitos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageKind {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
}

impl ImageKind {
    /// Pelo conteúdo.
    pub(crate) fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else {
            None
        }
    }

    /// Extensão do arquivo enviado.
    pub(crate) const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }

    const fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

/// Um print aceito, como a interface o vê.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportImage {
    /// Identificador no rascunho.
    pub(crate) token: String,
    /// Nome do arquivo (limpo), para a lista e o texto alternativo.
    pub(crate) name: String,
    /// Tamanho em bytes.
    pub(crate) size: u32,
    /// A imagem como `data:` (miniatura na tela).
    pub(crate) preview: String,
}

/// Uma imagem recusada.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefusedImage {
    /// Nome do arquivo.
    pub(crate) name: String,
    /// Por quê.
    pub(crate) error: AppError,
}

/// Resultado de escolher ou soltar várias imagens.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportImagesAdded {
    /// As aceitas.
    pub(crate) added: Vec<ReportImage>,
    /// As recusadas, com o motivo.
    pub(crate) refused: Vec<RefusedImage>,
}

/// Uma imagem guardada.
#[derive(Debug, Clone)]
pub(crate) struct StoredImage {
    /// Identificador (começo do SHA-256, em hexadecimal).
    pub(crate) id: String,
    /// Nome limpo.
    pub(crate) name: String,
    /// Tipo.
    pub(crate) kind: ImageKind,
    /// Conteúdo.
    pub(crate) bytes: Vec<u8>,
}

impl StoredImage {
    /// Nome do arquivo no repositório: `<id>.<ext>`.
    pub(crate) fn file_name(&self) -> String {
        format!("{}.{}", self.id, self.kind.extension())
    }
}

#[derive(Debug, Default)]
struct Draft {
    open: bool,
    images: Vec<StoredImage>,
}

/// O rascunho de prints do diálogo aberto.
#[derive(Debug, Default)]
pub(crate) struct ReportDraft {
    draft: Mutex<Draft>,
}

impl ReportDraft {
    fn lock(&self) -> std::sync::MutexGuard<'_, Draft> {
        self.draft.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Abre ou fecha o rascunho; nos dois casos, sem prints.
    pub(crate) fn reset(&self, open: bool) {
        let mut draft = self.lock();
        draft.open = open;
        draft.images.clear();
    }

    /// Se o diálogo está aberto.
    pub(crate) fn is_open(&self) -> bool {
        self.lock().open
    }

    /// Acrescenta uma imagem (o mesmo conteúdo de novo devolve a que já está).
    pub(crate) fn add(&self, name: &str, bytes: Vec<u8>) -> Result<ReportImage, AppError> {
        let name = clean_name(name);
        let Some(kind) = ImageKind::sniff(&bytes) else {
            return Err(AppError::new(AppErrorCode::ReportImageInvalid).with_param("name", name));
        };
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(too_large(&name));
        }
        let id = hex::encode(&Sha256::digest(&bytes)[..10]);
        let mut draft = self.lock();
        if let Some(existing) = draft.images.iter().find(|image| image.id == id) {
            return Ok(view(existing));
        }
        let total: usize = draft.images.iter().map(|image| image.bytes.len()).sum();
        if draft.images.len() >= MAX_IMAGES || total + bytes.len() > MAX_TOTAL_BYTES {
            return Err(AppError::new(AppErrorCode::ReportImagesLimit)
                .with_param("max", MAX_IMAGES.to_string())
                .with_param("maxTotalMb", (MAX_TOTAL_BYTES / 1024 / 1024).to_string())
                .with_param("name", name));
        }
        let image = StoredImage {
            id,
            name,
            kind,
            bytes,
        };
        let shown = view(&image);
        tracing::info!(bytes = image.bytes.len(), "print acrescentado ao relato");
        draft.images.push(image);
        Ok(shown)
    }

    /// Acrescenta imagens do disco (escolhidas ou soltas), cada uma com o seu resultado.
    pub(crate) fn add_files(&self, paths: &[PathBuf]) -> ReportImagesAdded {
        let mut result = ReportImagesAdded {
            added: Vec::new(),
            refused: Vec::new(),
        };
        for path in paths {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            match read_image(path, &name).and_then(|bytes| self.add(&name, bytes)) {
                Ok(image) => {
                    if !result.added.iter().any(|known| known.token == image.token) {
                        result.added.push(image);
                    }
                }
                Err(error) => result.refused.push(RefusedImage {
                    name: clean_name(&name),
                    error,
                }),
            }
        }
        result
    }

    /// Tira uma imagem.
    pub(crate) fn remove(&self, token: &str) {
        self.lock().images.retain(|image| image.id != token);
    }

    /// As imagens pedidas, na ordem pedida. Token desconhecido é erro (o rascunho mudou).
    pub(crate) fn images(&self, tokens: &[String]) -> Result<Vec<StoredImage>, AppError> {
        let draft = self.lock();
        tokens
            .iter()
            .map(|token| {
                draft
                    .images
                    .iter()
                    .find(|image| &image.id == token)
                    .cloned()
                    .ok_or_else(|| AppError::internal("relato: print fora do rascunho"))
            })
            .collect()
    }

    /// Os nomes das imagens pedidas (as desconhecidas ficam de fora).
    pub(crate) fn names(&self, tokens: &[String]) -> Vec<String> {
        let draft = self.lock();
        tokens
            .iter()
            .filter_map(|token| draft.images.iter().find(|image| &image.id == token))
            .map(|image| image.name.clone())
            .collect()
    }
}

fn too_large(name: &str) -> AppError {
    AppError::new(AppErrorCode::ReportImageTooLarge)
        .with_param("name", name)
        .with_param("maxMb", (MAX_IMAGE_BYTES / 1024 / 1024).to_string())
}

/// Lê o arquivo, recusando o grande demais antes de ler.
fn read_image(path: &Path, name: &str) -> Result<Vec<u8>, AppError> {
    let size = std::fs::metadata(path)
        .map_err(|error| AppError::from(warden_core::CoreError::io("ler", path, error)))?
        .len();
    if size > MAX_IMAGE_BYTES as u64 {
        return Err(too_large(&clean_name(name)));
    }
    std::fs::read(path).map_err(|error| warden_core::CoreError::io("ler", path, error).into())
}

fn view(image: &StoredImage) -> ReportImage {
    ReportImage {
        token: image.id.clone(),
        name: image.name.clone(),
        size: u32::try_from(image.bytes.len()).unwrap_or(u32::MAX),
        preview: format!(
            "data:{};base64,{}",
            image.kind.mime(),
            base64::engine::general_purpose::STANDARD.encode(&image.bytes)
        ),
    }
}

/// Nome mostrável e seguro no Markdown: só o nome do arquivo, sem colchetes, parênteses
/// nem quebras, até [`MAX_NAME_CHARS`].
pub(crate) fn clean_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
        .take(MAX_NAME_CHARS)
        .collect();
    let cleaned = cleaned.trim().to_owned();
    if cleaned.is_empty() {
        "print".to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn png(seed: u8, size: usize) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.resize(size.max(9), seed);
        bytes
    }

    fn code(error: &AppError) -> String {
        error.code.to_string()
    }

    /// CA-UI03-07: tipos pelo conteúdo, limites por imagem, por relato e somados.
    #[test]
    fn ca_ui03_07_tipos_e_limites() {
        let draft = ReportDraft::default();
        draft.reset(true);
        let first = draft.add("tela.png", png(1, 100)).unwrap();
        assert_eq!(first.name, "tela.png");
        assert_eq!(first.size, 100);
        assert!(first.preview.starts_with("data:image/png;base64,iVBORw0KGgo"));
        let jpeg = draft.add("C:\\Users\\Ana\\foto.jpeg", vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2]).unwrap();
        assert_eq!(jpeg.name, "foto.jpeg");
        assert!(jpeg.preview.starts_with("data:image/jpeg;base64,"));

        // Extensão de imagem com outro conteúdo: recusado.
        let error = draft.add("falso.png", b"GIF89a....".to_vec()).unwrap_err();
        assert_eq!(code(&error), "app.REPORT_IMAGE_INVALID");
        assert_eq!(error.params["name"], "falso.png");
        // Grande demais.
        let error = draft.add("enorme.png", png(2, MAX_IMAGE_BYTES + 1)).unwrap_err();
        assert_eq!(code(&error), "app.REPORT_IMAGE_TOO_LARGE");
        assert_eq!(error.params["maxMb"], "10");
        // O mesmo conteúdo de novo não conta duas vezes.
        assert_eq!(draft.add("de-novo.png", png(1, 100)).unwrap(), first);
        for seed in 3..6 {
            draft.add("mais.png", png(seed, 100)).unwrap();
        }
        // O sexto é recusado, sem perder os cinco.
        let error = draft.add("sexto.png", png(9, 100)).unwrap_err();
        assert_eq!(code(&error), "app.REPORT_IMAGES_LIMIT");
        assert_eq!(error.params["max"], "5");
        let tokens: Vec<String> = draft.names(std::slice::from_ref(&first.token));
        assert_eq!(tokens, ["tela.png"]);

        // Somados: três de 9 MB passam de 25 MB.
        draft.reset(true);
        draft.add("a.png", png(1, 9 * 1024 * 1024)).unwrap();
        draft.add("b.png", png(2, 9 * 1024 * 1024)).unwrap();
        let error = draft.add("c.png", png(3, 9 * 1024 * 1024)).unwrap_err();
        assert_eq!(code(&error), "app.REPORT_IMAGES_LIMIT");
        assert_eq!(error.params["maxTotalMb"], "25");
    }

    #[test]
    fn abrir_fechar_e_remover_esvaziam() {
        let draft = ReportDraft::default();
        assert!(!draft.is_open());
        draft.reset(true);
        assert!(draft.is_open());
        let image = draft.add("a.png", png(1, 50)).unwrap();
        let stored = draft.images(std::slice::from_ref(&image.token)).unwrap();
        assert_eq!(stored[0].file_name(), format!("{}.png", image.token));
        assert_eq!(image.token.len(), 20);
        draft.remove(&image.token);
        assert!(draft.images(std::slice::from_ref(&image.token)).is_err());
        draft.add("a.png", png(1, 50)).unwrap();
        draft.reset(false);
        assert!(!draft.is_open());
        assert!(draft.images(&[image.token]).is_err());
    }

    #[test]
    fn arquivos_do_disco_aceitos_e_recusados() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("print 1.png");
        std::fs::write(&good, png(1, 64)).unwrap();
        let text = dir.path().join("notas.png");
        std::fs::write(&text, "não é imagem").unwrap();
        let big = dir.path().join("grande.jpg");
        let mut bytes = vec![0xFF, 0xD8, 0xFF];
        bytes.resize(MAX_IMAGE_BYTES + 10, 0);
        std::fs::write(&big, bytes).unwrap();
        let draft = ReportDraft::default();
        draft.reset(true);
        let result = draft.add_files(&[good.clone(), text, big, dir.path().join("sumiu.png"), good]);
        assert_eq!(result.added.len(), 1);
        assert_eq!(result.added[0].name, "print 1.png");
        let refused: Vec<_> = result
            .refused
            .iter()
            .map(|refused| (refused.name.as_str(), code(&refused.error)))
            .collect();
        assert_eq!(
            refused,
            [
                ("notas.png", "app.REPORT_IMAGE_INVALID".to_owned()),
                ("grande.jpg", "app.REPORT_IMAGE_TOO_LARGE".to_owned()),
                ("sumiu.png", "core.IO".to_owned()),
            ]
        );
    }

    #[test]
    fn nomes_limpos_para_o_markdown() {
        assert_eq!(clean_name("/home/ana/tela [1](x).png"), "tela 1x.png");
        assert_eq!(clean_name("C:\\Users\\Ana\\Área de trabalho\\ç.png"), "ç.png");
        assert_eq!(clean_name("\n]["), "print");
        assert_eq!(clean_name(&"a".repeat(100)).chars().count(), 60);
    }
}
