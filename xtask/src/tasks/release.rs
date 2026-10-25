//! Monta a tag, o título e as notas da versão final no GitHub Releases (`--dry-run` só mostra).
//!
//! `cargo xtask release [--instalador <arquivo.exe>] [--dry-run]` (REL-01). Quem publica é o
//! workflow `.github/workflows/release.yml`, disparado à mão pelo orquestrador; este subcomando
//! só prepara o texto, com o mesmo código e as mesmas proteções do `pre-versao`:
//!
//! - **versão** do app lida do `tauri.conf.json` (ou, sem ela, do `Cargo.toml` da `warden-app`);
//! - **tag** `v<versão>`;
//! - **título** `Warden <versão>`;
//! - **notas**: o texto de `docs/versoes/<versão>.md` (sem o título), sem trechos `[preencher`,
//!   mais o bloco fixo da pré-versão (como instalar, o aviso do `SmartScreen`, como relatar
//!   problemas) e o SHA-256 do instalador.
//!
//! Com `--alvo <pasta>`, a versão sai de outro checkout (o do commit da release), e as notas
//! continuam vindo deste.
//!
//! Grava `tag.txt`, `titulo.txt` e `notas.md` em `--saida` (padrão `<target>/versao-final`, fora
//! da `target/release` do Cargo). Sem `--dry-run`, exige o instalador e falha se a tag já existir
//! no `origin`; com `--dry-run`, não consulta o GitHub, aceita a falta do instalador e imprime
//! tudo.

use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};

use super::pre_versao::{
    Instalador, Publicacao, entregar, instalador_anexado, ler_notas, notas_com_bloco_fixo,
    resolver_commit, separar_titulo, versao_do_app,
};
use crate::util::workspace_root;

/// Argumentos de `cargo xtask release`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Instalador `.exe` que vai anexado (obrigatório sem `--dry-run`).
    #[arg(long)]
    pub instalador: Option<PathBuf>,
    /// Commit da release (padrão: o `HEAD`).
    #[arg(long)]
    pub commit: Option<String>,
    /// Checkout do commit da release, de onde sai a versão (padrão: este repositório). As notas
    /// vêm sempre deste repositório.
    #[arg(long)]
    pub alvo: Option<PathBuf>,
    /// Pasta dos arquivos gerados (padrão: `<target>/versao-final`).
    #[arg(long)]
    pub saida: Option<PathBuf>,
    /// Só monta e mostra: não exige o instalador nem consulta as tags do GitHub.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: &Args) -> Result<()> {
    let root = workspace_root();
    let versao = versao_do_app(args.alvo.as_deref().unwrap_or(&root))?;
    let texto = ler_versao(&root, &versao)?;
    let commit = resolver_commit(args.commit.as_deref())?;
    let instalador = instalador_anexado(args.instalador.as_deref(), &versao, args.dry_run)?;
    let release = montar(&versao, &texto, &commit, &instalador)?;
    entregar(&root, &release, args.saida.as_deref(), "versao-final", args.dry_run)
}

/// Texto de `docs/versoes/<versão>.md`, conferido.
fn ler_versao(root: &Path, versao: &str) -> Result<String> {
    ler_notas(
        root,
        &format!("docs/versoes/{versao}.md"),
        "escreva as notas da versão (veja o docs/versoes/1.0.0.md) antes de publicar",
    )
}

