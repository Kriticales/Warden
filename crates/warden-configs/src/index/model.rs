//! O que a busca devolve (contrato com a interface, ARCHITECTURE §10.1).

use serde::Serialize;

/// Um arquivo que a origem tem, como a listagem do editor o descreve (`list_files` da
/// `warden-project`). A busca só lê o que vem nesta lista.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    /// Caminho relativo à origem, com `/`.
    pub path: String,
    /// Tamanho em bytes.
    pub size: u64,
    /// Última alteração, em milissegundos Unix.
    pub modified_ms: Option<f64>,
    /// O arquivo parece binário (não entra na busca).
    pub binary: bool,
}

/// Onde um resultado casou. A ordem é a da pontuação: chave > rótulo > comentário > valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HitField {
    /// Valor da chave.
    Value,
    /// Linha de um arquivo sem leitor estruturado (YAML, SNBT, scripts, texto).
    Line,
    /// Comentário da chave.
    Comment,
    /// Rótulo traduzido do mod (P1; C-04). Ainda não preenchido.
    Label,
    /// Nome da chave (com o caminho das seções).
    Key,
}

/// Um pedaço de texto, destacado ou não.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MarkedPart {
    /// O texto.
    pub text: String,
    /// Faz parte do que casou com a busca.
    pub marked: bool,
}

/// Texto com os trechos que casaram destacados. Um texto longo vem cortado em volta do primeiro
/// trecho, com "…" no lugar do que ficou de fora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MarkedText {
    /// Os pedaços, na ordem do texto.
    pub parts: Vec<MarkedPart>,
}

impl MarkedText {
    /// O texto inteiro, sem as marcas.
    #[must_use]
    pub fn plain(&self) -> String {
        self.parts.iter().map(|part| part.text.as_str()).collect()
    }

    /// Só os trechos destacados.
    #[must_use]
    pub fn marked(&self) -> Vec<&str> {
        self.parts
            .iter()
            .filter(|part| part.marked)
            .map(|part| part.text.as_str())
            .collect()
    }
}

/// Um resultado: uma chave (ou uma linha) de um arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSearchHit {
    /// Linha da chave no arquivo (começa em 1). Clicar abre o arquivo nela.
    pub line: u32,
    /// Onde a pontuação foi maior.
    pub field: HitField,
    /// Nome da chave com o caminho das seções (`geral.spawnRate`); `None` numa linha de texto.
    pub key: Option<MarkedText>,
    /// Rótulo traduzido do mod (P1; chega com a C-04).
    pub label: Option<MarkedText>,
    /// Valor (`None` em seções e linhas de texto).
    pub value: Option<MarkedText>,
    /// Trecho do comentário (só quando o comentário casou).
    pub comment: Option<MarkedText>,
    /// A linha inteira, nos arquivos sem leitor estruturado.
    pub text: Option<MarkedText>,
    /// O valor é diferente do padrão que o arquivo informa (C-06).
    pub changed: bool,
}

/// Os resultados de um arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSearchGroup {
    /// Caminho relativo à origem.
    pub path: String,
    /// Mod dono do arquivo, deduzido do caminho (`config/create-common.toml` → `create`).
    pub mod_id: Option<String>,
    /// Resultados, na ordem das linhas (cortados em [`SearchQuery::max_hits_per_file`]).
    pub hits: Vec<ConfigSearchHit>,
    /// Quantos resultados o arquivo tem, contando os cortados.
    pub total: u32,
}

/// Um mod com resultados, para o filtro "Só deste mod".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSearchMod {
    /// Id deduzido do caminho dos arquivos.
    pub id: String,
    /// Quantos resultados ele tem (sem o filtro de mod).
    pub hits: u32,
}

/// Resultado de uma busca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSearchResults {
    /// Arquivos com resultado, do mais relevante para o menos.
    pub groups: Vec<ConfigSearchGroup>,
    /// Resultados ao todo (com o filtro de mod, contando os cortados).
    pub total_hits: u32,
    /// Arquivos com resultado (com o filtro de mod).
    pub total_files: u32,
    /// Algum resultado ficou de fora pelos limites.
    pub truncated: bool,
    /// Mods com resultado, por id, para o filtro (sem o filtro de mod).
    pub mods: Vec<ConfigSearchMod>,
}

/// Situação do índice de uma origem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    /// A pasta da origem existe (a instância só existe depois do primeiro teste).
    pub available: bool,
    /// A primeira leitura de todos os arquivos terminou.
    pub ready: bool,
    /// Arquivos já lidos.
    pub indexed_files: u32,
    /// Arquivos da origem (0 enquanto a lista não foi lida).
    pub total_files: u32,
    /// Chaves e linhas no índice.
    pub entries: u32,
}

/// O que se procura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// Texto digitado. Palavras separadas por espaço: todas precisam aparecer na entrada.
    pub text: String,
    /// "Só deste mod": só os arquivos com este id.
    pub mod_id: Option<String>,
    /// "Só o que mudou do padrão" (C-06): só as chaves diferentes do padrão que o arquivo
    /// informa. Com o texto vazio, lista todas elas.
    pub only_changed: bool,
    /// Máximo de resultados devolvidos ao todo.
    pub max_hits: usize,
    /// Máximo de resultados devolvidos por arquivo.
    pub max_hits_per_file: usize,
}

impl SearchQuery {
    /// Busca com os limites padrão (500 resultados, 50 por arquivo).
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            mod_id: None,
            only_changed: false,
            max_hits: 500,
            max_hits_per_file: 50,
        }
    }

    /// Com o filtro "Só deste mod".
    #[must_use]
    pub fn with_mod(mut self, mod_id: Option<String>) -> Self {
        self.mod_id = mod_id;
        self
    }

    /// Com o filtro "Só o que mudou do padrão".
    #[must_use]
    pub fn only_changed(mut self, only_changed: bool) -> Self {
        self.only_changed = only_changed;
        self
    }
}
