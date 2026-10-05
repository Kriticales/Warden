//! Limites de segurança da leitura (QUALITY §9 item 6: entrada externa não é confiável).
//!
//! Nada é extraído para o disco e nenhum arquivo é descompactado além do limite: os tamanhos
//! declarados no zip não são confiáveis, então cada leitura para no limite contado nos bytes
//! realmente descompactados. Isso protege contra *zip bomb* (taxa de compressão absurda),
//! jars embutidos em cascata e arquivos enormes.

/// Limites aplicados a uma leitura de jar (o jar de cima e todos os embutidos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Tamanho máximo do arquivo do jar de cima, em bytes (o zip não é lido inteiro; só o
    /// diretório central e as entradas de interesse).
    pub max_file_bytes: u64,
    /// Número máximo de entradas de um zip.
    pub max_entries: usize,
    /// Tamanho máximo descompactado de um arquivo de metadados.
    pub max_descriptor_bytes: u64,
    /// Tamanho máximo descompactado de um jar embutido (precisa ir para a memória).
    pub max_nested_jar_bytes: u64,
    /// Soma máxima dos jars embutidos carregados numa leitura.
    pub max_total_nested_bytes: u64,
    /// Profundidade máxima de jars embutidos (o jar de cima é 0).
    pub max_depth: u32,
    /// Quantas classes têm o cabeçalho lido para achar a versão de classe.
    pub max_class_headers: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 1024 * 1024 * 1024,
            max_entries: 500_000,
            max_descriptor_bytes: 4 * 1024 * 1024,
            max_nested_jar_bytes: 64 * 1024 * 1024,
            max_total_nested_bytes: 512 * 1024 * 1024,
            max_depth: 8,
            max_class_headers: 512,
        }
    }
}
