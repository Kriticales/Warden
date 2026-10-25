//! Monta a tag, o título e as notas da pré-versão de um marco no GitHub Releases (`--dry-run` só mostra).
//!
//! `cargo xtask pre-versao --marco <N> [--instalador <arquivo.exe>] [--dry-run]` (INFRA-02).
//! Quem publica é o workflow `.github/workflows/pre-versao.yml`, disparado à mão pelo
//! orquestrador ao fechar cada marco; este subcomando só prepara o texto, para o mesmo código
//! rodar na CI e no teste a seco local:
//!
//! - **versão** do app lida do `tauri.conf.json` (`version`) ou, sem ela, do `Cargo.toml` da
//!   `warden-app` (que herda a do workspace), nunca escrita à mão;
//! - **tag** `v<versão>-marco.<N>`;
//! - **título** `Warden <versão> · <título de docs/pre-versoes/marco-<N>.md> (pré-versão)`;
//! - **notas**: o texto de `docs/pre-versoes/marco-<N>.md` (sem o título) mais um bloco fixo
//!   (como instalar, o aviso do `SmartScreen`, como relatar problemas) e o SHA-256 do instalador.
//!
//! Com `--alvo <pasta>`, a versão sai de outro checkout (o do commit da pré-versão), e as notas
//! continuam vindo deste.
//!
//! Grava `tag.txt`, `titulo.txt` e `notas.md` em `--saida` (padrão `<target>/pre-versao`).
//! Sem `--dry-run`, exige o instalador e falha se a tag já existir no `origin`; com
//! `--dry-run`, não consulta o GitHub, aceita a falta do instalador e imprime tudo.

use std::fmt::Write as _;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail, ensure};
use sha2::{Digest as _, Sha256};

use crate::util::{Cmd, target_dir, workspace_root};

/// Marcos que podem virar pré-versão (ROADMAP §13: M1 a M5 da v1 e M6, o Warden 1.2).
const MARCOS: std::ops::RangeInclusive<u8> = 1..=6;

/// Marca dos trechos do modelo ainda não preenchidos (`docs/pre-versoes/MODELO.md`).
const MARCA_PENDENTE: &str = "[preencher";

/// Argumentos de `cargo xtask pre-versao`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Número do marco (1 a 6).
    #[arg(long)]
    pub marco: u8,
    /// Instalador `.exe` que vai anexado (obrigatório sem `--dry-run`).
    #[arg(long)]
    pub instalador: Option<PathBuf>,
    /// Commit da pré-versão (padrão: o `HEAD`).
    #[arg(long)]
    pub commit: Option<String>,
    /// Checkout do commit da pré-versão, de onde sai a versão (padrão: este repositório). As
    /// notas vêm sempre deste repositório, para um marco antigo ganhar notas escritas depois.
    #[arg(long)]
    pub alvo: Option<PathBuf>,
    /// Pasta dos arquivos gerados (padrão: `<target>/pre-versao`).
    #[arg(long)]
    pub saida: Option<PathBuf>,
    /// Só monta e mostra: não exige o instalador nem consulta as tags do GitHub.
    #[arg(long)]
    pub dry_run: bool,
}

/// Instalador anexado à Release.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Instalador {
    pub(crate) nome: String,
    /// SHA-256 em hexadecimal minúsculo; `None` no teste a seco sem arquivo.
    pub(crate) sha256: Option<String>,
}

/// O que a publicação usa (pré-versão ou versão final, `release.rs`).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Publicacao {
    pub(crate) tag: String,
    pub(crate) titulo: String,
    pub(crate) notas: String,
}

pub fn run(args: &Args) -> Result<()> {
    let root = workspace_root();
    let versao = versao_do_app(args.alvo.as_deref().unwrap_or(&root))?;
    let texto_marco = ler_marco(&root, args.marco)?;
    let commit = resolver_commit(args.commit.as_deref())?;
    let instalador = instalador_anexado(args.instalador.as_deref(), &versao, args.dry_run)?;
    let pre = montar(&versao, args.marco, &texto_marco, &commit, &instalador)?;
    entregar(&root, &pre, args.saida.as_deref(), "pre-versao", args.dry_run)
}

