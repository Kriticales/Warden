//! Memória da JVM do jogo pelo `hsperfdata` (ROADMAP L-11; ARCHITECTURE §7.7; ADR-0038; spike
//! S-R5-2): os contadores que a `HotSpot` publica por padrão, os mesmos que o `jstat` lê, sem JDK,
//! sem anexar nada ao jogo e sem mudar os argumentos dele.
//!
//! - [`parse`]: o formato do buffer (puro, sem `unsafe`);
//! - [`reading`]: heap usada, máximo, coletas e o coletor;
//! - [`locate`]: onde fica o `hsperfdata` do processo;
//! - `windows`: o *file mapping* nomeado (o `unsafe` fica todo lá).
//!
//! [`JvmHeap`] junta tudo: tenta abrir logo depois de o jogo abrir (o buffer fica legível
//! ~50–70 ms depois) e de novo a cada amostra até achar; só aceita o buffer com
//! `accessible = 1`; relê a lista de contadores quando o cabeçalho muda; e solta o mapeamento
//! assim que o processo sai ([`JvmHeap::sample`]).
//!
//! **Privacidade** (S-R5-2 §9): do `hsperfdata` só saem números ([`HeapReading`]). O buffer cru
//! (PATH, caminhos, `sun.rt.javaCommand` com os argumentos do jogo) nunca é gravado, registrado
//! em log nem enviado, nem à IA.

pub mod locate;
pub mod parse;
pub mod reading;
#[cfg(windows)]
mod windows;

use std::path::{Path, PathBuf};

pub use locate::{Location, candidates, game_temp_dirs};
pub use reading::{Collector, HeapReading, JvmInfo, Layout, read_buffer};

use crate::process::{MemorySample, ProcessTree};

/// De onde vêm os bytes.
#[derive(Debug)]
enum Source {
    /// O *file mapping* do Windows (o caminho principal lá).
    #[cfg(windows)]
    Mapping(windows::Mapping),
    /// O arquivo `<pid>` (o caminho do Linux e a alternativa no Windows).
    File(PathBuf),
}

impl Source {
    fn snapshot(&self) -> std::io::Result<Vec<u8>> {
        match self {
            #[cfg(windows)]
            Self::Mapping(mapping) => Ok(mapping.snapshot()),
            Self::File(path) => read_file(path),
        }
    }
}

fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    #[cfg(windows)]
    {
        windows::read_file(path)
    }
    #[cfg(not(windows))]
    {
        std::fs::read(path)
    }
}

/// A leitura da memória da JVM de um processo, do começo ao fim do teste.
#[derive(Debug)]
pub struct JvmHeap {
    pid: u32,
    temp_dirs: Vec<PathBuf>,
    source: Option<Source>,
    layout: Option<Layout>,
    /// Depois de o processo sair, nada mais é aberto.
    finished: bool,
}

impl JvmHeap {
    /// Começa a procurar o `hsperfdata` do processo `pid` nas pastas temporárias dadas
    /// ([`game_temp_dirs`]). Nada é aberto aqui: a primeira tentativa é a primeira
    /// [`JvmHeap::read`].
    #[must_use]
    pub fn new(pid: u32, temp_dirs: Vec<PathBuf>) -> Self {
        Self {
            pid,
            temp_dirs,
            source: None,
            layout: None,
            finished: false,
        }
    }

    /// Lê a memória agora. `None` enquanto não há leitura (o buffer ainda não existe ou não está
    /// pronto, a JVM não publica os contadores, ou o processo já saiu).
    pub fn read(&mut self) -> Option<HeapReading> {
        if self.finished {
            return None;
        }
        if self.source.is_none() {
            self.source = self.open();
        }
        // O arquivo sumiu (a JVM saiu): procura de novo na próxima amostra.
        let Ok(buf) = self.source.as_ref()?.snapshot() else {
            self.forget();
            return None;
        };
        // Buffer inválido: não é o de uma JVM viva; procura de novo na próxima.
        read_buffer(&buf, &mut self.layout).unwrap_or_else(|_| {
            self.forget();
            None
        })
    }