fn montar(versao: &str, texto: &str, commit: &str, instalador: &Instalador) -> Result<Publicacao> {
    let (_, corpo) = separar_titulo(texto)?;
    ensure!(
        !corpo.is_empty(),
        "as notas da versão {versao} só têm o título"
    );
    let notas = notas_com_bloco_fixo(
        corpo,
        versao,
        commit,
        instalador,
        "Esta é a **versão final**; as pré-versões dos marcos continuam na lista de Releases, só para consulta.",
    );
    Ok(Publicacao {
        tag: format!("v{versao}"),
        titulo: format!("Warden {versao}"),
        notas,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const COMMIT: &str = "2a2b09e0123456789abcdef0123456789abcdef0";
    const NOTAS: &str = "# Warden 1.0\n\n## O que o Warden faz\n\n- Criar packs.\n";

    fn instalador(sha: Option<&str>) -> Instalador {
        Instalador {
            nome: "Warden_1.0.0_x64-setup.exe".to_owned(),
            sha256: sha.map(str::to_owned),
        }
    }

    #[test]
    fn monta_tag_titulo_e_notas() {
        let sha = "cd".repeat(32);
        let release = montar("1.0.0", NOTAS, COMMIT, &instalador(Some(&sha))).unwrap();
        assert_eq!(release.tag, "v1.0.0");
        assert_eq!(release.titulo, "Warden 1.0.0");
        assert!(release.notas.starts_with("## O que o Warden faz\n\n- Criar packs.\n"));
        assert!(!release.notas.contains("# Warden 1.0\n"), "o título não se repete nas notas");
        assert!(!release.notas.contains("pré-versão** de fim de marco"));
        for trecho in [
            "## Como instalar",
            "**Executar assim mesmo**",
            "**Relatar problema**",
            &format!("| `Warden_1.0.0_x64-setup.exe` | `{sha}` |"),
            "**versão final**",
            "commit [`2a2b09e`](https://github.com/Kriticales/Warden/commit/2a2b09e0123456789abcdef0123456789abcdef0)",
        ] {
            assert!(
                release.notas.contains(trecho),
                "faltou {trecho:?} nas notas:\n{}",
                release.notas
            );
        }
    }

    #[test]
    fn titulo_obrigatorio_e_corpo_nao_vazio() {
        assert!(montar("1.0.0", "Sem título\n\ntexto", COMMIT, &instalador(None)).is_err());
        assert!(montar("1.0.0", "# Warden 1.0\n\n", COMMIT, &instalador(None)).is_err());
    }

    #[test]
    fn notas_ausentes_ou_com_pendencia_sao_recusadas() {
        let dir = tempfile::tempdir().unwrap();
        let erro = ler_versao(dir.path(), "2.0.0").unwrap_err().to_string();
        assert!(erro.contains("docs/versoes/2.0.0.md"), "{erro}");
        fs::create_dir_all(dir.path().join("docs/versoes")).unwrap();
        fs::write(
            dir.path().join("docs/versoes/2.0.0.md"),
            "# Warden 2.0\n\n[preencher: o que mudou]\n",
        )
        .unwrap();
        assert!(ler_versao(dir.path(), "2.0.0").is_err());
    }

    #[test]
    fn notas_da_1_0_0_estao_prontas() {
        let texto = ler_versao(&workspace_root(), "1.0.0").unwrap();
        let release = montar("1.0.0", &texto, COMMIT, &instalador(None)).unwrap();
        assert_eq!(release.tag, "v1.0.0");
        assert!(release.notas.contains("`(calculado na publicação)`"));
    }

    #[test]
    fn notas_da_versao_atual_do_app_estao_prontas() {
        let root = workspace_root();
        let versao = versao_do_app(&root).unwrap();
        let texto = ler_versao(&root, &versao).unwrap();
        let release = montar(&versao, &texto, COMMIT, &instalador(None)).unwrap();
        assert_eq!(release.tag, format!("v{versao}"));
        assert_eq!(release.titulo, format!("Warden {versao}"));
    }

    #[test]
    fn notas_da_1_0_1_estao_prontas() {
        let texto = ler_versao(&workspace_root(), "1.0.1").unwrap();
        let release = montar("1.0.1", &texto, COMMIT, &instalador(None)).unwrap();
        assert_eq!(release.tag, "v1.0.1");
        assert_eq!(release.titulo, "Warden 1.0.1");
        assert!(release.notas.starts_with("Esta é uma versão de conserto"));
    }

    #[test]
    fn notas_da_1_1_0_estao_prontas() {
        let texto = ler_versao(&workspace_root(), "1.1.0").unwrap();
        let release = montar("1.1.0", &texto, COMMIT, &instalador(None)).unwrap();
        assert_eq!(release.tag, "v1.1.0");
        assert_eq!(release.titulo, "Warden 1.1.0");
        assert!(release.notas.starts_with("Esta versão traz os consertos"));
        for trecho in ["## Vindo da 1.0.1", "Cofre do Windows", "**1.1.0**"] {
            assert!(release.notas.contains(trecho), "faltou {trecho:?}");
        }
    }
}
