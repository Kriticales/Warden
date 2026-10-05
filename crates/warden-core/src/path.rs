//! Caminhos relativos vindos de fora (interface, arquivos de pack, zips) e a raiz em que
//! precisam ficar (ARCHITECTURE §4.1 e §20).
//!
//! [`resolve_inside`] é a única porta de entrada desses caminhos. As regras são as mesmas no
//! Windows e no Linux, para um pack se comportar igual nas duas plataformas:
//!
//! - separador `/` ou `\` (os dois valem em qualquer plataforma);
//! - recusa caminho vazio, absoluto (`/x`, `\x`), com unidade (`C:\x`, `C:x`), com prefixo
//!   `\\?\` ou UNC (`\\servidor\pasta`), qualquer `..`, `:` (unidade ou fluxo alternativo do
//!   NTFS), caracteres de controle e os proibidos no Windows (`<>"|?*`), nomes reservados do
//!   Windows (`CON`, `NUL`, `COM1`…, `COM¹`…, com ou sem extensão) e componentes que terminam
//!   em ponto ou espaço (o Windows os remove e o nome passaria a apontar para outro arquivo);
//! - `.` e separadores repetidos são ignorados;
//! - depois de juntar com a raiz, segue links simbólicos e junções que já existam no caminho e
//!   recusa se o destino real sair da raiz.

use std::path::{Component, Path, PathBuf};

use crate::error::CoreError;

/// Maior caminho relativo aceito (caracteres), bem acima do que um pack usa.
const MAX_RELATIVE_LEN: usize = 1024;

/// Nomes de dispositivo do Windows: abrir `CON.txt` abre o console, não um arquivo. O
/// Windows também lê os sobrescritos `¹`, `²` e `³` como dígitos (`COM¹` é uma porta serial).
const RESERVED_NAMES: [&str; 28] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "COM¹", "COM²", "COM³", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
    "LPT9", "LPT¹", "LPT²", "LPT³",
];

/// Junta `relative` a `root` garantindo que o resultado fica dentro de `root`.
///
/// Devolve `root` + os componentes de `relative` (sem `.`), sem resolver links: o caminho
/// devolvido é o que o chamador deve usar. Erros: [`CoreError::PathOutsideRoot`] com o
/// motivo; [`CoreError::Io`] se a raiz não puder ser lida.
pub fn resolve_inside(root: &Path, relative: &str) -> Result<PathBuf, CoreError> {
    let components = lexical_components(relative)?;
    let mut resolved = root.to_path_buf();
    for component in &components {
        resolved.push(component);
    }
    ensure_no_escape_through_links(root, &resolved, relative)?;
    Ok(resolved)
}

fn reject(relative: &str, reason: &'static str) -> CoreError {
    CoreError::PathOutsideRoot {
        path: relative.chars().take(260).collect(),
        reason,
    }
}

/// Valida o texto e devolve os componentes normais, sem tocar no disco.
fn lexical_components(relative: &str) -> Result<Vec<&str>, CoreError> {
    if relative.len() > MAX_RELATIVE_LEN {
        return Err(reject(relative, "caminho longo demais"));
    }
    if relative.starts_with(['/', '\\']) {
        return Err(reject(relative, "caminho absoluto"));
    }
    if relative.contains(':') {
        return Err(reject(relative, "unidade ou fluxo alternativo"));
    }
    if relative
        .chars()
        .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Err(reject(relative, "caractere proibido"));
    }

    let mut components = Vec::new();
    for part in relative.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return Err(reject(relative, "sobe de pasta")),
            _ => {
                if part.ends_with(['.', ' ']) {
                    return Err(reject(relative, "nome termina em ponto ou espaço"));
                }
                if is_reserved_name(part) {
                    return Err(reject(relative, "nome reservado do Windows"));
                }
                components.push(part);
            }
        }
    }
    if components.is_empty() {
        return Err(reject(relative, "caminho vazio"));
    }

    // Segunda barreira: o que sobrou tem de ser só componentes normais na plataforma atual.
    let rebuilt: PathBuf = components.iter().collect();
    if !rebuilt
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(reject(relative, "componente inválido"));
    }
    Ok(components)
}