/// SHA completo do `--commit` ou, sem ele, do `HEAD`.
pub(crate) fn resolver_commit(commit: Option<&str>) -> Result<String> {
    let commit = match commit {
        Some(commit) => commit.to_owned(),
        None => Cmd::new("git")
            .args(["rev-parse", "HEAD"])
            .read()?
            .trim()
            .to_owned(),
    };
    ensure!(
        commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()),
        "--commit precisa ser o SHA completo (40 caracteres hexadecimais), não `{commit}`"
    );
    Ok(commit)
}

/// O `--instalador` com o SHA-256; no teste a seco, sem ele, o nome que o Tauri daria.
pub(crate) fn instalador_anexado(
    caminho: Option<&Path>,
    versao: &str,
    dry_run: bool,
) -> Result<Instalador> {
    Ok(match caminho {
        Some(caminho) => Instalador {
            nome: nome_do_arquivo(caminho)?,
            sha256: Some(sha256_do_arquivo(caminho)?),
        },
        None if dry_run => Instalador {
            nome: format!("Warden_{versao}_x64-setup.exe"),
            sha256: None,
        },
        None => bail!("falta o --instalador (o .exe do Windows); só o --dry-run dispensa"),
    })
}

/// Confere a tag (fora do teste a seco), grava `tag.txt`, `titulo.txt` e `notas.md` em `saida`
/// (padrão `<target>/<pasta_padrao>`) e mostra o resultado.
pub(crate) fn entregar(
    root: &Path,
    publicacao: &Publicacao,
    saida: Option<&Path>,
    pasta_padrao: &str,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("(teste a seco: a existência da tag no GitHub não foi consultada)");
    } else {
        conferir_tag_livre(root, &publicacao.tag)?;
    }

    let saida = match saida {
        Some(pasta) => pasta.to_path_buf(),
        None => target_dir()?.join(pasta_padrao),
    };
    fs::create_dir_all(&saida).with_context(|| format!("falha ao criar {}", saida.display()))?;
    for (arquivo, conteudo) in [
        ("tag.txt", &publicacao.tag),
        ("titulo.txt", &publicacao.titulo),
        ("notas.md", &publicacao.notas),
    ] {
        let caminho = saida.join(arquivo);
        fs::write(&caminho, conteudo)
            .with_context(|| format!("falha ao gravar {}", caminho.display()))?;
    }

    println!("tag:    {}", publicacao.tag);
    println!("título: {}", publicacao.titulo);
    println!("arquivos em {}", saida.display());
    if dry_run {
        println!("\n----- notas.md -----\n{}", publicacao.notas);
    }
    Ok(())
}

/// Versão do app: `version` do `tauri.conf.json` ou, sem ela, a do `Cargo.toml` da `warden-app`
/// (o Tauri faz o mesmo). Só `X.Y.Z`: a tag acrescenta o `-marco.<N>`.
pub(crate) fn versao_do_app(root: &Path) -> Result<String> {
    let tauri_dir = root.join("apps").join("desktop").join("src-tauri");
    let conf_path = tauri_dir.join("tauri.conf.json");
    let conf: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&conf_path)
            .with_context(|| format!("falha ao ler {}", conf_path.display()))?,
    )
    .with_context(|| format!("{} não é JSON válido", conf_path.display()))?;
    // O Tauri também aceita ali um caminho para `package.json`; o Warden não usa, e um caminho
    // não passa no `versao_valida` abaixo.
    let versao = match conf.get("version") {
        Some(serde_json::Value::String(versao)) => versao.clone(),
        Some(_) => bail!("`version` do tauri.conf.json não é texto"),
        None => versao_do_cargo(root, &tauri_dir.join("Cargo.toml"))?,
    };
    ensure!(
        versao_valida(&versao),
        "a versão do app `{versao}` não é X.Y.Z (números); a tag seria v{versao}-marco.<N>"
    );
    Ok(versao)
}