    /// Mede a árvore do processo e a memória da JVM juntas. Quando o processo principal sai
    /// (a árvore não tem amostra), solta o `hsperfdata` **na mesma amostra** e devolve `None`.
    pub fn sample(
        &mut self,
        tree: &mut ProcessTree,
    ) -> Option<(MemorySample, Option<HeapReading>)> {
        let Some(memory) = tree.sample() else {
            self.close();
            return None;
        };
        Some((memory, self.read()))
    }

    /// O que se sabe da JVM (versão e coletor), depois da primeira leitura.
    #[must_use]
    pub fn info(&self) -> Option<JvmInfo> {
        self.layout.as_ref().map(Layout::info)
    }

    /// Se o `hsperfdata` está aberto (no Windows, o mapeamento seguro pelo Warden).
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.source.is_some()
    }

    /// Solta o `hsperfdata` e não abre mais nada (o processo saiu).
    pub fn close(&mut self) {
        self.forget();
        self.finished = true;
    }

    /// Solta o que estava aberto (a próxima leitura procura de novo).
    fn forget(&mut self) {
        self.source = None;
        self.layout = None;
    }

    /// Abre o primeiro candidato que funcionar, do mais novo para o mais velho: no Windows o
    /// mapeamento (um arquivo velho, sem JVM viva, não tem mapeamento) e, se nenhum abrir, o
    /// arquivo do mais novo.
    fn open(&self) -> Option<Source> {
        let found = candidates(&self.temp_dirs, self.pid);
        #[cfg(windows)]
        for location in &found {
            if let Some(mapping) = windows::Mapping::open(&location.mapping_name) {
                return Some(Source::Mapping(mapping));
            }
        }
        found
            .into_iter()
            .next()
            .map(|location| Source::File(location.file))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn sem_hsperfdata_nao_ha_leitura_e_nada_fica_aberto() {
        let temp = tempfile::tempdir().unwrap();
        let mut heap = JvmHeap::new(4242, vec![temp.path().to_path_buf()]);
        assert_eq!(heap.read(), None);
        assert!(!heap.is_open());
    }

    #[test]
    fn le_pelo_arquivo_e_procura_de_novo_quando_ele_some() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("hsperfdata_teste");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("31337");
        let buf = parse::test_buffer(
            &[
                ("sun.gc.generation.0.space.0.used", 64 << 20),
                ("sun.gc.collector.0.invocations", 3),
            ],
            &[("sun.gc.policy.name", "GarbageFirst")],
        );
        // Ainda não pronto (`accessible = 0`): sem leitura, mas o arquivo fica conhecido.
        let mut early = buf.clone();
        early[7] = 0;
        std::fs::write(&file, &early).unwrap();
        let mut heap = JvmHeap::new(31337, vec![temp.path().to_path_buf()]);
        assert_eq!(heap.read(), None);
        assert!(heap.is_open());

        std::fs::write(&file, &buf).unwrap();
        let reading = heap.read().unwrap();
        assert_eq!(reading.used_bytes, 64 << 20);
        assert_eq!(reading.gc_count, 3);
        assert_eq!(heap.info().unwrap().collector, Collector::G1);

        std::fs::remove_file(&file).unwrap();
        assert_eq!(heap.read(), None);
        assert!(!heap.is_open());

        // Um buffer corrompido não é aceito.
        std::fs::write(&file, b"lixo que nao e hsperfdata, nem de longe, nada").unwrap();
        assert_eq!(heap.read(), None);
        assert!(!heap.is_open());
    }

    #[test]
    fn fechar_impede_abrir_de_novo() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("hsperfdata_teste");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("9"),
            parse::test_buffer(&[("sun.gc.generation.0.space.0.used", 1)], &[]),
        )
        .unwrap();
        let mut heap = JvmHeap::new(9, vec![temp.path().to_path_buf()]);
        assert!(heap.read().is_some());
        heap.close();
        assert!(!heap.is_open());
        assert_eq!(heap.read(), None);
        assert!(!heap.is_open());
    }
}