fn is_reserved_name(part: &str) -> bool {
    // O Windows ignora a extensão e os espaços antes dela: `nul .txt` também é `NUL`.
    let stem = part.split('.').next().unwrap_or(part).trim_end();
    RESERVED_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
}

/// Segue os links e junções que existem no caminho e confere que o destino real fica dentro
/// da raiz real. Partes que ainda não existem não têm como sair da raiz.
fn ensure_no_escape_through_links(
    root: &Path,
    resolved: &Path,
    relative: &str,
) -> Result<(), CoreError> {
    let real_root = match root.canonicalize() {
        Ok(real_root) => real_root,
        // Raiz inexistente: nada abaixo dela existe, então não há link para seguir.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(CoreError::io("ler", root, error)),
    };

    // Do mais profundo para o mais raso: o primeiro trecho que existe (como arquivo, pasta ou
    // link, mesmo quebrado) é o que decide.
    let mut current = resolved;
    loop {
        if current == root {
            return Ok(());
        }
        if current.symlink_metadata().is_ok() {
            return match current.canonicalize() {
                Ok(real) if real.starts_with(&real_root) => Ok(()),
                Ok(_) => Err(reject(relative, "link que sai da pasta")),
                // Link quebrado ou sem permissão: não dá para provar que fica dentro.
                Err(_) => Err(reject(relative, "link que não pode ser conferido")),
            };
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{CoreErrorCode, DomainCode, DomainError as _};
    use proptest::prelude::*;

    fn is_rejected(root: &Path, relative: &str) -> bool {
        match resolve_inside(root, relative) {
            Err(error) => {
                assert_eq!(
                    error.code(),
                    DomainCode::Core(CoreErrorCode::PathOutsideRoot),
                    "{relative:?}: {error}"
                );
                true
            }
            Ok(_) => false,
        }
    }

    #[test]
    fn aceita_caminhos_comuns() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (relative, expected) in [
            ("config/jei.toml", vec!["config", "jei.toml"]),
            ("config\\jei.toml", vec!["config", "jei.toml"]),
            ("./mods//sodium.pw.toml", vec!["mods", "sodium.pw.toml"]),
            ("a/./b/", vec!["a", "b"]),
            (".minecraft/options.txt", vec![".minecraft", "options.txt"]),
            (
                "kubejs/server_scripts/ação.js",
                vec!["kubejs", "server_scripts", "ação.js"],
            ),
            ("CONSOLE.txt", vec!["CONSOLE.txt"]),
            ("com10", vec!["com10"]),
            ("..a/b..c", vec!["..a", "b..c"]),
        ] {
            let resolved = resolve_inside(root, relative).unwrap();
            let mut wanted = root.to_path_buf();
            wanted.extend(expected);
            assert_eq!(resolved, wanted, "{relative:?}");
        }
    }

    /// Critério 2 da F0-05: `../x`, caminhos absolutos e `C:\x` são recusados.
    #[test]
    fn f0_05_ca2_recusa_casos_conhecidos() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for relative in [
            "",
            ".",
            "./",
            "../x",
            "..\\x",
            "a/../../x",
            "a/../b",
            "..",
            "/etc/passwd",
            "\\Windows\\System32",
            "C:\\x",
            "C:/x",
            "c:x",
            "C:",
            "\\\\?\\C:\\x",
            "//?/C:/x",
            "\\\\servidor\\pasta\\x",
            "//servidor/pasta/x",
            "\\\\.\\PhysicalDrive0",
            "arquivo.txt:fluxo",
            "a/CON",
            "nul.txt",
            "Com1.log",
            "LPT9",
            "aux .txt",
            "pasta./x",
            "pasta /x",
            "a\0b",
            "a\nb",
            "a?b",
            "a*b",
            "a|b",
            "a<b",
            "a>b",
            "a\"b",
        ] {
            assert!(
                is_rejected(root, relative),
                "{relative:?} deveria ser recusado"
            );
        }
        assert!(is_rejected(root, &"a/".repeat(600)));
    }

    /// Nomes que o Win32 mudaria: ponto ou espaço no fim (inclusive `.. ` e `...`) e os
    /// dispositivos com dígito sobrescrito (`COM¹`–`COM³`, `LPT¹`–`LPT³`).
    #[test]
    fn recusa_nomes_que_o_windows_mudaria() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for relative in [
            ".. /x",
            ".. ",
            "a/.. ",
            "a/.. .",
            "a/...",
            ". /x",
            "pack.toml.",
            "COM\u{b9}",
            "x/lpt\u{b3}.txt",
            "com\u{b2} .log",
        ] {
            assert!(
                is_rejected(root, relative),
                "{relative:?} deveria ser recusado"
            );
        }
        for relative in ["..\u{2074}/x", "com\u{2074}", "lpt0", "a. b/c"] {
            assert!(resolve_inside(root, relative).is_ok(), "{relative:?}");
        }
    }

    #[test]
    fn mensagem_traz_o_motivo_e_o_caminho() {
        let error = resolve_inside(Path::new("raiz"), "../segredo").unwrap_err();
        let text = error.to_string();
        assert!(
            text.contains("../segredo") && text.contains("sobe de pasta"),
            "{text}"
        );
    }

    #[test]
    fn raiz_inexistente_ainda_valida_o_texto() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ainda-nao-existe");
        assert_eq!(
            resolve_inside(&root, "a/b").unwrap(),
            root.join("a").join("b")
        );
        assert!(is_rejected(&root, "../x"));
    }

    #[test]
    fn caminho_que_existe_dentro_da_raiz_e_aceito() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config/sub")).unwrap();
        std::fs::write(dir.path().join("config/sub/a.toml"), b"").unwrap();
        assert!(resolve_inside(dir.path(), "config/sub/a.toml").is_ok());
        assert!(resolve_inside(dir.path(), "config/sub/novo/b.toml").is_ok());
    }

    /// Cria um link de pasta: junção no Windows (não exige privilégio), symlink no Linux.
    fn link_dir(target: &Path, link: &Path) -> bool {
        #[cfg(windows)]
        {
            junction::create(target, link).is_ok()
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
    }

    /// Link simbólico de arquivo: no Windows só funciona com o Modo de Desenvolvedor ou como
    /// administrador; sem isso, o teste diz que pulou.
    fn link_file(target: &Path, link: &Path) -> bool {
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
    }

    /// Critério 2 da F0-05: links (junção no Windows) que saem da raiz são recusados.
    #[test]
    fn f0_05_ca2_recusa_juncao_ou_link_de_pasta_para_fora() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("segredo.txt"), b"x").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(
            link_dir(outside.path(), &root.join("fuga")),
            "não criou o link de pasta"
        );

        assert!(is_rejected(root, "fuga"));
        assert!(is_rejected(root, "fuga/segredo.txt"));
        assert!(is_rejected(root, "fuga/novo/arquivo.txt"));
    }

    #[test]
    fn aceita_link_de_pasta_que_fica_dentro_da_raiz() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("real")).unwrap();
        assert!(link_dir(&root.join("real"), &root.join("atalho")));
        assert_eq!(
            resolve_inside(root, "atalho/a.txt").unwrap(),
            root.join("atalho").join("a.txt")
        );
    }

    #[test]
    #[allow(clippy::print_stderr)] // aviso de caso pulado, ver abaixo
    fn f0_05_ca2_recusa_link_de_arquivo_para_fora_e_link_quebrado() {
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("segredo.txt");
        std::fs::write(&target, b"x").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        if !link_file(&target, &root.join("fuga.txt")) {
            // Sem privilégio para link simbólico de arquivo (Windows sem Modo de
            // Desenvolvedor): a junção acima já cobre a fuga por link. O aviso vai para a
            // saída do teste (QUALITY §4.1: teste pulado é declarado).
            eprintln!("aviso: link simbólico de arquivo indisponível; caso pulado");
            return;
        }
        assert!(is_rejected(root, "fuga.txt"));
        std::fs::remove_file(&target).unwrap();
        assert!(is_rejected(root, "fuga.txt"));
    }

    fn segment() -> impl Strategy<Value = String> {
        prop_oneof![
            4 => "[a-zA-Z0-9_\\-]{1,12}",
            1 => Just(".".to_owned()),
            1 => Just("..".to_owned()),
            1 => Just(String::new()),
            1 => "[a-zA-Z]:",
            1 => "(?i)(con|nul|aux|prn|com[1-9\u{b9}\u{b2}\u{b3}]|lpt[1-9\u{b9}\u{b2}\u{b3}])(\\.[a-z]{1,3})?",
            1 => "[a-z]{1,5}[. ]",
            1 => "\\.{1,3}[ .]{1,3}",
            1 => "\\.\\.[\u{b2}\u{b3}\u{b9}\u{2074}\u{a0}\u{3000}\u{ff0e}]",
            1 => "[a-z]{1,4}[<>\"|?*\\x00-\\x1f][a-z]{0,4}",
        ]
    }

    fn prefix() -> impl Strategy<Value = String> {
        prop_oneof![
            6 => Just(String::new()),
            1 => Just("/".to_owned()),
            1 => Just("\\".to_owned()),
            1 => Just("\\\\?\\".to_owned()),
            1 => Just("\\\\?\\UNC\\".to_owned()),
            1 => Just("\\\\".to_owned()),
            1 => Just("//".to_owned()),
            1 => "[a-zA-Z]:[\\\\/]?",
        ]
    }

    fn relative_path() -> impl Strategy<Value = String> {
        (
            prefix(),
            prop::collection::vec(segment(), 0..8),
            prop::collection::vec(prop_oneof![Just("/"), Just("\\"), Just("//")], 8),
        )
            .prop_map(|(prefix, segments, separators)| {
                let mut text = prefix;
                for (index, segment) in segments.iter().enumerate() {
                    if index > 0 {
                        text.push_str(separators[index]);
                    }
                    text.push_str(segment);
                }
                text
            })
    }

    proptest! {
        // Semente fixa: os mesmos casos em toda execução e em todas as máquinas.
        #![proptest_config(ProptestConfig {
            cases: 4096,
            rng_seed: proptest::test_runner::RngSeed::Fixed(0x5741_5244_454e),
            ..ProptestConfig::default()
        })]

        /// Critério 2 da F0-05: para qualquer entrada aceita, o resultado começa na raiz, não
        /// tem `..`, unidade, prefixo nem raiz, e cada componente é um nome comum. No Windows,
        /// o próprio `GetFullPathNameW` (por trás de `std::path::absolute`) confirma que não
        /// muda nenhum componente.
        #[test]
        fn f0_05_ca2_resultado_aceito_fica_sempre_dentro(relative in relative_path()) {
            let root = Path::new("raiz-inexistente-do-teste");
            if let Ok(resolved) = resolve_inside(root, &relative) {
                #[cfg(windows)]
                prop_assert_eq!(
                    std::path::absolute(&resolved).unwrap(),
                    std::path::absolute(root).unwrap().join(resolved.strip_prefix(root).unwrap()),
                    "{:?}", relative
                );
                let rest = resolved.strip_prefix(root).unwrap();
                prop_assert!(!rest.as_os_str().is_empty());
                for component in rest.components() {
                    let Component::Normal(name) = component else {
                        return Err(TestCaseError::fail(format!("componente {component:?} em {relative:?}")));
                    };
                    let name = name.to_str().unwrap();
                    prop_assert!(name != ".." && !name.contains(':'));
                    prop_assert!(!name.ends_with(['.', ' ']));
                    prop_assert!(!is_reserved_name(name));
                }
                prop_assert!(!relative.starts_with(['/', '\\']));
                prop_assert!(!relative.contains(':'));
                prop_assert!(!relative.split(['/', '\\']).any(|part| part == ".."));
            }
        }

        /// Toda entrada com `..`, prefixo, unidade ou raiz é recusada.
        #[test]
        fn f0_05_ca2_entradas_perigosas_sempre_recusadas(
            prefix in prefix(),
            segments in prop::collection::vec(segment(), 1..6),
        ) {
            let relative = format!("{prefix}{}", segments.join("/"));
            let dangerous = !prefix.is_empty()
                || segments.iter().any(|segment| segment == ".." || segment.contains(':'));
            if dangerous {
                prop_assert!(resolve_inside(Path::new("raiz"), &relative).is_err(), "{relative:?}");
            }
        }

        /// Texto arbitrário nunca entra em pânico.
        #[test]
        fn texto_arbitrario_nao_entra_em_panico(relative in "\\PC{0,64}") {
            let _ = resolve_inside(Path::new("raiz"), &relative);
        }
    }
}
