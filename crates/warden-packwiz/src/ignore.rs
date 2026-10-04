//! `.packwizignore` com a semântica exata do packwiz (R3 §1.8).
//!
//! O packwiz não usa o gitignore do git: usa a biblioteca Go `github.com/sabhiram/go-gitignore`
//! (MIT, commit `525f6e181f06`), que traduz cada linha numa expressão regular com algumas
//! diferenças do git (por exemplo, `foo/*.txt` sem barra inicial fica ancorado na raiz). Este
//! módulo porta essa tradução linha a linha, para o Warden prever exatamente o que o
//! `packwiz refresh` põe no índice. Os padrões embutidos do packwiz (`ignoreDefaults` de
//! `core/index.go`) vêm antes dos do usuário, então `!padrão` consegue reincluir.
//!
//! O `refresh` percorre a pasta e **não desce** em pastas que casam com algum padrão: um
//! arquivo dentro delas fica de fora mesmo que um `!padrão` posterior o reincluísse. Use
//! [`PackwizIgnore::is_excluded`] para essa regra completa.
//!
//! Atenção: o packwiz compara os padrões com o caminho que o percurso produz. Chamado com
//! `--pack-file` absoluto, esse caminho é absoluto e os padrões ancorados (`/logs/`) deixam de
//! funcionar (verificado com o packwiz real; ver `tests/fixtures/FIXTURES.md`). Este matcher
//! reproduz o caso correto: caminhos relativos à pasta do pack, chamada com
//! `--pack-file pack.toml` a partir da pasta do pack.

use std::sync::OnceLock;

use regex::{NoExpand, Regex};

/// Padrões embutidos do packwiz (`ignoreDefaults`), na ordem do código.
pub const PACKWIZ_DEFAULT_PATTERNS: [&str; 8] = [
    ".git/**",
    ".gitattributes",
    ".gitignore",
    ".DS_Store",
    "/*.zip",
    "*.mrpack",
    "packwiz.exe",
    "packwiz",
];

/// Um padrão compilado.
#[derive(Debug, Clone)]
struct Pattern {
    regex: Regex,
    negate: bool,
    line: String,
}

/// Conjunto de padrões com a semântica do packwiz.
#[derive(Debug, Clone, Default)]
pub struct PackwizIgnore {
    patterns: Vec<Pattern>,
}

impl PackwizIgnore {
    /// O que o `refresh` usa: os padrões embutidos seguidos das linhas do `.packwizignore`
    /// (`None` se o arquivo não existe).
    #[must_use]
    pub fn for_pack(packwizignore: Option<&str>) -> Self {
        let mut lines: Vec<&str> = PACKWIZ_DEFAULT_PATTERNS.to_vec();
        if let Some(text) = packwizignore {
            // O packwiz divide só por "\n"; o "\r" do fim de linha é tirado ao traduzir.
            lines.extend(text.split('\n'));
        }
        Self::from_lines(lines)
    }

    /// Só as linhas dadas, sem os padrões embutidos.
    #[must_use]
    pub fn from_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> Self {
        let patterns = lines
            .into_iter()
            .filter_map(|line| {
                let (expression, negate) = translate_line(line)?;
                let regex = Regex::new(&expression).ok()?;
                Some(Pattern {
                    regex,
                    negate,
                    line: line.to_owned(),
                })
            })
            .collect();
        Self { patterns }
    }

    /// `MatchesPath`: se o caminho (relativo, com `/`) casa com o conjunto. Padrões com `!`
    /// só desfazem um casamento anterior.
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        self.matching_line(path).is_some()
    }

    /// A linha que fez o caminho ser ignorado, se houver (`MatchesPathHow`).
    #[must_use]
    pub fn matching_line(&self, path: &str) -> Option<&str> {
        let mut matched: Option<&str> = None;
        for pattern in &self.patterns {
            if pattern.regex.is_match(path) {
                if !pattern.negate {
                    matched = Some(&pattern.line);
                } else if matched.is_some() {
                    matched = None;
                }
            }
        }
        matched
    }

    /// Se o `refresh` deixa o arquivo de fora: o próprio arquivo casa ou alguma pasta acima
    /// dele casa (o percurso não desce nela).
    #[must_use]
    pub fn is_excluded(&self, file: &str) -> bool {
        self.excluding_line(file).is_some()
    }

    /// A linha responsável por deixar o arquivo de fora, olhando as pastas acima primeiro.
    #[must_use]
    pub fn excluding_line(&self, file: &str) -> Option<&str> {
        let mut end = 0;
        while let Some(offset) = file[end..].find('/') {
            let directory = &file[..end + offset];
            if let Some(line) = self.matching_line(directory) {
                return Some(line);
            }
            end += offset + 1;
        }
        self.matching_line(file)
    }

    /// Quantos padrões válidos o conjunto tem.
    #[must_use]
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Se não há nenhum padrão.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