fn versao_do_cargo(root: &Path, manifesto: &Path) -> Result<String> {
    let ler = |caminho: &Path| -> Result<toml::Table> {
        fs::read_to_string(caminho)
            .with_context(|| format!("falha ao ler {}", caminho.display()))?
            .parse()
            .with_context(|| format!("{} não é TOML válido", caminho.display()))
    };
    let pacote = ler(manifesto)?;
    let versao = pacote
        .get("package")
        .and_then(|package| package.get("version"))
        .with_context(|| format!("{} não tem package.version", manifesto.display()))?;
    if let Some(versao) = versao.as_str() {
        return Ok(versao.to_owned());
    }
    ensure!(
        versao.get("workspace").and_then(toml::Value::as_bool) == Some(true),
        "package.version de {} não é texto nem `workspace = true`",
        manifesto.display()
    );
    ler(&root.join("Cargo.toml"))?
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .context("o Cargo.toml da raiz não tem workspace.package.version")
}

fn versao_valida(versao: &str) -> bool {
    let partes: Vec<&str> = versao.split('.').collect();
    partes.len() == 3
        && partes
            .iter()
            .all(|parte| !parte.is_empty() && parte.bytes().all(|b| b.is_ascii_digit()))
}

/// Texto de `docs/pre-versoes/marco-<N>.md`, conferido.
fn ler_marco(root: &Path, marco: u8) -> Result<String> {
    ensure!(
        MARCOS.contains(&marco),
        "o marco precisa ser de {} a {}, não {marco}",
        MARCOS.start(),
        MARCOS.end()
    );
    ler_notas(
        root,
        &format!("docs/pre-versoes/marco-{marco}.md"),
        "copie o docs/pre-versoes/MODELO.md e preencha antes de publicar",
    )
}

/// Texto de `root/<relativo>`, sem trechos `[preencher …]`; `dica` diz o que fazer se faltar.
pub(crate) fn ler_notas(root: &Path, relativo: &str, dica: &str) -> Result<String> {
    let caminho = root.join(relativo);
    let texto = fs::read_to_string(&caminho)
        .with_context(|| format!("falta o {relativo}: {dica}"))?;
    ensure!(
        !texto.contains(MARCA_PENDENTE),
        "o {relativo} ainda tem trechos `{MARCA_PENDENTE} …]` do modelo"
    );
    Ok(texto)
}

/// Separa o título (`# ...`, primeira linha não vazia) do resto do texto do marco.
pub(crate) fn separar_titulo(texto: &str) -> Result<(&str, &str)> {
    let texto = texto.trim_start();
    let (primeira, resto) = texto.split_once('\n').unwrap_or((texto, ""));
    let titulo = primeira
        .trim_end()
        .strip_prefix("# ")
        .map(str::trim)
        .filter(|titulo| !titulo.is_empty())
        .context("o arquivo das notas precisa começar por um título `# ...`")?;
    Ok((titulo, resto.trim()))
}

fn montar(
    versao: &str,
    marco: u8,
    texto_marco: &str,
    commit: &str,
    instalador: &Instalador,
) -> Result<Publicacao> {
    let (titulo_marco, corpo) = separar_titulo(texto_marco)?;
    ensure!(
        !corpo.is_empty(),
        "o arquivo do marco {marco} só tem o título"
    );
    let tag = format!("v{versao}-marco.{marco}");
    let titulo = format!("Warden {versao} · {titulo_marco} (pré-versão)");

    let notas = notas_com_bloco_fixo(
        corpo,
        versao,
        commit,
        instalador,
        "Esta é uma **pré-versão** de fim de marco, para teste.",
    );

    Ok(Publicacao {
        tag,
        titulo,
        notas,
    })
}