/// Tradução de `getPatternFromLine`: devolve a expressão e se a linha nega (`!`), ou `None`
/// para linhas vazias e comentários.
fn translate_line(line: &str) -> Option<(String, bool)> {
    // "Trim OS-specific carriage returns."
    let line = line.trim_end_matches('\r');
    // Comentários [regra 2].
    if line.starts_with('#') {
        return None;
    }
    // Espaços nas pontas [regra 3] (só espaço, como `strings.Trim(line, " ")`).
    let mut line = line.trim_matches(' ').to_owned();
    if line.is_empty() {
        return None;
    }
    // Negação [regra 4].
    let mut negate = false;
    if line.starts_with('!') {
        negate = true;
        line.remove(0);
    }
    // `\#` e `\!` escapados: a biblioteca tira o primeiro caractere se for `#` ou `!`.
    if line.starts_with('#') || line.starts_with('!') {
        line.remove(0);
    }
    // "foo/*.blah" numa pasta: acrescenta a barra inicial.
    if regex_static(r"([^/+])/.*\*\.").is_match(&line) && !line.starts_with('/') {
        line.insert(0, '/');
    }
    // Escapa o ponto.
    line = line.replace('.', r"\.");

    let magic_star = "#$~";
    // "/**/" no começo.
    if line.starts_with("/**/") {
        line.remove(0);
    }
    line = regex_static(r"/\*\*/")
        .replace_all(&line, NoExpand("(/|/.+/)"))
        .into_owned();
    line = regex_static(r"\*\*/")
        .replace_all(&line, NoExpand(&format!("(|.{magic_star}/)")))
        .into_owned();
    line = regex_static(r"/\*\*")
        .replace_all(&line, NoExpand(&format!("(|/.{magic_star})")))
        .into_owned();
    // `\*` escapado.
    line = regex_static(r"\\\*")
        .replace_all(&line, NoExpand(&format!(r"\{magic_star}")))
        .into_owned();
    line = line.replace('*', "([^/]*)");
    // `?` vira literal.
    line = line.replace('?', r"\?");
    line = line.replace(magic_star, "*");

    let mut expression = if line.ends_with('/') {
        format!("{line}(|.*)$")
    } else {
        format!("{line}(|/.*)$")
    };
    expression = match expression.strip_prefix('/') {
        Some(rest) => format!("^(|/){rest}"),
        None => format!("^(|.*/){expression}"),
    };
    Some((go_braces_to_rust(&expression), negate))
}

/// O RE2 do Go trata `{` que não forma uma repetição (`{n}`, `{n,}`, `{n,m}`) como literal;
/// o `regex` do Rust recusa. Escapa essas chaves para as duas lerem igual.
fn go_braces_to_rust(expression: &str) -> String {
    static REPEAT: OnceLock<Regex> = OnceLock::new();
    let repeat = REPEAT.get_or_init(|| compile_static(r"^\{[0-9]+(,[0-9]*)?\}"));
    let mut out = String::with_capacity(expression.len());
    let mut rest = expression;
    while let Some(c) = rest.chars().next() {
        let mut taken = c.len_utf8();
        match c {
            '\\' => {
                out.push(c);
                if let Some(next) = rest[taken..].chars().next() {
                    out.push(next);
                    taken += next.len_utf8();
                }
            }
            '{' => match repeat.find(rest) {
                Some(found) => {
                    out.push_str(found.as_str());
                    taken = found.end();
                }
                None => out.push_str(r"\{"),
            },
            '}' => out.push_str(r"\}"),
            other => out.push(other),
        }
        rest = &rest[taken..];
    }
    out
}

/// Expressões fixas usadas na tradução, compiladas uma vez.
fn regex_static(pattern: &'static str) -> &'static Regex {
    static CACHE: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        FIXED_PATTERNS
            .iter()
            .map(|&fixed| (fixed, compile_static(fixed)))
            .collect()
    });
    cache
        .iter()
        .find(|(fixed, _)| *fixed == pattern)
        .map_or_else(|| &cache[0].1, |(_, regex)| regex)
}

const FIXED_PATTERNS: [&str; 5] = [r"([^/+])/.*\*\.", r"/\*\*/", r"\*\*/", r"/\*\*", r"\\\*"];

#[allow(clippy::expect_used)] // só expressões literais deste módulo, conferidas pelos testes
fn compile_static(pattern: &str) -> Regex {
    Regex::new(pattern).expect("expressão fixa válida")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expr(line: &str) -> Option<String> {
        translate_line(line).map(|(expression, _)| expression)
    }

    #[test]
    fn traducoes_iguais_as_da_biblioteca_go() {
        // Saídas conferidas com `getPatternFromLine` do go-gitignore.
        let cases = [
            ("/logs/", r"^(|/)logs/(|.*)$"),
            ("*.log", r"^(|.*/)([^/]*)\.log(|/.*)$"),
            (".git/**", r"^(|.*/)\.git(|/.*)(|/.*)$"),
            ("/*.zip", r"^(|/)([^/]*)\.zip(|/.*)$"),
            ("packwiz", r"^(|.*/)packwiz(|/.*)$"),
            ("foo/*.txt", r"^(|/)foo/([^/]*)\.txt(|/.*)$"),
            ("**/foo", r"^(|.*/)(|.*/)foo(|/.*)$"),
            ("a/**/b", r"^(|.*/)a(/|/.+/)b(|/.*)$"),
            ("/**/b", r"^(|.*/)(|.*/)b(|/.*)$"),
            ("a?b", r"^(|.*/)a\?b(|/.*)$"),
            (r"a\*b", r"^(|.*/)a\*b(|/.*)$"),
            ("x{1,2}", r"^(|.*/)x{1,2}(|/.*)$"),
            ("x{a}", r"^(|.*/)x\{a\}(|/.*)$"),
            ("  /a  \r", r"^(|/)a(|/.*)$"),
        ];
        for (line, expected) in cases {
            assert_eq!(expr(line).as_deref(), Some(expected), "{line}");
        }
        assert_eq!(expr("# comentário"), None);
        assert_eq!(expr("   "), None);
        assert_eq!(expr("\r"), None);
        assert_eq!(translate_line("!/a").map(|(_, negate)| negate), Some(true));
        assert_eq!(expr("!#a"), Some(r"^(|.*/)a(|/.*)$".to_owned()));
    }

    #[test]
    fn padroes_embutidos_do_packwiz() {
        let ignore = PackwizIgnore::for_pack(None);
        assert_eq!(ignore.len(), 8);
        for path in [
            ".git/config",
            ".gitignore",
            "sub/.gitattributes",
            "export.zip",
            "a/b.mrpack",
            "packwiz.exe",
            "packwiz/x",
        ] {
            assert!(ignore.matches(path), "{path}");
        }
        for path in ["mods/a.zip", "config/a.txt", "packwiz-installer.jar"] {
            assert!(!ignore.matches(path), "{path}");
        }
    }

    #[test]
    fn negacao_e_pastas_acima() {
        let ignore =
            PackwizIgnore::for_pack(Some("logs\n!logs/keep.txt\n/x/\n!x/y\n*.tmp\n!a.tmp"));
        // A pasta "logs" casa, então o refresh nem entra nela.
        assert!(ignore.matches("logs"));
        assert!(!ignore.matches("logs/keep.txt"));
        assert!(ignore.is_excluded("logs/keep.txt"));
        assert_eq!(ignore.excluding_line("logs/keep.txt"), Some("logs"));
        // "/x/" não casa com a pasta "x" (sem barra final), só com o que está dentro.
        assert!(!ignore.matches("x"));
        assert!(!ignore.is_excluded("x/y"));
        assert!(ignore.is_excluded("x/z"));
        assert!(!ignore.is_excluded("a.tmp"));
        assert!(ignore.is_excluded("b.tmp"));
        assert!(!PackwizIgnore::default().matches("x"));
        assert!(PackwizIgnore::default().is_empty());
    }

    #[test]
    fn linha_invalida_e_ignorada_como_no_go() {
        let ignore = PackwizIgnore::from_lines(["a(b", "c"]);
        assert_eq!(ignore.len(), 1);
        assert!(ignore.matches("c"));
    }

    #[test]
    fn chaves_soltas() {
        assert_eq!(go_braces_to_rust("a}b"), r"a\}b");
        assert_eq!(go_braces_to_rust(r"a\{b"), r"a\{b");
        assert_eq!(go_braces_to_rust("a{2}b{x"), r"a{2}b\{x");
        assert_eq!(go_braces_to_rust("a{,2}"), r"a\{,2\}");
        assert_eq!(go_braces_to_rust("a{1,}é"), "a{1,}é");
        assert!(Regex::new(&go_braces_to_rust("^(|.*/)x{1,2}}$")).is_ok());
    }

    #[test]
    fn expressoes_fixas_sao_validas() {
        for pattern in FIXED_PATTERNS {
            assert_eq!(regex_static(pattern).as_str(), pattern);
        }
    }
}