/// `corpo` mais o bloco fixo (como instalar, o `SmartScreen`, como relatar problemas e o
/// SHA-256 do instalador), fechado pela linha da versão e do commit seguida de `fecho`.
pub(crate) fn notas_com_bloco_fixo(
    corpo: &str,
    versao: &str,
    commit: &str,
    instalador: &Instalador,
    fecho: &str,
) -> String {
    let nome = &instalador.nome;
    let sha = instalador
        .sha256
        .as_deref()
        .unwrap_or("(calculado na publicação)");
    let curto = commit.get(..7).unwrap_or(commit);
    let mut notas = String::new();
    let _ = writeln!(notas, "{corpo}\n");
    let _ = writeln!(notas, "---\n");
    let _ = writeln!(notas, "## Como instalar\n");
    let _ = writeln!(
        notas,
        "1. Em **Assets**, logo abaixo, clique em `{nome}` para baixar o instalador."
    );
    let _ = writeln!(
        notas,
        "2. Dê duplo clique no arquivo baixado. Ele instala só para o seu usuário, sem pedir senha de administrador."
    );
    let _ = writeln!(
        notas,
        "3. Já tem o Warden instalado? Instalar de novo **atualiza** o programa e **não apaga** seus packs, configurações nem chaves.\n"
    );
    let _ = writeln!(notas, "### Se aparecer \"O Windows protegeu o computador\"\n");
    let _ = writeln!(
        notas,
        "O instalador **não tem assinatura digital** (um certificado pago; a decisão D4 do projeto foi não comprar um, porque o Warden é só para você). Por isso o Windows desconfia de arquivos assim baixados pelo navegador. Para instalar mesmo assim:\n"
    );
    let _ = writeln!(notas, "1. clique em **Mais informações**;");
    let _ = writeln!(
        notas,
        "2. confira que o aplicativo é `{nome}` e que o fornecedor aparece como **Fornecedor desconhecido**;"
    );
    let _ = writeln!(notas, "3. clique em **Executar assim mesmo**.\n");
    let _ = writeln!(
        notas,
        "Se o antivírus reclamar, não o desligue nem crie exceções: tire um print do aviso e mande ao orquestrador.\n"
    );
    let _ = writeln!(notas, "## Encontrou um problema?\n");
    let _ = writeln!(
        notas,
        "No Warden, clique em **Relatar problema** (no alto da janela, perto dos botões de fechar) ou aperte **Ctrl+Shift+E** em qualquer tela. Preencha o título e o que aconteceu, confira o texto e clique em **Abrir no GitHub**: a página da issue abre já preenchida e você só envia. Se puder, cole um print direto na issue.\n"
    );
    let _ = writeln!(notas, "## Conferência do arquivo\n");
    let _ = writeln!(notas, "| Arquivo | SHA-256 |");
    let _ = writeln!(notas, "|---|---|");
    let _ = writeln!(notas, "| `{nome}` | `{sha}` |\n");
    let _ = writeln!(
        notas,
        "Para conferir no PowerShell: `Get-FileHash .\\{nome} -Algorithm SHA256`. O resultado tem que ser igual ao da tabela (maiúsculas ou minúsculas tanto faz).\n"
    );
    let _ = writeln!(
        notas,
        "Versão {versao}, commit [`{curto}`](https://github.com/Kriticales/Warden/commit/{commit}). {fecho}"
    );
    notas

}

fn nome_do_arquivo(caminho: &Path) -> Result<String> {
    let nome = caminho
        .file_name()
        .and_then(|nome| nome.to_str())
        .with_context(|| format!("nome de arquivo inválido: {}", caminho.display()))?;
    ensure!(
        Path::new(nome)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe")),
        "o instalador precisa ser um .exe, não `{nome}`"
    );
    Ok(nome.to_owned())
}

fn sha256_do_arquivo(caminho: &Path) -> Result<String> {
    let mut arquivo = fs::File::open(caminho)
        .with_context(|| format!("falha ao abrir o instalador {}", caminho.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 16];
    loop {
        let lidos = arquivo
            .read(&mut buffer)
            .with_context(|| format!("falha ao ler {}", caminho.display()))?;
        if lidos == 0 {
            break;
        }
        hasher.update(buffer.get(..lidos).unwrap_or_default());
    }
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

/// Falha se a tag já existir no `origin` do repositório em `repo` (a criação na CI também
/// recusa sobrescrever).
fn conferir_tag_livre(repo: &Path, tag: &str) -> Result<()> {
    let referencia = format!("refs/tags/{tag}");
    let saida = Cmd::new("git")
        .cwd(repo)
        .args(["ls-remote", "--tags", "origin", &referencia])
        .read()?;
    if !saida.trim().is_empty() {
        bail!("a tag {tag} já existe no GitHub; a pré-versão não é sobrescrita (apague a tag e a release à mão, se for o caso)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "2a2b09e0123456789abcdef0123456789abcdef0";
    const MARCO: &str = "# Marco 1: Montar packs\n\n## O que chegou\n\n- Criar packs.\n";

    fn instalador(sha: Option<&str>) -> Instalador {
        Instalador {
            nome: "Warden_0.1.0_x64-setup.exe".to_owned(),
            sha256: sha.map(str::to_owned),
        }
    }

    #[test]
    fn monta_tag_titulo_e_notas() {
        let sha = "ab".repeat(32);
        let pre = montar("0.1.0", 1, MARCO, COMMIT, &instalador(Some(&sha))).unwrap();
        assert_eq!(pre.tag, "v0.1.0-marco.1");
        assert_eq!(pre.titulo, "Warden 0.1.0 · Marco 1: Montar packs (pré-versão)");
        assert!(pre.notas.starts_with("## O que chegou\n\n- Criar packs.\n"));
        assert!(!pre.notas.contains("# Marco 1"), "o título não se repete nas notas");
        for trecho in [
            "## Como instalar",
            "**Mais informações**",
            "**Executar assim mesmo**",
            "D4",
            "**Relatar problema**",
            "Ctrl+Shift+E",
            &format!("| `Warden_0.1.0_x64-setup.exe` | `{sha}` |"),
            "Get-FileHash .\\Warden_0.1.0_x64-setup.exe -Algorithm SHA256",
            "commit [`2a2b09e`](https://github.com/Kriticales/Warden/commit/2a2b09e0123456789abcdef0123456789abcdef0)",
        ] {
            assert!(pre.notas.contains(trecho), "faltou {trecho:?} nas notas:\n{}", pre.notas);
        }
    }

    #[test]
    fn teste_a_seco_sem_instalador_avisa_o_hash() {
        let pre = montar("0.1.0", 6, MARCO, COMMIT, &instalador(None)).unwrap();
        assert_eq!(pre.tag, "v0.1.0-marco.6");
        assert!(pre.notas.contains("`(calculado na publicação)`"));
    }

    #[test]
    fn titulo_obrigatorio_e_corpo_nao_vazio() {
        assert!(montar("0.1.0", 1, "Sem título\n\ntexto", COMMIT, &instalador(None)).is_err());
        assert!(montar("0.1.0", 1, "#  \n\ntexto", COMMIT, &instalador(None)).is_err());
        assert!(montar("0.1.0", 1, "# Marco 1: Só título\n\n", COMMIT, &instalador(None)).is_err());
    }

    #[test]
    fn versao_precisa_ser_xyz() {
        assert!(versao_valida("0.1.0"));
        assert!(versao_valida("12.0.31"));
        for invalida in ["0.1", "0.1.0-beta", "v0.1.0", "0..1", "0.1.0.0", ""] {
            assert!(!versao_valida(invalida), "{invalida} deveria ser recusada");
        }
    }

    fn workspace_falso(conf: &str, pacote: &str, raiz: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let tauri = dir.path().join("apps/desktop/src-tauri");
        fs::create_dir_all(&tauri).unwrap();
        fs::write(tauri.join("tauri.conf.json"), conf).unwrap();
        fs::write(tauri.join("Cargo.toml"), pacote).unwrap();
        fs::write(dir.path().join("Cargo.toml"), raiz).unwrap();
        dir
    }

    #[test]
    fn versao_vem_do_workspace_quando_o_tauri_nao_tem() {
        let dir = workspace_falso(
            r#"{"productName": "Warden"}"#,
            "[package]\nname = \"warden-app\"\nversion.workspace = true\n",
            "[workspace.package]\nversion = \"0.3.1\"\n",
        );
        assert_eq!(versao_do_app(dir.path()).unwrap(), "0.3.1");
    }

    #[test]
    fn versao_do_tauri_conf_vence() {
        let dir = workspace_falso(
            r#"{"version": "1.2.3"}"#,
            "[package]\nversion = \"9.9.9\"\n",
            "",
        );
        assert_eq!(versao_do_app(dir.path()).unwrap(), "1.2.3");
        let dir = workspace_falso(r"{}", "[package]\nversion = \"2.0.0\"\n", "");
        assert_eq!(versao_do_app(dir.path()).unwrap(), "2.0.0");
        let dir = workspace_falso(r#"{"version": "../package.json"}"#, "", "");
        assert!(versao_do_app(dir.path()).is_err());
    }

    #[test]
    fn versao_do_repositorio_e_valida() {
        let versao = versao_do_app(&workspace_root()).unwrap();
        assert!(versao_valida(&versao));
    }

    #[test]
    fn marco_fora_da_faixa_ou_sem_arquivo_ou_com_pendencia() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ler_marco(dir.path(), 0).is_err());
        assert!(ler_marco(dir.path(), 7).is_err());
        let erro = ler_marco(dir.path(), 2).unwrap_err().to_string();
        assert!(erro.contains("docs/pre-versoes/marco-2.md"), "{erro}");
        fs::create_dir_all(dir.path().join("docs/pre-versoes")).unwrap();
        fs::write(
            dir.path().join("docs/pre-versoes/marco-2.md"),
            "# Marco 2: Testar\n\n[preencher: o que testar]\n",
        )
        .unwrap();
        assert!(ler_marco(dir.path(), 2).is_err());
    }

    #[test]
    fn marco_1_do_repositorio_esta_pronto() {
        let texto = ler_marco(&workspace_root(), 1).unwrap();
        let pre = montar("0.1.0", 1, &texto, COMMIT, &instalador(None)).unwrap();
        assert!(pre.titulo.contains("Marco 1"), "{}", pre.titulo);
    }

    #[test]
    fn modelo_tem_marcas_de_pendencia() {
        let modelo = fs::read_to_string(workspace_root().join("docs/pre-versoes/MODELO.md")).unwrap();
        assert!(modelo.contains(MARCA_PENDENTE));
    }

    fn git(cwd: &Path, args: &[&str]) {
        Cmd::new("git")
            .cwd(cwd)
            .args(["-c", "user.name=t", "-c", "user.email=t@invalid", "-c", "commit.gpgsign=false"])
            .args(args)
            .read()
            .unwrap();
    }

    #[test]
    fn tag_existente_no_origin_e_recusada() {
        let dir = tempfile::tempdir().unwrap();
        let origem = dir.path().join("origem");
        let clone = dir.path().join("clone");
        fs::create_dir_all(&origem).unwrap();
        git(&origem, &["init", "-q"]);
        git(&origem, &["commit", "-q", "--allow-empty", "-m", "inicial"]);
        git(&origem, &["tag", "v0.1.0-marco.1"]);
        git(dir.path(), &["clone", "-q", "origem", "clone"]);
        let erro = conferir_tag_livre(&clone, "v0.1.0-marco.1")
            .unwrap_err()
            .to_string();
        assert!(erro.contains("já existe"), "{erro}");
        conferir_tag_livre(&clone, "v0.1.0-marco.2").unwrap();
    }

    #[test]
    fn sha256_e_nome_do_instalador() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("Warden_0.1.0_x64-setup.exe");
        fs::write(&exe, b"abc").unwrap();
        assert_eq!(
            sha256_do_arquivo(&exe).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(nome_do_arquivo(&exe).unwrap(), "Warden_0.1.0_x64-setup.exe");
        assert!(nome_do_arquivo(&dir.path().join("notas.md")).is_err());
    }
}
