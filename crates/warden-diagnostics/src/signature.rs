//! Assinatura de travamento (ARCHITECTURE §9.3 item 6 e §9.6; R5A §7.5; ROADMAP D-06).
//!
//! A assinatura junta o que identifica a **causa** de um travamento e deixa de fora o que muda
//! de uma vez para outra:
//! - o id do padrão do catálogo que deu o achado principal (`crash_report.ticking_entity`);
//! - o tipo da exceção (a causa mais profunda, quando há `Caused by:`; as exceções suprimidas,
//!   `Suppressed:`, e as causas delas ficam de fora); no Fabric com nomes intermediary o tipo
//!   pode ser `net.minecraft.class_148`;
//! - o primeiro frame de mod da pilha, normalizado: só classe e método, sem número de linha,
//!   módulo, jar, endereço, classes geradas (`$$Lambda$5/1234567`, `LambdaForm$MH/0x…`), sufixos
//!   de lambda, contadores do `MixinExtras` (`…$36`, `mixinextras$bridge$…$57`) nem o prefixo e o
//!   id que o Mixin dá aos métodos injetados (`handler$zza00a$meumod$aoTick` → `meumod$aoTick`;
//!   o id do mod pode ter hífen, `fabric-registry-sync-v0`) e o id de sessão dos métodos
//!   fundidos (`md6558fa$meumod$lambda$tick$0$1` → `md$meumod$lambda$tick`); numa queda nativa
//!   da JVM, o módulo do frame problemático sem o deslocamento nem endereço. Um método injetado
//!   ou fundido pelo Mixin é frame de mod mesmo numa classe do jogo; quando o nome não traz o mod
//!   (Forge), as configs de mixin anotadas no frame (`{re:mixin,pl:mixin:APP:create.mixins.json:…}`)
//!   entram no sujeito (`frame_mixin=create.mixins.json`). Frame de mod é o de um pacote que não é
//!   do jogo, do loader, do Java, de biblioteca (guava, fastutil, joml…, mesmo com o jar anotado)
//!   nem de launcher; o que vem depois do `Main.main` do jogo é o launcher. Quando o jogo quebrou
//!   acima do frame de mod (o mod chamou o jogo, ou embrulha um trecho inteiro dele com
//!   `@WrapOperation`), o ponto da queda entra no sujeito (`where=…GameRenderer.renderLevel`);
//! - sem frame de mod, quando onde quebrou importa (achado do crash report, genérico ou nenhum):
//!   o **frame da queda**, o primeiro frame da causa mais profunda que não é do Java (`java.`,
//!   `jdk.`, `sun.`, reflexão), de biblioteca, de launcher, de uma classe que só confere e lança
//!   (`Preconditions` do Guava, `Validate` do commons-lang) nem de uma lambda gerada (`$$Lambda`),
//!   com o fim da pilha de cima no lugar do `... N more` (`net.minecraft.client.renderer.GameRenderer.render`),
//!   e só as configs de mixin anotadas nesse frame. Uma config anotada num frame de baixo
//!   (`Minecraft.runTick` quase sempre tem, num pack com mods) não escolhe o frame nem entra na
//!   assinatura: senão quedas diferentes teriam a mesma assinatura e o título diria "em
//!   Minecraft.runTick". Exceção sem nenhum frame (a JVM omite a pilha das exceções repetidas)
//!   usa a pilha da seção `-- Head --` do crash report. Sem nenhum frame, ficam a exceção e a
//!   descrição do crash report. Um achado do ambiente (falta de memória, Java errado, opção da
//!   JVM) não usa a pilha: quem alocou por último não identifica o travamento;
//! - os mods citados pelo achado principal (ids; jars sem a versão no nome);
//! - o **sujeito** do achado: os parâmetros que o catálogo usa para identificá-lo (`key`: o mod e
//!   a dependência, a config de mixin, a classe…) e, no crash report, a descrição e o que estava
//!   sendo atualizado ("Ticking entity": o tipo e a classe da entidade; "Ticking block entity": o
//!   bloco e a classe). Sem isso, dois crash reports diferentes só com classes do jogo na pilha
//!   teriam a mesma assinatura. Numa falha de mixin que o catálogo não identificou (só o
//!   `InjectionError` na cadeia), a config lida da mensagem (`mixin_config=x.mixins.json`) ou,
//!   sem ela, a classe que estava sendo transformada (`mixin_target=…`).
//!
//! Mensagens, coordenadas, UUIDs, caminhos e horários nunca entram: dois travamentos com a mesma
//! causa e números diferentes têm o mesmo [`CrashSignature::hash`] (CA-T14-08). O hash é estável
//! entre versões do app enquanto [`SIGNATURE_VERSION`] não mudar. O corpus adversário
//! (`tests/corpus-assinatura/`, relatórios reais) prova a regra: mesmo grupo ⇔ mesmo hash.
//!
//! Limites: só lê a análise e as fontes já carregadas; não abre arquivos.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::LazyLock;

use md5::{Digest as _, Md5};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::model::{Evidence, Finding, ItemRef};
use crate::postcrash::{
    Analysis, CleanLine, LogLoader, LogSource, SourceKind, crash_blocks, deepest_cause,
    is_library_jar, mods_in, pattern_identity, stack_range, text_fixed_regex as fixed_regex,
};

/// Versão das regras de normalização. Mudar as regras muda os hashes: suba a versão.
///
/// 2: o sujeito do achado entra na assinatura; o id dos métodos injetados pelo Mixin sai em
/// todas as formas.
/// 3: método injetado pelo Mixin é frame de mod (com as configs anotadas); sem frame de mod, o
/// primeiro frame anotado com config de mixin ou o primeiro da causa mais profunda; id de mixin
/// reconhecido pela posição; a pilha continua depois de `[...]`.
/// 4: sem frame de mod, o frame da queda (o primeiro da causa mais profunda fora do Java e das
/// lambdas geradas, com o fim da pilha de cima no lugar do `... N more`) e só as configs de mixin
/// anotadas nele; config anotada num frame de baixo não escolhe o frame.
/// 5: o id de sessão dos métodos fundidos pelo Mixin (`md1a2b3c$…`) e os contadores do
/// `MixinExtras` saem; método injetado com id de mod com hífen (`handler$…$fabric-api-x$…`) é
/// lido; a classe gerada do Java 8 (`$$Lambda$5/1234567`) não é frame de mod; frames do
/// launcher (depois do `Main.main` do jogo) e de bibliotecas (pelo pacote, mesmo com jar
/// anotado) não são frames de mod, e o frame da queda pula as bibliotecas; exceção sem pilha
/// usa a pilha do `-- Head --`; tipo de exceção intermediary (`net.minecraft.class_148`)
/// reconhecido; falha de mixin sem achado próprio leva a config (ou o alvo) no sujeito;
/// `Suppressed:` fica fora das seções; endereço no frame nativo mascarado; o frame da queda
/// também pula as classes que só conferem e lançam (`Preconditions` do Guava).
pub const SIGNATURE_VERSION: u32 = 5;

/// Até quantas linhas depois da evidência a exceção do achado é procurada.
const EXCEPTION_WITHIN: usize = 60;

/// Padrões cujo problema é do ambiente (memória, versão do Java, opções da JVM): a pilha não
/// entra na assinatura.
const ENVIRONMENT_PATTERNS: [&str; 3] = ["memory.", "java.", "jvm."];

/// Máximo de mods guardados na assinatura.
const MAX_MODS: usize = 8;

/// Linha de exceção (`java.lang.NullPointerException: …`, `Caused by: …`). No Fabric com nomes
/// intermediary, as exceções do jogo chamam-se `net.minecraft.class_148` (`ReportedException`),
/// `net.minecraft.class_151` (`InvalidIdentifierException`)…: o nome não termina em `Exception`.
static EXCEPTION_LINE: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"^\s*(?:Caused by(?: \d+)?: |Exception in thread .*? )?(?P<type>(?:[A-Za-z_$][\w$]*\.)+(?:[\w$]*(?:Exception|Error|Throwable)[\w$]*|class_\d+(?:\$class_\d+)*))(?::|\s*$)",
    )
});

/// Frame Java: `at [módulo/]pacote.Classe.método(Arquivo.java:12) [~jar]`. O método aceita hífen:
/// o fork do Mixin do Fabric põe o id do mod no nome do método injetado
/// (`handler$ddo000$fabric-registry-sync-v0$disconnectAfter`).
static JAVA_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"^\s*at\s+(?:[^\s/(]*/)*(?P<method>[\w$.<>\-]+)\((?P<location>[^)]*)\)(?P<rest>.*)$",
    )
});

/// O sufixo das classes escondidas (`$$Lambda$5/1234567890` no Java 8, `$$Lambda/0x0000…` e
/// `LambdaForm$MH/0x0000…` nos novos): muda a cada execução e, com a barra, seria lido como
/// prefixo de módulo (o método viraria `1234567890.run`).
static HIDDEN_CLASS: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"/(?:0x[0-9a-fA-F]+|\d+)(?P<method>\.[\w$<>\-]+\()"));

/// Frame nativo do `hs_err`: `C  [ig9icd64.dll+0x8599ee]`, `J 123 C2 pacote.Classe.m()V …`,
/// `v  ~StubRoutines::jbyte_disjoint_arraycopy`, `C  0x00007ff8…` (sem módulo).
static NATIVE_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"^#?\s*(?P<kind>[CJjVv])\s+(?:\[(?P<module>[^\]+]+)|(?:\d+\s+)?(?:C\d\s+)?(?P<method>~?[\w$.<>/:]+))",
    )
});

/// Endereço num frame nativo sem módulo (`C  0x00007ff8a1b2c3d4`): muda a cada execução.
static HEX_ADDRESS: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"0x[0-9a-fA-F]+"));

/// Classes geradas para lambdas (`$$Lambda$123/0x0000…`, `$$Lambda/0x…`).
static LAMBDA_CLASS: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"\$\$Lambda(?:\$\d+)?(?:/0x[0-9a-fA-F]+)?"));

/// Métodos de lambda (`lambda$tick$12`, `lambda$3`; do Kotlin, `aoTick$lambda-3`) e
/// acessores sintéticos (`access$000`): o número muda com a compilação.
static LAMBDA_METHOD: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"(lambda(?:\$[A-Za-z_][\w]*?)?)[$-]\d+\b|(access\$)\d+\b"));

/// Sufixo dos métodos fundidos pelo Mixin com o prefixo preservado (`$md$1a2b3c$0`,
/// `$md$create$1a2b3c$0`): os seis hexadecimais são o fim do id da sessão do Mixin (um UUID
/// sorteado a cada execução) e o índice conta por classe.
static MIXIN_MERGED: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"\$md\$(?:[A-Za-z0-9_\-]+\$)?[0-9a-f]{6}\$[0-9a-f]+"));

/// Prefixo dos métodos (`md`) e campos (`fd`) fundidos pelo Mixin sem prefixo preservado
/// (`md23f78d$lambda$setEmf$0$0`, no Fabric `md6558fa$meumod$lambda$tick$0$1`): o id da sessão
/// sai, o resto fica.
static MIXIN_UNIQUE: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"^(?P<kind>md|fd)[0-9a-f]{6}\$"));

/// Contador no fim de um nome de método (`mixinextras$bridge$method_25402$116`,
/// `wrapOperation$…$wrapIsServerCall$36`, o índice dos métodos fundidos): depende da ordem em
/// que as classes carregam.
static TRAILING_COUNTER: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"(?:\$\d+)+$"));

/// Prefixos que o Mixin e o `MixinExtras` dão aos métodos injetados (`handler$…`, `redirect$…`).
const MIXIN_PREFIXES: [&str; 15] = [
    "handler",
    "redirect",
    "modify",
    "localvar",
    "constant",
    "args",
    "wrapOperation",
    "wrapWithCondition",
    "wrapMethod",
    "modifyExpressionValue",
    "modifyReturnValue",
    "modifyReceiver",
    "modifyVariable",
    "modifyArg",
    "modifyArgs",
];

/// O id que o Mixin dá a cada método injetado (`zza000`, `zza00a`, `bkb1f3`, `zzaabc`): três
/// letras e um número em hexadecimal (que pode não ter algarismo). Muda de uma sessão para outra
/// conforme a ordem em que as classes carregam.
static MIXIN_ID: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"^[a-z]{3}[0-9a-f]{3,}$"));

/// Config de mixin aplicada à classe do frame, na anotação do `ModLauncher` (Forge 1.13+ e
/// NeoForge): `pl:mixin:APP:create.mixins.json:MixinClasse`, com ou sem ` from mod create`.
static FRAME_MIXIN_CONFIG: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"pl:mixin:APP:(?P<config>[^:,{}\s]+\.json)(?::[^,{}\s]*)?(?:\s+from mod (?P<mod>[^,{}\s]+))?",
    )
});

/// Fim de uma seção `Caused by:` que repete o fim da pilha de cima: `... 3 more`.
static MORE: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"^\s*\.\.\.\s*(?P<count>\d+)\s+more\b"));

/// Trecho omitido no meio da pilha de um crash report (`[...]`, num `StackOverflowError`).
static ELIDED: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"^\s*\[\.\.\.\]\s*$"));

/// Pacotes do próprio Java: o primeiro frame da causa mais profunda é o primeiro fora deles
/// (`Objects.requireNonNull` não diz onde quebrou).
const JAVA_PACKAGES: [&str; 5] = ["java.", "javax.", "jdk.", "sun.", "com.sun."];

/// Classes de bibliotecas que só conferem um argumento e lançam a exceção
/// (`Preconditions.checkState`): como `Objects.requireNonNull`, não dizem onde quebrou, e quedas
/// em chamadores diferentes teriam a mesma assinatura.
const THROW_HELPERS: [&str; 3] = [
    "com.google.common.base.Preconditions.",
    "com.google.common.base.Verify.",
    "org.apache.commons.lang3.Validate.",
];

/// A exceção que estoura a pilha: o topo dela cai num ponto qualquer da recursão.
const STACK_OVERFLOW: &str = "java.lang.StackOverflowError";

/// O jar anotado num frame (`~[meumod-1.0.jar%2312!/:1.0]`, `[meumod-1.0.jar:?]`).
static ANNOTATED_JAR: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"\[(?P<jar>[^\[\]%!/\\:\s]+?\.jar)"));

/// Classes geradas com contador (`ASMEventHandler_12_…`, `GeneratedMethodAccessor34`,
/// `$Proxy56`).
static GENERATED_CLASS: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"(ASMEventHandler_)\d+_|(Generated(?:Method|Constructor|SerializationConstructor)Accessor)\d+|(\$Proxy)\d+",
    )
});

/// Descrição do crash report.
static DESCRIPTION: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"^\s*Description:\s*(?P<text>\S.*?)\s*$"));

/// A descrição de um crash report de entidade ou de bloco que quebrou ao ser atualizado.
static TICKING: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"(?i)^ticking (?:(?P<block>block |tile )?entity|player)$"));

/// Entidade do "Ticking entity": `Entity Type: alpha:golem (com.alpha.Golem)`.
static ENTITY_TYPE: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(r"^\s*Entity Type:\s*(?P<kind>[^\s(]+)(?:\s*\((?P<class>[^)\s]+)\))?")
});

/// Bloco do "Ticking block entity": `Name: minecraft:furnace // net.minecraft….FurnaceBlockEntity`.
static BLOCK_ENTITY: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"^\s*Name:\s*(?P<kind>[^\s/]+)(?:\s*//\s*(?P<class>\S+))?"));

/// Números (contadores, coordenadas, ids) numa descrição ou argumento.
static NUMBERS: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"0x[0-9a-fA-F]+|\d+(?:[.,]\d+)*"));

/// Sufixo de versão no nome de um jar (`-1.20.1-0.5.1.f`, `_1.2`, `+mc1.20`).
static JAR_VERSION: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"[-_+](?:mc|v)?\d.*$|%23\d+$"));

/// Frames do `main` do jogo: o que vem depois na pilha é o launcher (`com.modrinth.theseus`,
/// `org.prismlauncher`, o `ForgeWrapper`…), nunca um mod.
const GAME_MAIN: [&str; 4] = [
    "net.minecraft.client.main.Main.main",
    "net.minecraft.client.Main.main",
    "net.minecraft.server.Main.main",
    "net.minecraft.server.MinecraftServer.main",
];

/// Pacotes do jogo e dos loaders: não são de mod, mas dizem onde o jogo quebrou.
const GAME_PACKAGES: [&str; 7] = [
    "net.minecraft.",
    "com.mojang.",
    "net.minecraftforge.",
    "net.neoforged.",
    "cpw.mods.",
    "net.fabricmc.",
    "org.quiltmc.",
];

/// Pacotes de bibliotecas, do Java e de launchers: não são de mod nem dizem onde o jogo
/// quebrou (o frame da queda pula estes).
const LIBRARY_PACKAGES: [&str; 39] = [
    "java.",
    "javax.",
    "jdk.",
    "sun.",
    "com.sun.",
    "org.spongepowered.",
    "com.llamalad7.mixinextras.",
    "org.lwjgl.",
    "io.netty.",
    "com.google.",
    "org.apache.",
    "it.unimi.",
    "org.objectweb.",
    "kotlin.",
    "scala.",
    "org.slf4j.",
    "oshi.",
    "org.prismlauncher.",
    "org.multimc.",
    "org.jline.",
    "com.electronwill.",
    "net.jodah.",
    "org.openjdk.",
    "org.joml.",
    "com.github.benmanes.",
    "org.jetbrains.",
    "com.ibm.icu.",
    "com.fasterxml.",
    "org.yaml.",
    "org.lz4.",
    "net.jpountz.",
    "org.bouncycastle.",
    "org.antlr.",
    "org.codehaus.",
    "org.ow2.",
    "com.modrinth.theseus.",
    "io.github.zekerzhayard.forgewrapper.",
    "org.polymc.",
    "net.technicpack.",
];

/// Se o método (classe e método) é de um pacote do jogo, do loader, do Java ou de biblioteca:
/// nunca um frame de mod.
fn is_not_mod_package(method: &str) -> bool {
    GAME_PACKAGES
        .iter()
        .chain(LIBRARY_PACKAGES.iter())
        .any(|prefix| method.starts_with(prefix))
}

/// Se o método é de biblioteca, do Java ou de launcher: não diz onde o jogo quebrou.
fn is_library_package(method: &str) -> bool {
    LIBRARY_PACKAGES
        .iter()
        .any(|prefix| method.starts_with(prefix))
}

/// A assinatura de um travamento.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CrashSignature {
    /// [`SIGNATURE_VERSION`] das regras que calcularam o hash.
    pub version: u32,
    /// Hash estável das partes abaixo (16 dígitos hexadecimais). Agrupa os travamentos.
    pub hash: String,
    /// Id do padrão do catálogo do achado principal (`None`: nenhum padrão reconheceu).
    pub rule: Option<String>,
    /// Tipo da exceção (`java.lang.NullPointerException`).
    pub exception: Option<String>,
    /// Primeiro frame de mod, normalizado (`com.example.Foo.tick`).
    pub frame: Option<String>,
    /// Mods citados, normalizados e em ordem (`mod:balm`, `jar:create`).
    pub mods: Vec<String>,
    /// O sujeito do achado, normalizado (`description=rendering overlay`,
    /// `entity_type=alpha:golem`, `entity_class=com.alpha.Golem`, `mixin_config=x.mixins.json`).
    #[serde(default)]
    pub subject: Vec<String>,
}

impl CrashSignature {
    /// Monta a assinatura com as partes já normalizadas e calcula o hash.
    #[must_use]
    pub fn new(
        rule: Option<String>,
        exception: Option<String>,
        frame: Option<String>,
        mods: Vec<String>,
        subject: Vec<String>,
    ) -> Self {
        let mods: Vec<String> = mods
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(MAX_MODS)
            .collect();
        let mut hasher = Md5::new();
        hasher.update(format!("v{SIGNATURE_VERSION}").as_bytes());
        for part in [&rule, &exception, &frame] {
            hasher.update([0x1e]);
            hasher.update(part.as_deref().unwrap_or_default().as_bytes());
        }
        for name in &mods {
            hasher.update([0x1f]);
            hasher.update(name.as_bytes());
        }
        for part in &subject {
            hasher.update([0x1d]);
            hasher.update(part.as_bytes());
        }
        let digest = hasher.finalize();
        let hash = digest[..8]
            .iter()
            .fold(String::with_capacity(16), |mut out, byte| {
                use std::fmt::Write as _;
                let _ = write!(out, "{byte:02x}");
                out
            });
        Self {
            version: SIGNATURE_VERSION,
            hash,
            rule,
            exception,
            frame,
            mods,
            subject,
        }
    }
}

/// A assinatura do travamento analisado. `sources` são as mesmas fontes passadas a
/// [`crate::analyze`] (a pilha é lida delas, a partir da evidência do achado principal).
#[must_use]
pub fn signature(analysis: &Analysis, sources: &[LogSource]) -> CrashSignature {
    let fabric = matches!(analysis.loader, Some(LogLoader::Fabric | LogLoader::Quilt));
    let Some(primary) = analysis.primary() else {
        let (exception, place) = first_crash_block(sources, fabric).unwrap_or_default();
        let mut subject = report_subject(sources, None);
        place.extend_subject(&mut subject);
        return CrashSignature::new(None, exception, place.frame, Vec::new(), subject);
    };
    let finding = &primary.finding;
    let at = finding
        .evidence()
        .iter()
        .find_map(|evidence| match evidence {
            Evidence::Log { file, line, .. } => sources
                .iter()
                .find(|source| source.name() == file)
                .map(|source| (source, *line)),
            _ => None,
        });
    let mods = finding.items().iter().map(normalize_item).collect();
    let (key, generic) = pattern_identity(&primary.pattern);
    let mut subject = key_subject(finding, key);
    // O crash report diz o que quebrou quando o achado é dele ou é genérico sem sujeito próprio.
    // Um achado de causa certa (falta de memória, Java errado…) não depende de onde quebrou.
    let where_matters =
        primary.pattern.starts_with("crash_report.") || (generic && subject.is_empty());
    // Falta de memória, Java errado, opção da JVM: o problema é do ambiente, e o frame de mod
    // que por acaso estava rodando (quem alocou por último) não identifica o travamento.
    let environment = ENVIRONMENT_PATTERNS
        .iter()
        .any(|prefix| primary.pattern.starts_with(prefix));
    let (scanned_exception, place) = at
        .filter(|_| !environment)
        .and_then(|(source, line)| stack_at(source, line, fabric, where_matters))
        .unwrap_or_default();
    let exception = finding
        .param("cause_type")
        .or_else(|| finding.param("exception_type"))
        .map(ToOwned::to_owned)
        .or(scanned_exception);
    if where_matters {
        for part in report_subject(sources, at) {
            if !subject.contains(&part) {
                subject.push(part);
            }
        }
    }
    // `server.ticking_entity`: se foi entidade ou bloco.
    if let Some(what) = finding.param("what") {
        subject.push(format!("what={}", normalize_param("what", what)));
    }
    place.extend_subject(&mut subject);
    CrashSignature::new(
        Some(primary.pattern.clone()),
        exception,
        place.frame,
        mods,
        subject,
    )
}

/// Os parâmetros que o catálogo usa para identificar o achado (`key`), normalizados. O tipo da
/// exceção já tem lugar próprio na assinatura.
fn key_subject(finding: &Finding, key: &[String]) -> Vec<String> {
    key.iter()
        .map(String::as_str)
        .filter(|name| !matches!(*name, "exception_type" | "cause_type"))
        .filter_map(|name| {
            let value = normalize_param(name, finding.param(name)?);
            (!value.is_empty()).then(|| format!("{name}={value}"))
        })
        .collect()
}

/// Um parâmetro do achado sem as partes que mudam de uma vez para outra.
fn normalize_param(name: &str, value: &str) -> String {
    let value = value.trim();
    match name {
        "file" => jar_stem(value),
        "class" | "entity_class" | "block_class" => normalize_class(value),
        "mod" | "dependency" | "other" => value.to_lowercase().replace('-', "_"),
        "description" | "argument" | "what" => {
            let lower = value.to_lowercase();
            let masked = NUMBERS.replace_all(&lower, "#");
            masked.split_whitespace().collect::<Vec<_>>().join(" ")
        }
        _ => value.to_lowercase(),
    }
}

/// O sujeito do crash report do travamento: o bloco que contém `at` (a evidência do achado) ou,
/// se ela não está num crash report, o primeiro crash report das fontes (os arquivos de crash
/// report antes dos logs).
fn report_subject(sources: &[LogSource], at: Option<(&LogSource, u32)>) -> Vec<String> {
    let from_evidence = at.and_then(|(source, number)| {
        let lines = source.clean_lines();
        let index = lines.iter().position(|line| line.number == number)?;
        let block = crash_blocks(&lines, source.kind() == SourceKind::CrashReport)
            .into_iter()
            .find(|block| block.contains(&index))?;
        Some(block_subject(&lines, block))
    });
    from_evidence
        .or_else(|| {
            let mut ordered: Vec<&LogSource> = sources.iter().collect();
            ordered.sort_by_key(|source| source.kind());
            ordered.into_iter().find_map(|source| {
                if source.kind() == SourceKind::JvmCrash {
                    return None;
                }
                let lines = source.clean_lines();
                let block = crash_blocks(&lines, source.kind() == SourceKind::CrashReport)
                    .into_iter()
                    .next()?;
                Some(block_subject(&lines, block))
            })
        })
        .unwrap_or_default()
}

/// A descrição de um bloco de crash report e, quando uma entidade ou um bloco quebrou ao ser
/// atualizado, o tipo e a classe dele.
fn block_subject(lines: &[CleanLine], block: Range<usize>) -> Vec<String> {
    let mut parts = Vec::new();
    let Some((at, description)) = block.clone().find_map(|at| {
        DESCRIPTION
            .captures(&lines[at].text)
            .and_then(|captures| captures.name("text"))
            .map(|text| (at, text.as_str().to_owned()))
    }) else {
        return parts;
    };
    parts.push(format!(
        "description={}",
        normalize_param("description", &description)
    ));
    let Some(ticking) = TICKING.captures(description.trim()) else {
        return parts;
    };
    let (pattern, kind, class) = if ticking.name("block").is_some() {
        (&*BLOCK_ENTITY, "block_entity", "block_class")
    } else {
        (&*ENTITY_TYPE, "entity_type", "entity_class")
    };
    if let Some(captures) = (at + 1..block.end).find_map(|line| pattern.captures(&lines[line].text))
    {
        if let Some(value) = captures.name("kind") {
            parts.push(format!("{kind}={}", normalize_param(kind, value.as_str())));
        }
        if let Some(value) = captures.name("class") {
            parts.push(format!(
                "{class}={}",
                normalize_param(class, value.as_str())
            ));
        }
    }
    parts
}

/// O frame que diz onde o travamento aconteceu e, quando o nome dele não traz o mod, as configs
/// de mixin anotadas nele.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Place {
    /// Classe e método normalizados.
    frame: Option<String>,
    /// Configs de mixin do frame (sem repetição, em ordem).
    mixins: Vec<String>,
    /// O que a pilha diz do sujeito além do frame: o ponto da queda quando o jogo quebrou acima
    /// do frame de mod (`where=net.minecraft….GameRenderer.renderLevel`) e a config (ou o alvo)
    /// de uma falha de mixin que o catálogo não identificou (`mixin_config=x.mixins.json`).
    extra: Vec<String>,
}

impl Place {
    fn of(frame: Option<String>) -> Self {
        Self {
            frame,
            mixins: Vec::new(),
            extra: Vec::new(),
        }
    }

    /// Acrescenta as configs ao sujeito da assinatura (`frame_mixin=create.mixins.json`) e o
    /// sujeito da falha de mixin, quando o catálogo não trouxe a config.
    fn extend_subject(&self, subject: &mut Vec<String>) {
        for config in &self.mixins {
            let part = format!("frame_mixin={config}");
            if !subject.contains(&part) {
                subject.push(part);
            }
        }
        let has_config = subject.iter().any(|part| part.starts_with("mixin_config="));
        for part in &self.extra {
            if has_config && part.starts_with("mixin_") {
                continue;
            }
            if !subject.contains(part) {
                subject.push(part.clone());
            }
        }
    }
}

/// O alvo de uma falha de mixin (`Mixin transformation of net.minecraft.class_1754 failed`).
static MIXIN_TARGET: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"Mixin transformation of (?P<target>[\w.$/]+) failed"));

/// A config de mixin numa mensagem de falha: `Mixin [x.mixins.json:Classe from mod m]`
/// (`MixinApplyError`), `in x.mixins.json:Classe` (`InjectionError`), `failed x.mixins.json:Classe
/// -> alvo` (`Mixin apply for mod m failed …`).
static MIXIN_CONFIG_IN_MESSAGE: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"(?:Mixin \[|\bin |failed )(?P<config>[\w.\-]+\.json):[\w.$/]+"));

/// Pacote das exceções do Mixin: a pilha delas é a do transformador, não diz de que mod é.
const MIXIN_EXCEPTIONS: &str = "org.spongepowered.asm.mixin.";

/// O sujeito de uma falha de mixin lida das mensagens da cadeia de exceções (a linha de cima e
/// cada `Caused by:`): a config, se alguma mensagem a traz; senão a classe que estava sendo
/// transformada. Só quando a causa mais profunda é uma exceção do Mixin.
fn mixin_subject(lines: &[CleanLine], stack: Range<usize>, exception: Option<&str>) -> Vec<String> {
    if !exception.is_some_and(|kind| kind.starts_with(MIXIN_EXCEPTIONS)) {
        return Vec::new();
    }
    let messages: Vec<&str> = stack
        .filter(|&at| EXCEPTION_LINE.is_match(&lines[at].text))
        .map(|at| lines[at].text.as_str())
        .collect();
    if let Some(config) = messages.iter().find_map(|text| {
        MIXIN_CONFIG_IN_MESSAGE
            .captures(text)
            .map(|captures| captures["config"].to_lowercase())
    }) {
        return vec![format!("mixin_config={config}")];
    }
    messages
        .iter()
        .find_map(|text| {
            MIXIN_TARGET
                .captures(text)
                .map(|captures| format!("mixin_target={}", captures["target"].replace('/', ".")))
        })
        .into_iter()
        .collect()
}

/// Exceção e frame da pilha que começa na linha `number` (ou logo depois).
fn stack_at(
    source: &LogSource,
    number: u32,
    fabric: bool,
    where_matters: bool,
) -> Option<(Option<String>, Place)> {
    let lines = source.clean_lines();
    let index = lines.iter().position(|line| line.number == number)?;
    if source.kind() == SourceKind::JvmCrash {
        return Some((None, Place::of(native_frame(&lines, index))));
    }
    Some(exception_and_frame(&lines, index, fabric, where_matters))
}

/// Sem achado: a exceção e o frame do primeiro crash report (os arquivos de crash report vêm
/// antes dos logs, como na análise). Sem achado, onde quebrou importa.
fn first_crash_block(sources: &[LogSource], fabric: bool) -> Option<(Option<String>, Place)> {
    let mut ordered: Vec<&LogSource> = sources.iter().collect();
    ordered.sort_by_key(|source| source.kind());
    ordered.into_iter().find_map(|source| {
        let lines = source.clean_lines();
        if source.kind() == SourceKind::JvmCrash {
            let index = lines
                .iter()
                .position(|line| line.text.contains("Problematic frame"))?;
            return Some((None, Place::of(native_frame(&lines, index))));
        }
        let block = crash_blocks(&lines, source.kind() == SourceKind::CrashReport)
            .into_iter()
            .next()?;
        Some(exception_and_frame(&lines, block.start, fabric, true))
    })
}

/// A exceção depois de `index` (sem passar para outro registro do log) e o frame da pilha dela.
fn exception_and_frame(
    lines: &[CleanLine],
    index: usize,
    fabric: bool,
    where_matters: bool,
) -> (Option<String>, Place) {
    let end = lines.len().min(index + EXCEPTION_WITHIN);
    let start = (index..end).find(|&at| {
        (at == index || !lines[at].record_start) && EXCEPTION_LINE.is_match(&lines[at].text)
    });
    let Some(start) = start else {
        return (None, Place::default());
    };
    let stack = whole_stack(lines, start);
    let exception = deepest_cause(lines, stack.clone())
        .as_deref()
        .and_then(exception_type)
        .or_else(|| exception_type(&lines[start].text));
    let overflow = exception.as_deref() == Some(STACK_OVERFLOW);
    let extra = mixin_subject(lines, stack.clone(), exception.as_deref());
    // Exceção sem nenhum frame (a JVM omite a pilha das exceções repetidas): a pilha da thread
    // está na seção `-- Head --` do crash report, logo abaixo.
    let stack = if has_frames(lines, stack.clone()) {
        stack
    } else {
        head_stack(lines, stack.end).unwrap_or(stack)
    };
    let mut place = stack_place(lines, stack, fabric, where_matters, overflow);
    place.extra.extend(extra);
    (exception, place)
}

/// Se alguma linha da faixa é um frame Java.
fn has_frames(lines: &[CleanLine], range: Range<usize>) -> bool {
    range
        .into_iter()
        .any(|at| JAVA_FRAME.is_match(&lines[at].text))
}

/// A pilha da primeira seção `Stacktrace:` depois de `from` (o `-- Head --` do crash report),
/// sem passar para outro registro do log.
fn head_stack(lines: &[CleanLine], from: usize) -> Option<Range<usize>> {
    let end = lines.len().min(from + EXCEPTION_WITHIN);
    let header = (from..end)
        .take_while(|&at| at == from || !lines[at].record_start)
        .find(|&at| lines[at].text.trim() == "Stacktrace:")?;
    let stack = stack_range(lines, header);
    has_frames(lines, stack.clone()).then_some(stack)
}

/// A pilha que começa em `start`, seguindo depois de um trecho omitido (`[...]`): no crash
/// report de um `StackOverflowError` o Forge mostra o topo e o fim da pilha.
fn whole_stack(lines: &[CleanLine], start: usize) -> Range<usize> {
    let mut stack = stack_range(lines, start);
    while let Some(line) = lines.get(stack.end)
        && ELIDED.is_match(&line.text)
        && !line.record_start
    {
        let rest = stack_range(lines, stack.end);
        if rest.end <= stack.end + 1 {
            break;
        }
        stack.end = rest.end;
    }
    stack
}

/// O tipo de exceção de uma linha.
fn exception_type(text: &str) -> Option<String> {
    EXCEPTION_LINE
        .captures(text)
        .and_then(|captures| captures.name("type"))
        .map(|kind| kind.as_str().to_owned())
}

/// Um frame Java da pilha, já lido.
struct Frame {
    /// Classe e método normalizados.
    method: String,
    /// Se é frame de mod: cita um mod ou não é de um pacote do jogo, do loader ou de biblioteca.
    of_mod: bool,
    /// Se o método foi injetado pelo Mixin (`handler$…`, `redirect$…`, `…$md$…`).
    injected: bool,
    /// Se o nome ou a anotação do frame já diz de que mod ele é.
    names_mod: bool,
    /// Configs de mixin anotadas no frame.
    mixins: Vec<String>,
}

/// Onde o travamento aconteceu, nesta ordem: o primeiro frame de mod (método injetado pelo
/// Mixin incluído), procurado na seção da causa mais profunda (o último `Caused by:`) e depois
/// nas seções de cima; e, só quando `where_matters`, o **frame da queda**: o primeiro frame da
/// causa mais profunda que não é do Java nem de uma lambda gerada, com as configs de mixin
/// anotadas nele. Configs anotadas em frames de baixo (`Minecraft.runTick` quase sempre tem) não
/// escolhem o frame nem entram na assinatura. Na seção de um `StackOverflowError` (`overflow`: é
/// a causa mais profunda) o topo da pilha cai num ponto qualquer da recursão: ali vale o menor
/// frame de cada nível, que é o mesmo de uma vez para outra.
fn stack_place(
    lines: &[CleanLine],
    stack: Range<usize>,
    fabric: bool,
    where_matters: bool,
    overflow: bool,
) -> Place {
    // As seções da cadeia principal: a exceção de cima e cada `Caused by:` dela. As exceções
    // suprimidas (`Suppressed:`, com os frames e o `... N more` delas indentados um nível a
    // mais) ficam de fora: são outro erro, não a causa.
    let main_lines: Vec<usize> = main_chain(lines, stack.clone());
    let mut starts: Vec<usize> = main_lines
        .iter()
        .copied()
        .filter(|&at| lines[at].text.trim_start().starts_with("Caused by"))
        .collect();
    starts.insert(0, stack.start);
    let mut sections: Vec<Vec<Frame>> = Vec::with_capacity(starts.len());
    // Quantos frames do fim de cada seção são os da seção de cima (`... 3 more`).
    let mut shared: Vec<usize> = Vec::with_capacity(starts.len());
    for (position, &start) in starts.iter().enumerate() {
        let end = starts.get(position + 1).copied().unwrap_or(stack.end);
        let section: Vec<usize> = main_lines
            .iter()
            .copied()
            .filter(|at| (start..end).contains(at))
            .collect();
        // Depois do `main` do jogo só vem o launcher.
        let mut frames = Vec::new();
        for frame in section
            .iter()
            .filter_map(|&at| java_frame(lines, at, fabric))
        {
            let game_main = GAME_MAIN.contains(&frame.method.as_str());
            frames.push(frame);
            if game_main {
                break;
            }
        }
        sections.push(frames);
        shared.push(
            section
                .iter()
                .find_map(|&at| MORE.captures(&lines[at].text))
                .and_then(|captures| captures["count"].parse().ok())
                .unwrap_or(0),
        );
    }
    // A pilha inteira de cada seção: os frames dela e, no lugar do `... N more`, os N últimos da
    // seção de cima (já inteira).
    let mut whole: Vec<Vec<&Frame>> = Vec::with_capacity(sections.len());
    for (position, section) in sections.iter().enumerate() {
        let mut frames: Vec<&Frame> = section.iter().collect();
        if let Some(above) = position.checked_sub(1).map(|above| &whole[above]) {
            frames.extend_from_slice(&above[above.len().saturating_sub(shared[position])..]);
        }
        whole.push(frames);
    }
    // Da causa mais profunda para cima.
    let mut own: Vec<Vec<&Frame>> = sections
        .iter()
        .map(|section| section.iter().collect())
        .collect();
    own.reverse();
    whole.reverse();
    // O frame da queda, na pilha inteira da causa mais profunda (uma causa só com `... N more`
    // tem os frames da seção de cima). Bibliotecas (guava, fastutil, joml…), o Java e os
    // launchers não dizem onde o jogo quebrou: vale o primeiro frame do jogo, do loader ou de
    // mod; sem nenhum, o primeiro fora do Java; sem nenhum, o primeiro que houver.
    let fallen = |accept: &dyn Fn(&Frame) -> bool| first_accepted(&whole, accept, overflow);
    let not_generated = |frame: &Frame| !frame.method.contains("$$Lambda");
    // As classes que só conferem e lançam (`Preconditions.checkState`) também não dizem onde
    // quebrou, nem quando só sobram frames de biblioteca.
    let throw_helper = |frame: &Frame| {
        THROW_HELPERS
            .iter()
            .any(|prefix| frame.method.starts_with(prefix))
    };
    let outside_libraries = |frame: &Frame| {
        not_generated(frame) && !throw_helper(frame) && !is_library_package(&frame.method)
    };
    let outside_java = |frame: &Frame| {
        not_generated(frame)
            && !throw_helper(frame)
            && !JAVA_PACKAGES
                .iter()
                .any(|prefix| frame.method.starts_with(prefix))
    };
    let fall = fallen(&outside_libraries)
        .or_else(|| fallen(&outside_java))
        .or_else(|| fallen(&|_| true));
    let pick = |accept: &dyn Fn(&Frame) -> bool| first_accepted(&own, accept, overflow);
    if let Some(frame) = pick(&|frame| frame.of_mod || frame.injected) {
        // Método injetado sem o mod no nome (Forge): as configs anotadas dizem de quem pode ser.
        let mixins = if frame.names_mod {
            Vec::new()
        } else {
            frame.mixins.clone()
        };
        // Quando o jogo quebrou acima do frame de mod (o mod chamou o jogo, ou embrulha um
        // trecho inteiro dele, como o `renderLevel` do Iris ou o `managedBlock` do servidor), o
        // ponto da queda também identifica o travamento: senão quedas diferentes dentro do mesmo
        // embrulho teriam a mesma assinatura.
        let extra = match fall {
            Some(fall) if where_matters && fall.method != frame.method => {
                vec![format!("where={}", fall.method)]
            }
            _ => Vec::new(),
        };
        return Place {
            frame: Some(frame.method.clone()),
            mixins,
            extra,
        };
    }
    if !where_matters {
        return Place::default();
    }
    match fall {
        Some(frame) => Place {
            frame: Some(frame.method.clone()),
            mixins: frame.mixins.clone(),
            extra: Vec::new(),
        },
        None => Place::default(),
    }
}

/// As linhas da cadeia principal da pilha: tudo menos as exceções suprimidas (`Suppressed:`) e
/// o que vem indentado dentro delas (frames, `... N more` e as causas delas).
fn main_chain(lines: &[CleanLine], stack: Range<usize>) -> Vec<usize> {
    let mut inside: Option<usize> = None;
    stack
        .filter(|&at| {
            let text = &lines[at].text;
            let indent = text.len() - text.trim_start().len();
            let content = text.trim_start();
            if let Some(level) = inside {
                // Dentro da suprimida: os frames dela (mais fundo) e a causa dela (`Caused by:`
                // no mesmo nível).
                if indent > level || (indent == level && content.starts_with("Caused by")) {
                    return false;
                }
                inside = None;
            }
            if content.starts_with("Suppressed:") {
                inside = Some(indent);
                return false;
            }
            true
        })
        .collect()
}

/// O primeiro frame aceito da primeira lista que tiver um (da causa mais profunda para cima). Na
/// seção de um `StackOverflowError` (`overflow`, a primeira lista), o menor frame aceito.
fn first_accepted<'a>(
    lists: &[Vec<&'a Frame>],
    accept: &dyn Fn(&Frame) -> bool,
    overflow: bool,
) -> Option<&'a Frame> {
    lists.iter().enumerate().find_map(|(position, frames)| {
        let mut accepted = frames.iter().copied().filter(|frame| accept(frame));
        if overflow && position == 0 {
            accepted.min_by(|one, two| one.method.cmp(&two.method))
        } else {
            accepted.next()
        }
    })
}

/// O frame Java da linha `at`, lido: método normalizado, se é de mod, se foi injetado pelo Mixin
/// e as configs de mixin anotadas.
fn java_frame(lines: &[CleanLine], at: usize, fabric: bool) -> Option<Frame> {
    let text = HIDDEN_CLASS.replace_all(&lines[at].text, "$method");
    let text = LAMBDA_CLASS.replace_all(&text, "$$$$Lambda");
    let captures = JAVA_FRAME.captures(&text)?;
    let method = captures.name("method")?.as_str();
    let rest = captures.name("rest").map_or("", |rest| rest.as_str());
    let annotated = text.contains(".jar") || text.contains('@');
    let mentions = !mods_in(lines, at..at + 1, fabric).is_empty();
    // O pacote decide: uma biblioteca com o jar anotado (`~[fastutil-8.5.9.jar…]`) não é mod,
    // e um mod anotado com um jar que não é do jogo (`[meumod-1.0.jar:?]`, Forge 1.12) é.
    let library = is_not_mod_package(method);
    let cites_mod_jar = ANNOTATED_JAR
        .captures(rest)
        .is_some_and(|captures| !is_library_jar(&captures["jar"]));
    let mut mixins = BTreeSet::new();
    let mut owners = BTreeSet::new();
    for captures in FRAME_MIXIN_CONFIG.captures_iter(rest) {
        if let Some(config) = captures.name("config") {
            mixins.insert(config.as_str().to_lowercase());
        }
        if let Some(owner) = captures.name("mod") {
            owners.insert(owner.as_str().to_lowercase());
        }
    }
    let name = method.rsplit('.').next().unwrap_or(method);
    // Métodos injetados (`handler$…`) e os fundidos pelo Mixin (`md1a2b3c$…`, `…$md$…`: as
    // lambdas e os `@Unique` de um mixin) são código de mod dentro da classe do jogo.
    let injected = MIXIN_MERGED.is_match(name)
        || MIXIN_UNIQUE.is_match(name)
        || name
            .split_once('$')
            .is_some_and(|(prefix, _)| MIXIN_PREFIXES.contains(&prefix));
    let method = normalize_method(method);
    // NeoForge: o handler traz o mod (`handler$zza000$meumod$aoTick`) e a anotação diz de que mod
    // é cada config (`… from mod meumod`).
    let owner_in_name = injected
        && method
            .rsplit('.')
            .next()
            .and_then(|name| name.split_once('$'))
            .is_some_and(|(owner, _)| owners.contains(&owner.to_lowercase()));
    Some(Frame {
        method,
        of_mod: !library && (mentions || !annotated || cites_mod_jar),
        injected,
        names_mod: mentions || !library || owner_in_name,
        mixins: mixins.into_iter().collect(),
    })
}

/// Classe e método sem as partes que mudam a cada execução: o número das lambdas e das classes
/// geradas, e o prefixo e o id dos métodos injetados pelo Mixin (o mod, quando o nome traz, e o
/// método ficam: `Classe.handler$zza00a$meumod$aoTick` → `Classe.meumod$aoTick`).
fn normalize_method(method: &str) -> String {
    let method = normalize_class(method);
    let method = MIXIN_MERGED.replace_all(&method, "$$");
    let (owner, name) = match method.rsplit_once('.') {
        Some((owner, name)) => (Some(owner), name),
        None => (None, method.as_ref()),
    };
    let name = match name.split_once('$') {
        Some((prefix, rest)) if MIXIN_PREFIXES.contains(&prefix) => without_mixin_id(rest),
        _ => MIXIN_UNIQUE.replace(name, "$kind$$").into_owned(),
    };
    let name = TRAILING_COUNTER.replace(&name, "").into_owned();
    match owner {
        Some(owner) => format!("{owner}.{name}"),
        None => name,
    }
}

/// O nome de um método injetado depois do prefixo, sem o id. O id está numa posição fixa do nome
/// gerado: logo depois do prefixo no Mixin 0.8 e no fork do Fabric (`zza00a$meumod$aoTick`) e no
/// fim no Mixin 0.7 (`onUpdate$zza000`, do Forge 1.12). Só sai quando tem o formato do id e
/// sobra um nome; senão o resto fica (`meumod$aoTick`).
fn without_mixin_id(rest: &str) -> String {
    if let Some((id, after)) = rest.split_once('$')
        && is_mixin_id(id)
        && !after.is_empty()
    {
        return after.to_owned();
    }
    if let Some((before, id)) = rest.rsplit_once('$')
        && is_mixin_id(id)
        && !before.is_empty()
    {
        return before.to_owned();
    }
    rest.to_owned()
}

/// Se o trecho tem o formato do id de um método injetado (três letras e um número hexadecimal,
/// com ou sem algarismo: `zza000`, `zzaabc`).
fn is_mixin_id(segment: &str) -> bool {
    MIXIN_ID.is_match(segment)
}

/// Classe (ou classe e método) sem os números de lambdas, acessores e classes geradas.
fn normalize_class(text: &str) -> String {
    let text = LAMBDA_CLASS.replace_all(text, "$$$$Lambda");
    let text = LAMBDA_METHOD.replace_all(&text, "$1$2");
    GENERATED_CLASS.replace_all(&text, "$1$2$3").into_owned()
}

/// O frame problemático de um `hs_err`: a partir de `index`, a linha depois de
/// `# Problematic frame:` (ou a própria linha da evidência), sem endereços.
fn native_frame(lines: &[CleanLine], index: usize) -> Option<String> {
    let header = (index.saturating_sub(1)..lines.len().min(index + 20))
        .find(|&at| lines[at].text.contains("Problematic frame"));
    let at = header.map_or(index, |header| header + 1);
    let text = lines.get(at)?.text.trim_start_matches('#').trim();
    let captures = NATIVE_FRAME.captures(text)?;
    let kind = captures.name("kind")?.as_str();
    let place = captures
        .name("module")
        .or_else(|| captures.name("method"))?
        .as_str()
        .trim();
    let place = HEX_ADDRESS.replace_all(place, "#");
    Some(format!("{kind} {}", normalize_method(&place)))
}

/// Um mod citado, normalizado.
fn normalize_item(item: &ItemRef) -> String {
    match item {
        ItemRef::ModId { id } => format!("mod:{}", id.trim().to_lowercase().replace('-', "_")),
        ItemRef::JarFile { name } => format!("jar:{}", jar_stem(name)),
        ItemRef::Metafile { path } => format!("item:{}", path.trim()),
    }
}

/// O nome do jar sem pastas, extensão e versão (`create-1.20.1-0.5.1.f.jar` → `create`).
fn jar_stem(name: &str) -> String {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let lower = file.trim().to_lowercase();
    let base = lower.strip_suffix(".jar").unwrap_or(&lower);
    let base = base.split("%23").next().unwrap_or(base);
    let stem = JAR_VERSION.replace(base, "");
    if stem.is_empty() {
        base.to_owned()
    } else {
        stem.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::postcrash::{AnalysisContext, analyze};

    fn source(name: &str, kind: SourceKind, text: &str) -> LogSource {
        LogSource::from_text(name, kind, text)
    }

    fn of(sources: &[LogSource]) -> CrashSignature {
        let analysis = analyze(sources, &AnalysisContext::default(), None).unwrap();
        signature(&analysis, sources)
    }

    // ---- Corpus de logs reais (`tests/corpus/`, da D-02) ------------------------------------

    fn corpus_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("corpus")
    }

    fn kind_of(relative: &str) -> SourceKind {
        let name = relative.rsplit('/').next().unwrap_or(relative);
        if relative.starts_with("crash-reports/") {
            SourceKind::CrashReport
        } else if name.starts_with("hs_err_pid") {
            SourceKind::JvmCrash
        } else if name == "output.log" {
            SourceKind::Output
        } else if name == "latest.log" {
            SourceKind::LatestLog
        } else if name == "debug.log" || name.starts_with("fml-") {
            SourceKind::DebugLog
        } else {
            SourceKind::Text
        }
    }

    /// Os arquivos de um caso: (nome relativo, bytes).
    fn files_of(case: &Path) -> Vec<(String, Vec<u8>)> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, base, out);
                    continue;
                }
                let relative = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                if relative != "esperado.toml" && relative != ".gitattributes" {
                    out.push((relative, fs::read(&path).unwrap()));
                }
            }
        }
        let mut files = Vec::new();
        walk(case, case, &mut files);
        files.sort();
        files
    }

    fn sources_from(files: &[(String, Vec<u8>)]) -> Vec<LogSource> {
        files
            .iter()
            .map(|(name, bytes)| LogSource::from_bytes(name.clone(), kind_of(name), bytes))
            .collect()
    }

    fn corpus_cases() -> Vec<PathBuf> {
        let mut cases: Vec<PathBuf> = fs::read_dir(corpus_dir())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect();
        cases.sort();
        cases
    }

    /// O mesmo travamento outra vez: números de linha das pilhas, índices de jar, endereços,
    /// coordenadas, horários, UUIDs, lambdas e prefixos de mixin diferentes, e o log com linhas
    /// a mais antes do travamento (a evidência cai em outra linha do arquivo).
    fn another_run(name: &str, text: &str) -> String {
        static JAVA_LINE: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"\.java:(\d+)\)"));
        static JAR_INDEX: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"%23(\d+)"));
        static HEX: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"0x[0-9a-fA-F]+"));
        static LAMBDA: LazyLock<Regex> =
            LazyLock::new(|| fixed_regex(r"(\$\$Lambda\$|lambda\$\w*?\$)(\d+)"));
        static MIXIN: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"\$[a-z]{3}[0-9a-f]{3,}\$"));
        static MIXIN_SESSION: LazyLock<Regex> =
            LazyLock::new(|| fixed_regex(r"\b(md|fd)[0-9a-f]{6}\$"));
        static COUNTER: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"\$(\d+)\("));
        static JAVA8_LAMBDA: LazyLock<Regex> =
            LazyLock::new(|| fixed_regex(r"(\$\$Lambda\$\d+/)\d+"));
        static UUID: LazyLock<Regex> = LazyLock::new(|| {
            fixed_regex(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
        });
        static CLOCK: LazyLock<Regex> =
            LazyLock::new(|| fixed_regex(r"\b\d{2}:\d{2}:\d{2}(?:\.\d+)?\b"));
        static NUMBER: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"-?\d+(?:\.\d+)?"));

        let bump = |captures: &regex::Captures<'_>, add: u64| {
            captures[1].parse::<u64>().unwrap_or(0) + add
        };
        let text = JAVA_LINE.replace_all(text, |c: &regex::Captures<'_>| {
            format!(".java:{})", bump(c, 17))
        });
        let text = JAR_INDEX.replace_all(&text, |c: &regex::Captures<'_>| {
            format!("%23{}", bump(c, 5))
        });
        let text = HEX.replace_all(&text, "0x7ffd3c2a10");
        let text = LAMBDA.replace_all(&text, |c: &regex::Captures<'_>| {
            format!("{}{}", &c[1], c[2].parse::<u64>().unwrap_or(0) + 3)
        });
        // Outra sessão do Mixin (4ª revisão): os métodos fundidos (`md6558fa$…`) mudam de nome.
        let text = MIXIN_SESSION.replace_all(&text, "${1}f0f0f0$$");
        // Outros contadores do `MixinExtras` e do Java 8 (`$$Lambda$5/1234567890`).
        let text = COUNTER.replace_all(&text, |c: &regex::Captures<'_>| {
            format!("${}(", c[1].parse::<u64>().unwrap_or(0) + 7)
        });
        let text = JAVA8_LAMBDA.replace_all(&text, "${1}2020452718");
        // Id sem algarismo: o id vale pela posição (2ª revisão).
        let text = MIXIN.replace_all(&text, "$$qqxfed$$");
        let text = UUID.replace_all(&text, "0f0e0d0c-0b0a-0908-0706-050403020100");
        let text = CLOCK.replace_all(&text, "23:59:58");
        let mut out = String::new();
        if matches!(kind_of(name), SourceKind::Output | SourceKind::LatestLog) {
            if text.contains("<log4j:Event") {
                out.push_str(concat!(
                    "<log4j:Event logger=\"x\" timestamp=\"1\" level=\"INFO\" thread=\"main\">\n",
                    "  <log4j:Message><![CDATA[linha a mais]]></log4j:Message>\n",
                    "</log4j:Event>\n",
                ));
            } else {
                out.push_str(
                    "[00:00:01] [main/INFO]: linha a mais\n[00:00:02] [main/INFO]: outra\n",
                );
            }
        }
        for line in text.lines() {
            let lower = line.to_lowercase();
            if lower.contains("location") || lower.contains("time:") || lower.contains("pos=") {
                out.push_str(&NUMBER.replace_all(line, "42.5"));
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn ca_t14_08_corpus_o_mesmo_travamento_com_numeros_diferentes_tem_a_mesma_assinatura() {
        let mut checked = 0;
        let mut with_frame = 0;
        for case in corpus_cases() {
            let files = files_of(&case);
            let original = of(&sources_from(&files));
            if original.rule.is_none() {
                continue;
            }
            let changed: Vec<(String, Vec<u8>)> = files
                .iter()
                .map(|(name, bytes)| {
                    let text = String::from_utf8_lossy(bytes);
                    (name.clone(), another_run(name, &text).into_bytes())
                })
                .collect();
            let again = of(&sources_from(&changed));
            assert_eq!(original, again, "{}", case.display());
            checked += 1;
            with_frame += usize::from(original.frame.is_some());
        }
        // O corpus tem dezenas de travamentos; muitos com frame de mod na pilha.
        assert!(checked >= 25, "casos conferidos: {checked}");
        assert!(with_frame >= 5, "casos com frame de mod: {with_frame}");
    }

    #[test]
    fn ca_t14_08_corpus_causas_diferentes_tem_assinaturas_diferentes() {
        let mut seen: std::collections::HashMap<String, (String, CrashSignature)> =
            std::collections::HashMap::new();
        for case in corpus_cases() {
            let name = case.file_name().unwrap().to_string_lossy().into_owned();
            let signature = of(&sources_from(&files_of(&case)));
            if signature.rule.is_none() {
                continue;
            }
            if let Some((other, previous)) = seen.get(&signature.hash) {
                // Só pode coincidir quando as partes são as mesmas (mesma causa em logs
                // diferentes), nunca por colisão.
                assert_eq!(
                    (
                        &previous.rule,
                        &previous.exception,
                        &previous.frame,
                        &previous.mods,
                        &previous.subject
                    ),
                    (
                        &signature.rule,
                        &signature.exception,
                        &signature.frame,
                        &signature.mods,
                        &signature.subject
                    ),
                    "{other} × {name}"
                );
            }
            seen.insert(signature.hash.clone(), (name, signature));
        }
        // Os dois "Ticking entity" do corpus são de mods diferentes.
        let ticking: Vec<&CrashSignature> = seen
            .values()
            .map(|(_, signature)| signature)
            .filter(|signature| signature.rule.as_deref() == Some("crash_report.ticking_entity"))
            .collect();
        assert!(ticking.len() >= 2, "{ticking:?}");
    }

    #[test]
    fn corpus_partes_da_assinatura_em_casos_conhecidos() {
        let case = |name: &str| of(&sources_from(&files_of(&corpus_dir().join(name))));

        let ticking = case("codex-forge-ticking-entity-1-18-2");
        assert_eq!(ticking.rule.as_deref(), Some("crash_report.ticking_entity"));
        assert_eq!(ticking.mods, ["jar:village-employment"]);
        assert!(ticking.exception.is_some());

        let native = case("github-hs-err-driver-intel");
        assert_eq!(
            native.rule.as_deref(),
            Some("graphics.native_frame_in_driver")
        );
        assert_eq!(native.frame.as_deref(), Some("C ig9icd64.dll"));

        // Formas de pilha da 2ª revisão (na ponta 199efdc, todas sem frame).
        // Forge 1.7.10: método injetado com o mod no nome, sem ser Fabric.
        assert_eq!(
            case("codex-forge-1-7-10-crash-report").frame.as_deref(),
            Some("net.minecraft.world.chunk.Chunk.hodgepodge$validateTileInBounds")
        );
        // Forge 1.16: `StackOverflowError` com o meio da pilha omitido (`[...]`).
        assert_eq!(
            case("codex-forge-ticking-block-entity-crash-report")
                .frame
                .as_deref(),
            Some("com.supermartijn642.tesseract.capabilities.CombinedEnergyStorage.receiveEnergy")
        );
        // `latest.log` com a causa mais profunda sem frames: o primeiro da seção de cima.
        assert_eq!(
            case("codex-forge-ticking-block-entity").frame.as_deref(),
            Some("net.minecraft.server.MinecraftServer.func_71190_q")
        );
        // Fabric e Quilt: o crash de depuração (F3 + C) diz onde foi pedido.
        assert_eq!(
            case("codex-fabric-crash-report-client").frame.as_deref(),
            Some("net.minecraft.class_309.method_1474")
        );
    }

    // ---- Normalização --------------------------------------------------------------------

    const NPE_REPORT: &str = "---- Minecraft Crash Report ----
// Who set us up the TNT?

Time: 2026-10-13 12:00:01
Description: Ticking entity

java.lang.NullPointerException: Cannot invoke \"Object.hashCode()\" because \"<local3>\" is null
\tat com.example.meumod.EntityTicker.lambda$tick$12(EntityTicker.java:41) ~[meumod-1.0.jar%23101!/:?]
\tat net.minecraft.world.level.Level.guardEntityTick(Level.java:501) ~[client-1.20.1-20230612.114412-srg.jar%23185!/:?]

-- Entity being ticked --
Details:
\tEntity Type: meumod:golem (com.example.meumod.Golem)
\tEntity's Exact location: 13.37, 64.00, -7.10
";

    #[test]
    fn numeros_mudam_assinatura_nao() {
        let first = of(&[source(
            "crash-reports/crash-1.txt",
            SourceKind::CrashReport,
            NPE_REPORT,
        )]);
        let second_text = NPE_REPORT
            .replace("EntityTicker.java:41", "EntityTicker.java:97")
            .replace("lambda$tick$12", "lambda$tick$3")
            .replace("%23101", "%2377")
            .replace("13.37, 64.00, -7.10", "-512.00, 70.00, 1024.50")
            .replace("12:00:01", "18:44:09");
        let second = of(&[source(
            "crash-reports/crash-2.txt",
            SourceKind::CrashReport,
            &second_text,
        )]);
        assert_eq!(first, second);
        assert_eq!(first.rule.as_deref(), Some("crash_report.ticking_entity"));
        assert_eq!(
            first.exception.as_deref(),
            Some("java.lang.NullPointerException")
        );
        assert_eq!(
            first.frame.as_deref(),
            Some("com.example.meumod.EntityTicker.lambda$tick")
        );
        assert_eq!(first.mods, ["jar:meumod"]);
        assert_eq!(first.hash.len(), 16);
        assert_eq!(first.version, SIGNATURE_VERSION);

        // Outro mod na pilha: outra assinatura.
        let other = NPE_REPORT.replace("com.example.meumod.EntityTicker", "net.outro.Ticker");
        let other = of(&[source(
            "crash-reports/crash-3.txt",
            SourceKind::CrashReport,
            &other,
        )]);
        assert_ne!(first.hash, other.hash);
    }

    // ---- Sujeito do achado (falsos agrupamentos da 1ª revisão) ---------------------------

    /// Crash report com `NullPointerException` e pilha só do jogo.
    fn game_only_report(description: &str, details: &str) -> String {
        format!(
            "---- Minecraft Crash Report ----
// Oops.

Time: 2026-10-13 12:00:01
Description: {description}

java.lang.NullPointerException: Cannot invoke \"Object.hashCode()\" because \"<local3>\" is null
\tat net.minecraft.world.level.Level.guardEntityTick(Level.java:501) ~[client-1.20.1-srg.jar%23185!/:?]
\tat net.minecraft.server.MinecraftServer.tickServer(MinecraftServer.java:880) ~[client-1.20.1-srg.jar%23185!/:?]

{details}
-- System Details --
Details:
\tMinecraft Version: 1.20.1
"
        )
    }

    fn report_signature(name: &str, text: &str) -> CrashSignature {
        of(&[source(
            &format!("crash-reports/{name}.txt"),
            SourceKind::CrashReport,
            text,
        )])
    }

    #[test]
    fn descricoes_diferentes_com_pilha_so_do_jogo_tem_assinaturas_diferentes() {
        let overlay = report_signature(
            "a",
            &game_only_report("Rendering overlay", "-- Overlay render details --\n"),
        );
        let tick = report_signature(
            "b",
            &game_only_report("Exception in server tick loop", "-- Server --\n"),
        );
        // Na ponta da 1ª revisão as duas davam 6fa9d7e2a146d61a.
        assert_eq!(overlay.rule.as_deref(), Some("crash_report.description"));
        assert_eq!(tick.rule.as_deref(), Some("crash_report.description"));
        // Sem frame de mod: o primeiro frame da pilha (3ª revisão).
        assert_eq!(
            overlay.frame.as_deref(),
            Some("net.minecraft.world.level.Level.guardEntityTick")
        );
        assert!(overlay.mods.is_empty());
        assert_ne!(overlay.hash, tick.hash);
        assert_eq!(overlay.subject, ["description=rendering overlay"]);
        assert_eq!(tick.subject, ["description=exception in server tick loop"]);

        // A mesma descrição com números diferentes continua junta.
        let again = report_signature(
            "c",
            &game_only_report("Rendering overlay", "-- Overlay render details --\n")
                .replace("Level.java:501", "Level.java:530")
                .replace("%23185", "%23190")
                .replace("12:00:01", "21:13:09"),
        );
        assert_eq!(overlay, again);
    }

    fn ticking_entity(kind: &str, class: &str, location: &str) -> String {
        game_only_report(
            "Ticking entity",
            &format!(
                "-- Entity being ticked --
Details:
\tEntity Type: {kind} ({class})
\tEntity ID: 4817
\tEntity Name: {kind}
\tEntity's Exact location: {location}
\tEntity's Block location: World: (13,64,-8), Section: (at 13,0,8 in 0,4,-1)
"
            ),
        )
    }

    #[test]
    fn entidades_diferentes_com_pilha_so_do_jogo_tem_assinaturas_diferentes() {
        let golem = report_signature(
            "a",
            &ticking_entity("alpha:golem", "com.alpha.Golem", "13.37, 64.00, -7.10"),
        );
        let wolf = report_signature(
            "b",
            &ticking_entity("beta:wolf", "com.beta.Wolf", "13.37, 64.00, -7.10"),
        );
        // Na ponta da 1ª revisão as duas davam 5845882d291a584d.
        assert_eq!(golem.rule.as_deref(), Some("crash_report.ticking_entity"));
        assert_eq!(
            golem.frame.as_deref(),
            Some("net.minecraft.world.level.Level.guardEntityTick")
        );
        assert_ne!(golem.hash, wolf.hash);
        assert_eq!(
            golem.subject,
            [
                "entity_type=alpha:golem",
                "description=ticking entity",
                "entity_class=com.alpha.Golem",
            ]
        );

        // A mesma entidade em outro lugar e com outro id: a mesma assinatura.
        let elsewhere = report_signature(
            "c",
            &ticking_entity("alpha:golem", "com.alpha.Golem", "-512.00, 70.00, 1024.50")
                .replace("4817", "99")
                .replace("(13,64,-8)", "(-512,70,1024)"),
        );
        assert_eq!(golem, elsewhere);
    }

    #[test]
    fn blocos_diferentes_com_pilha_so_do_jogo_tem_assinaturas_diferentes() {
        let block = |name: &str, class: &str, at: &str| {
            game_only_report(
                "Ticking block entity",
                &format!(
                    "-- Block entity being ticked --
Details:
\tName: {name} // {class}
\tBlock: Block{{{name}}}[facing=north]
\tBlock location: World: ({at}), Section: (at 1,2,3 in 4,5,6)
"
                ),
            )
        };
        let furnace = report_signature(
            "a",
            &block("alpha:forno", "com.alpha.FornoBlockEntity", "10,64,10"),
        );
        let chest = report_signature(
            "b",
            &block("beta:bau", "com.beta.BauBlockEntity", "10,64,10"),
        );
        let moved = report_signature(
            "c",
            &block("alpha:forno", "com.alpha.FornoBlockEntity", "-300,12,77"),
        );
        assert_eq!(
            furnace.rule.as_deref(),
            Some("crash_report.ticking_block_entity")
        );
        assert_ne!(furnace.hash, chest.hash);
        assert_eq!(furnace, moved);
        assert!(
            furnace
                .subject
                .contains(&"block_class=com.alpha.FornoBlockEntity".to_owned()),
            "{:?}",
            furnace.subject
        );
    }

    #[test]
    fn sujeito_do_achado_vem_da_key_do_catalogo() {
        // Duas dependências diferentes do mesmo mod: causas diferentes.
        let missing = |dependency: &str| {
            of(&[source(
                "latest.log",
                SourceKind::LatestLog,
                &format!(
                    "[12:00:00] [main/ERROR]: Missing or unsupported mandatory dependencies:\n\tMod ID: '{dependency}', Requested by: 'supplementaries', Expected range: '[1.20-2.0,)', Actual version: '[MISSING]'\n"
                ),
            )])
        };
        let moonlight = missing("moonlight");
        let other = missing("cloth_config");
        assert!(moonlight.rule.is_some(), "{moonlight:?}");
        assert_ne!(moonlight.hash, other.hash, "{moonlight:?} × {other:?}");
    }

    #[test]
    fn causa_certa_nao_depende_de_onde_o_jogo_quebrou() {
        // Falta de memória com e sem crash report (e com descrições diferentes): a mesma causa.
        let log = "[12:00:00] [Render thread/ERROR]: Erro\njava.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.client.Minecraft.run(Minecraft.java:10)\n";
        let alone = of(&[source("latest.log", SourceKind::LatestLog, log)]);
        let with_report = |description: &str| {
            of(&[
                source(
                    "crash-reports/c.txt",
                    SourceKind::CrashReport,
                    &format!(
                        "---- Minecraft Crash Report ----\nDescription: {description}\n\njava.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.client.Minecraft.run(Minecraft.java:10)\n"
                    ),
                ),
                source("latest.log", SourceKind::LatestLog, log),
            ])
        };
        assert_eq!(alone.rule.as_deref(), Some("memory.out_of_memory"));
        assert_eq!(alone.subject, Vec::<String>::new());
        assert_eq!(with_report("Unexpected error"), alone);
        assert_eq!(with_report("Rendering overlay"), alone);
    }

    // ---- Pilhas só do jogo e métodos injetados no Forge/NeoForge (2ª revisão) -----------------

    /// Crash report do Forge 1.20.1 com um `NullPointerException` "Unexpected error" e a pilha
    /// dada, com as anotações do `ModLauncher`.
    fn forge_report(stack: &str) -> String {
        format!(
            "---- Minecraft Crash Report ----
// Why did you do that?

Time: 2026-10-13 12:00:01
Description: Unexpected error

java.lang.NullPointerException: Cannot invoke \"Object.hashCode()\" because \"<local3>\" is null
{stack}

A detailed walkthrough of the error, its code path and all known details is as follows:
---------------------------------------------------------------------------------------

-- Head --
Thread: Render thread
Stacktrace:
{stack}

-- System Details --
Details:
\tMinecraft Version: 1.20.1
"
        )
    }

    const SRG: &str = "~[client-1.20.1-20230612.114412-srg.jar%23185!/:?]";

    /// (a) No render, só classes do jogo.
    fn render_stack() -> String {
        format!(
            "\tat net.minecraft.client.renderer.GameRenderer.render(GameRenderer.java:954) {SRG} {{re:classloading,pl:accesstransformer:B,pl:runtimedistcleaner:A}}
\tat net.minecraft.client.Minecraft.runTick(Minecraft.java:1146) {SRG} {{re:classloading,pl:accesstransformer:B,pl:runtimedistcleaner:A}}
\tat net.minecraft.client.main.Main.main(Main.java:218) ~[forge-47.2.0.jar:?] {{re:classloading}}"
        )
    }

    /// (b) No tick do servidor, só classes do jogo.
    fn server_stack() -> String {
        format!(
            "\tat net.minecraft.server.level.ServerLevel.tick(ServerLevel.java:331) {SRG} {{re:classloading,pl:accesstransformer:B}}
\tat net.minecraft.server.MinecraftServer.tickChildren(MinecraftServer.java:893) {SRG} {{re:classloading,pl:accesstransformer:B}}
\tat net.minecraft.server.MinecraftServer.tickServer(MinecraftServer.java:814) {SRG} {{re:classloading,pl:accesstransformer:B}}"
        )
    }

    /// (c) Num método injetado pelo Mixin numa classe do jogo; `configs` são as anotadas.
    fn injected_stack(id: &str, line: u32, configs: &[&str]) -> String {
        let applied = configs
            .iter()
            .map(|config| format!(",pl:mixin:APP:{config}:MixinEntity"))
            .collect::<Vec<_>>()
            .concat();
        format!(
            "\tat net.minecraft.world.entity.Entity.handler${id}$aoTick(Entity.java:{line}) {SRG} {{re:mixin,pl:accesstransformer:B,re:classloading{applied},pl:mixin:A}}
\tat net.minecraft.world.entity.Entity.tick(Entity.java:{line}) {SRG} {{re:mixin,pl:accesstransformer:B,re:classloading{applied},pl:mixin:A}}
\tat net.minecraft.server.level.ServerLevel.tickNonPassenger(ServerLevel.java:693) {SRG} {{re:classloading,pl:accesstransformer:B}}"
        )
    }

    fn forge_signature(stack: &str) -> CrashSignature {
        let sources = [source(
            "crash-reports/crash-2026-10-13_12.00.01-client.txt",
            SourceKind::CrashReport,
            &forge_report(stack),
        )];
        let context = AnalysisContext {
            loader: Some(LogLoader::Forge),
            minecraft: Some("1.20.1".into()),
        };
        signature(&analyze(&sources, &context, None).unwrap(), &sources)
    }

    #[test]
    fn forge_sem_frame_de_mod_render_tick_e_metodo_injetado_tem_assinaturas_diferentes() {
        let render = forge_signature(&render_stack());
        let server = forge_signature(&server_stack());
        let create = forge_signature(&injected_stack("zza000", 1234, &["create.mixins.json"]));
        let sodium = forge_signature(&injected_stack("zza000", 1234, &["sodium.mixins.json"]));
        // Na ponta da 2ª revisão (199efdc) as quatro davam 5d79fb915e49c225.
        let hashes: BTreeSet<&str> = [&render, &server, &create, &sodium]
            .iter()
            .map(|signature| signature.hash.as_str())
            .collect();
        assert_eq!(
            hashes.len(),
            4,
            "{render:?}\n{server:?}\n{create:?}\n{sodium:?}"
        );
        assert_eq!(render.rule.as_deref(), Some("crash_report.description"));
        assert_eq!(
            render.frame.as_deref(),
            Some("net.minecraft.client.renderer.GameRenderer.render")
        );
        assert_eq!(
            server.frame.as_deref(),
            Some("net.minecraft.server.level.ServerLevel.tick")
        );
        // O método injetado é o frame de mod; a config anotada diz de quem pode ser.
        assert_eq!(
            create.frame.as_deref(),
            Some("net.minecraft.world.entity.Entity.aoTick")
        );
        assert!(
            create
                .subject
                .contains(&"frame_mixin=create.mixins.json".to_owned()),
            "{:?}",
            create.subject
        );
    }

    #[test]
    fn forge_sem_frame_de_mod_numeros_e_ids_de_mixin_diferentes_continuam_juntos() {
        let render = forge_signature(&render_stack());
        let render_again = forge_signature(
            &render_stack()
                .replace("GameRenderer.java:954", "GameRenderer.java:960")
                .replace("Minecraft.java:1146", "Minecraft.java:1150")
                .replace("%23185", "%23190"),
        );
        assert_eq!(render, render_again);

        let server = forge_signature(&server_stack());
        let server_again = forge_signature(
            &server_stack()
                .replace("ServerLevel.java:331", "ServerLevel.java:340")
                .replace("%23185", "%2399"),
        );
        assert_eq!(server, server_again);

        let first = forge_signature(&injected_stack("zza000", 1234, &["create.mixins.json"]));
        // Outro id (com e sem algarismo) e outra linha: o mesmo travamento.
        for (id, line) in [
            ("zza00a", 1250),
            ("bkb1f3", 77),
            ("zzaabc", 1234),
            ("cfdfed", 9),
        ] {
            assert_eq!(
                forge_signature(&injected_stack(id, line, &["create.mixins.json"])),
                first,
                "{id}"
            );
        }
        // Configs anotadas em outra ordem: o mesmo travamento.
        let two = forge_signature(&injected_stack(
            "zza000",
            1234,
            &["create.mixins.json", "flywheel.mixins.json"],
        ));
        let swapped = forge_signature(&injected_stack(
            "zzb001",
            1300,
            &["flywheel.mixins.json", "create.mixins.json"],
        ));
        assert_eq!(two, swapped);
    }

    #[test]
    fn neoforge_handler_com_o_mod_no_nome_nao_depende_das_outras_configs_da_classe() {
        // No NeoForge o handler traz o mod e a anotação diz de que mod é cada config: o mod que
        // outro mod acrescentou à mesma classe não separa o travamento.
        let stack = |extra: &str| {
            format!(
                "\tat TRANSFORMER/minecraft@1.21.1/net.minecraft.server.level.ServerPlayer.handler$bbp000$meumod$aoEntrar(ServerPlayer.java:3183) ~[client-1.21.1-20240808.144430-srg.jar%23345!/:?] {{re:mixin,re:classloading,pl:mixin:APP:meumod.mixins.json:MixinJogador from mod meumod{extra},pl:mixin:A}}
\tat TRANSFORMER/minecraft@1.21.1/net.minecraft.server.level.ServerPlayer.<init>(ServerPlayer.java:294) ~[client-1.21.1-20240808.144430-srg.jar%23345!/:?] {{re:mixin}}"
            )
        };
        let alone = forge_signature(&stack(""));
        let with_other = forge_signature(&stack(
            ",pl:mixin:APP:sable.mixins.json:ServerPlayerMixin from mod sable",
        ));
        assert_eq!(
            alone.frame.as_deref(),
            Some("net.minecraft.server.level.ServerPlayer.meumod$aoEntrar")
        );
        assert_eq!(alone, with_other);
        assert!(
            !alone
                .subject
                .iter()
                .any(|part| part.starts_with("frame_mixin=")),
            "{:?}",
            alone.subject
        );
    }

    #[test]
    fn frame_so_anotado_com_config_de_mixin_vale_com_a_config() {
        // Um método que um mod acrescentou à classe do jogo (sem prefixo de injeção) com a config
        // anotada: sem frame de mod, o frame anotado e a config.
        let stack = |config: &str| {
            format!(
                "\tat net.minecraft.world.level.Level.getChunkLithium(Level.java:3705) {SRG} {{re:mixin,pl:mixin:APP:{config}:WorldMixin,pl:mixin:A}}
\tat net.minecraft.server.MinecraftServer.tickServer(MinecraftServer.java:814) {SRG} {{re:classloading}}"
            )
        };
        let lithium = forge_signature(&stack("radium.mixins.json"));
        let other = forge_signature(&stack("roadrunner.mixins.json"));
        assert_eq!(
            lithium.frame.as_deref(),
            Some("net.minecraft.world.level.Level.getChunkLithium")
        );
        assert!(
            lithium
                .subject
                .contains(&"frame_mixin=radium.mixins.json".to_owned())
        );
        assert_ne!(lithium.hash, other.hash);
        assert_eq!(
            forge_signature(&stack("radium.mixins.json").replace("3705", "3800")),
            lithium
        );
    }

    #[test]
    fn primeiro_frame_da_causa_mais_profunda_com_caused_by_encadeado_e_pilha_truncada() {
        // A causa mais profunda só com frames do Java e `... N more`: vale o primeiro frame fora
        // do Java, subindo de seção se preciso.
        let report = |deep: &str| {
            format!(
                "---- Minecraft Crash Report ----
Description: Unexpected error

java.lang.RuntimeException: Erro
\tat net.minecraft.client.Minecraft.runTick(Minecraft.java:1146) {SRG}
\tat net.minecraft.client.main.Main.main(Main.java:218) {SRG}
Caused by: java.lang.IllegalStateException: estado
\tat {deep}(Unknown Source) {SRG}
\tat net.minecraft.client.Minecraft.runTick(Minecraft.java:1140) {SRG}
\t... 1 more
Caused by: java.lang.NullPointerException
\tat java.base/java.util.Objects.requireNonNull(Objects.java:233)
\t... 3 more
"
            )
        };
        let sign = |deep: &str| report_signature("c", &report(deep));
        let gui = sign("net.minecraft.client.gui.Gui.render");
        let sound = sign("net.minecraft.client.sounds.SoundEngine.tick");
        assert_eq!(
            gui.exception.as_deref(),
            Some("java.lang.NullPointerException")
        );
        assert_eq!(
            gui.frame.as_deref(),
            Some("net.minecraft.client.gui.Gui.render")
        );
        assert_ne!(gui.hash, sound.hash);
        // Outros números e outro "N more": o mesmo travamento.
        let again = report_signature(
            "d",
            &report("net.minecraft.client.gui.Gui.render")
                .replace("1146", "1200")
                .replace("... 3 more", "... 4 more"),
        );
        assert_eq!(gui, again);
    }

    #[test]
    fn stack_overflow_com_trecho_omitido_e_recursao_em_outro_ponto() {
        // O topo de um `StackOverflowError` cai num ponto qualquer da recursão; o Forge corta o
        // meio da pilha com `[...]`.
        let report = |cycle: &[&str]| {
            let frames = cycle
                .iter()
                .cycle()
                .take(12)
                .map(|frame| format!("\tat {frame}(X.java:1) ~[?:?] {{re:classloading}}\n"))
                .collect::<Vec<_>>()
                .concat();
            format!(
                "---- Minecraft Crash Report ----
Description: Ticking block entity

java.lang.StackOverflowError: Ticking block entity
\tat net.minecraft.world.level.Level.getBlockState(Level.java:1) ~[?:?] {{re:classloading}}
    [...]
{frames}-- Block entity being ticked --
Details:
\tName: pipez:energy_pipe // de.maxhenkel.pipez.EnergyPipeTileEntity
"
            )
        };
        let one = report_signature(
            "a",
            &report(&[
                "com.alpha.Cano.receber",
                "com.beta.Tanque.encher",
                "com.alpha.Cano.passar",
            ]),
        );
        let shifted = report_signature(
            "b",
            &report(&[
                "com.beta.Tanque.encher",
                "com.alpha.Cano.passar",
                "com.alpha.Cano.receber",
            ]),
        );
        let other = report_signature(
            "c",
            &report(&["com.gama.Fio.receber", "com.gama.Fio.passar"]),
        );
        assert_eq!(one.frame.as_deref(), Some("com.alpha.Cano.passar"));
        assert_eq!(one, shifted);
        assert_ne!(one.hash, other.hash);
    }

    // ---- Frame da queda (3ª revisão) --------------------------------------------------------

    const CLIENT_1_20: &str = "~[client-1.20.1-20230612.114412-srg.jar%23185!/:?]";

    /// Crash report do Forge 1.20.1 com `exception` e a pilha `frames`, repetida na seção
    /// `-- Head --`, e a lista de mods em `mods` (nessa ordem).
    fn forge_1_20_report(exception: &str, frames: &str, mods: &[&str]) -> String {
        let list = mods
            .iter()
            .map(|id| {
                format!(
                    "\t\t{id}-1.20.1.jar                                 |{id}                          |{id}                          |1.0                 |DONE      |Manifest: NOSIGNATURE\n"
                )
            })
            .collect::<Vec<_>>()
            .concat();
        format!(
            "---- Minecraft Crash Report ----
// Don't do that.

Time: 2026-10-13 21:14:07
Description: Unexpected error

{exception}
{frames}

A detailed walkthrough of the error, its code path and all known details is as follows:
---------------------------------------------------------------------------------------

-- Head --
Thread: Render thread
Suspected Mod: NONE
Stacktrace:
{frames}

-- System Details --
Details:
\tMinecraft Version: 1.20.1
\tMinecraft Version ID: 1.20.1
\tJava Version: 17.0.8, Eclipse Adoptium
\tMod List: 
\t\tclient-1.20.1-20230612.114412-srg.jar             |Minecraft                     |minecraft                     |1.20.1              |DONE      |Manifest: a1:d4:5e:04
{list}\tCrash Report UUID: 5d1c43a6-0c5f-4a3e-9b38-3fa0d6b35a51
\tFML: 47.2
\tForge: net.minecraftforge:47.2.0
"
        )
    }

    /// O fim da pilha do Render thread no Forge 1.20.1: `Minecraft.runTick` (`m_91383_`) e o
    /// `Main` com as configs de mixin que os mods do pack anotam neles (`configs`).
    fn forge_1_20_tail(configs: &[&str], address: u32, line: u32) -> String {
        let applied = configs
            .iter()
            .map(|config| format!(",pl:mixin:APP:{config}:MinecraftMixin"))
            .collect::<Vec<_>>()
            .concat();
        let client = CLIENT_1_20.replace("%23185", &format!("%23{address}"));
        format!(
            "\tat net.minecraft.client.Minecraft.m_91383_(Minecraft.java:{line}) {client} {{re:mixin,pl:accesstransformer:B,pl:runtimedistcleaner:A,re:classloading,pl:accesstransformer:B{applied},pl:mixin:A,pl:runtimedistcleaner:A}}
\tat net.minecraft.client.Minecraft.m_91374_(Minecraft.java:718) {client} {{re:mixin,pl:accesstransformer:B,pl:runtimedistcleaner:A,re:classloading,pl:accesstransformer:B{applied},pl:mixin:A,pl:runtimedistcleaner:A}}
\tat net.minecraft.client.main.Main.main(Main.java:218) ~[forge-47.2.0.jar:?] {{re:mixin,pl:runtimedistcleaner:A,re:classloading,pl:mixin:APP:flywheel.mixins.json:ClientMainMixin,pl:mixin:A,pl:runtimedistcleaner:A}}
\tat java.base/jdk.internal.reflect.DirectMethodHandleAccessor.invoke(DirectMethodHandleAccessor.java:104) ~[?:?] {{}}
\tat java.base/java.lang.reflect.Method.invoke(Method.java:578) ~[?:?] {{}}
\tat net.minecraftforge.fml.loading.targets.CommonLaunchHandler.runTarget(CommonLaunchHandler.java:111) ~[fmlloader-1.20.1-47.2.0.jar:?] {{}}
\tat net.minecraftforge.fml.loading.targets.CommonClientLaunchHandler.lambda$launchService$0(CommonClientLaunchHandler.java:20) ~[fmlloader-1.20.1-47.2.0.jar:?] {{}}
\tat cpw.mods.modlauncher.LaunchServiceHandlerDecorator.launch(LaunchServiceHandlerDecorator.java:30) ~[modlauncher-10.0.9.jar:?] {{}}
\tat cpw.mods.modlauncher.Launcher.run(Launcher.java:108) ~[modlauncher-10.0.9.jar:?] {{}}
\tat cpw.mods.bootstraplauncher.BootstrapLauncher.main(BootstrapLauncher.java:141) ~[bootstraplauncher-1.1.2.jar:?] {{}}"
        )
    }

    /// Frames do jogo sem config de mixin, no formato do Forge 1.20.1.
    fn forge_1_20_frames(frames: &[(&str, &str, u32)], address: u32) -> String {
        let client = CLIENT_1_20.replace("%23185", &format!("%23{address}"));
        frames
            .iter()
            .map(|(method, file, line)| {
                format!(
                    "\tat {method}({file}.java:{line}) {client} {{re:mixin,pl:accesstransformer:B,pl:runtimedistcleaner:A,re:classloading,pl:accesstransformer:B,pl:mixin:A,pl:runtimedistcleaner:A}}\n"
                )
            })
            .collect::<Vec<_>>()
            .concat()
    }

    fn signature_of(name: &str, text: &str, loader: LogLoader, minecraft: &str) -> CrashSignature {
        let sources = [source(
            &format!("crash-reports/{name}.txt"),
            SourceKind::CrashReport,
            text,
        )];
        let context = AnalysisContext {
            loader: Some(loader),
            minecraft: Some(minecraft.into()),
        };
        signature(&analyze(&sources, &context, None).unwrap(), &sources)
    }

    const NPE_1_20: &str = "java.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.m_20185_()\" because \"p_109094_\" is null";
    const CREATE_MODERNFIX: [&str; 2] = ["create.mixins.json", "modernfix-common.mixins.json"];

    /// As três quedas da 3ª revisão: no render, no render do mundo e no tick das entidades do
    /// cliente, todas com `Minecraft.runTick` anotado mais abaixo.
    fn forge_1_20_three() -> [CrashSignature; 3] {
        let mods = ["create", "modernfix", "flywheel"];
        let tail = forge_1_20_tail(&CREATE_MODERNFIX, 185, 1146);
        let render = forge_1_20_frames(
            &[(
                "net.minecraft.client.renderer.GameRenderer.m_109093_",
                "GameRenderer",
                954,
            )],
            185,
        );
        let level = forge_1_20_frames(
            &[
                (
                    "net.minecraft.client.renderer.LevelRenderer.m_109599_",
                    "LevelRenderer",
                    1150,
                ),
                (
                    "net.minecraft.client.renderer.GameRenderer.m_109089_",
                    "GameRenderer",
                    1131,
                ),
                (
                    "net.minecraft.client.renderer.GameRenderer.m_109093_",
                    "GameRenderer",
                    954,
                ),
            ],
            185,
        );
        let entities = forge_1_20_frames(
            &[
                (
                    "net.minecraft.client.multiplayer.ClientLevel.m_104804_",
                    "ClientLevel",
                    274,
                ),
                ("net.minecraft.client.Minecraft.m_91398_", "Minecraft", 1814),
            ],
            185,
        );
        [render, level, entities].map(|top| {
            signature_of(
                "crash-2026-10-13_21.14.07-client",
                &forge_1_20_report(NPE_1_20, &format!("{top}{tail}"), &mods),
                LogLoader::Forge,
                "1.20.1",
            )
        })
    }

    #[test]
    fn forge_1_20_quedas_diferentes_com_run_tick_anotado_embaixo_tem_assinaturas_diferentes() {
        let [render, level, entities] = forge_1_20_three();
        // Na ponta da 3ª revisão (d045cd1) as três davam o mesmo hash, com o frame
        // `Minecraft.m_91383_` (runTick) e as configs anotadas nele.
        let hashes: BTreeSet<&str> = [&render, &level, &entities]
            .iter()
            .map(|signature| signature.hash.as_str())
            .collect();
        assert_eq!(hashes.len(), 3, "{render:?}\n{level:?}\n{entities:?}");
        assert_eq!(
            render.frame.as_deref(),
            Some("net.minecraft.client.renderer.GameRenderer.m_109093_")
        );
        assert_eq!(
            level.frame.as_deref(),
            Some("net.minecraft.client.renderer.LevelRenderer.m_109599_")
        );
        assert_eq!(
            entities.frame.as_deref(),
            Some("net.minecraft.client.multiplayer.ClientLevel.m_104804_")
        );
        for signature in [&render, &level, &entities] {
            assert_eq!(signature.rule.as_deref(), Some("crash_report.description"));
            assert!(
                !signature
                    .subject
                    .iter()
                    .any(|part| part.starts_with("frame_mixin=")),
                "{:?}",
                signature.subject
            );
        }
    }

    #[test]
    fn forge_1_20_a_mesma_queda_com_outras_configs_embaixo_linhas_enderecos_e_mods_continua_junta()
    {
        let [render, ..] = forge_1_20_three();
        let top = |address| {
            forge_1_20_frames(
                &[(
                    "net.minecraft.client.renderer.GameRenderer.m_109093_",
                    "GameRenderer",
                    961,
                )],
                address,
            )
        };
        // Outras configs anotadas em `runTick` e em outra ordem, outras linhas, outros endereços
        // e os mods do pack em outra ordem.
        let again = signature_of(
            "crash-2026-10-14_08.02.55-client",
            &forge_1_20_report(
                NPE_1_20,
                &format!(
                    "{}{}",
                    top(201),
                    forge_1_20_tail(
                        &[
                            "oculus.mixins.json",
                            "embeddium.mixins.json",
                            "ferritecore.mixin.json"
                        ],
                        201,
                        1152
                    )
                ),
                &["flywheel", "modernfix", "create"],
            ),
            LogLoader::Forge,
            "1.20.1",
        );
        assert_eq!(render, again);
        // Sem nenhuma config anotada embaixo: a mesma queda.
        let bare = signature_of(
            "crash-2026-10-15_10.00.00-client",
            &forge_1_20_report(
                NPE_1_20,
                &format!("{}{}", top(77), forge_1_20_tail(&[], 77, 1146)),
                &["create"],
            ),
            LogLoader::Forge,
            "1.20.1",
        );
        assert_eq!(render, bare);
    }

    /// NeoForge 1.21.1 (nomes mojmap): `frames` com `TRANSFORMER/minecraft@1.21.1/` e o fim da
    /// pilha com `runTick` anotado com as configs `configs` (`from mod …`).
    fn neoforge_1_21_report(frames: &[(&str, &str, u32)], configs: &[(&str, &str)]) -> String {
        let client = "~[client-1.21.1-20240808.144430-srg.jar%23345!/:?]";
        let applied = configs
            .iter()
            .map(|(config, owner)| {
                format!(",pl:mixin:APP:{config}:MinecraftMixin from mod {owner}")
            })
            .collect::<Vec<_>>()
            .concat();
        let top = frames
            .iter()
            .map(|(method, file, line)| {
                format!(
                    "\tat TRANSFORMER/minecraft@1.21.1/{method}({file}.java:{line}) {client} {{re:mixin,pl:accesstransformer:B,xf:fml:neoforge:neoforge_method_redirector,re:classloading,pl:accesstransformer:B,xf:fml:neoforge:neoforge_method_redirector,pl:mixin:A}}\n"
                )
            })
            .collect::<Vec<_>>()
            .concat();
        let stack = format!(
            "{top}\tat TRANSFORMER/minecraft@1.21.1/net.minecraft.client.Minecraft.runTick(Minecraft.java:1225) {client} {{re:mixin,pl:accesstransformer:B,xf:fml:neoforge:neoforge_method_redirector,re:classloading,pl:accesstransformer:B,xf:fml:neoforge:neoforge_method_redirector{applied},pl:mixin:A}}
\tat TRANSFORMER/minecraft@1.21.1/net.minecraft.client.Minecraft.run(Minecraft.java:875) {client} {{re:mixin,pl:accesstransformer:B,xf:fml:neoforge:neoforge_method_redirector,re:classloading,pl:accesstransformer:B,xf:fml:neoforge:neoforge_method_redirector{applied},pl:mixin:A}}
\tat TRANSFORMER/minecraft@1.21.1/net.minecraft.client.main.Main.main(Main.java:241) {client} {{re:classloading,pl:mixin:A}}
\tat java.base/jdk.internal.reflect.DirectMethodHandleAccessor.invoke(DirectMethodHandleAccessor.java:103) ~[?:?] {{}}
\tat java.base/java.lang.reflect.Method.invoke(Method.java:580) ~[?:?] {{}}
\tat MC-BOOTSTRAP/fml_loader@4.0.24/net.neoforged.fml.loading.targets.CommonLaunchHandler.runTarget(CommonLaunchHandler.java:136) ~[loader-4.0.24.jar%23121!/:4.0] {{}}
\tat MC-BOOTSTRAP/cpw.mods.modlauncher@11.0.4/cpw.mods.modlauncher.Launcher.run(Launcher.java:103) ~[modlauncher-11.0.4.jar%23109!/:?] {{}}
\tat MC-BOOTSTRAP/cpw.mods.modlauncher@11.0.4/cpw.mods.modlauncher.Launcher.main(Launcher.java:74) ~[modlauncher-11.0.4.jar%23109!/:?] {{}}
\tat cpw.mods.bootstraplauncher.BootstrapLauncher.main(BootstrapLauncher.java:210) ~[bootstraplauncher-2.0.2.jar:?] {{}}"
        );
        format!(
            "---- Minecraft Crash Report ----
// Hi. I'm Minecraft, and I'm a crashaholic.

Time: 2026-10-13 21:14:07
Description: Unexpected error

java.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.getX()\" because \"entity\" is null
{stack}

A detailed walkthrough of the error, its code path and all known details is as follows:
---------------------------------------------------------------------------------------

-- Head --
Thread: Render thread
Stacktrace:
{stack}

-- System Details --
Details:
\tMinecraft Version: 1.21.1
"
        )
    }

    fn neoforge_signature(
        frames: &[(&str, &str, u32)],
        configs: &[(&str, &str)],
    ) -> CrashSignature {
        signature_of(
            "crash-2026-10-13_21.14.07-client",
            &neoforge_1_21_report(frames, configs),
            LogLoader::NeoForge,
            "1.21.1",
        )
    }

    #[test]
    fn neoforge_from_mod_sodium_so_em_run_tick_nao_escolhe_run_tick() {
        let sodium = [("sodium-neoforge.mixins.json", "sodium")];
        let render = [(
            "net.minecraft.client.renderer.GameRenderer.render",
            "GameRenderer",
            1024,
        )];
        let level = [
            (
                "net.minecraft.client.renderer.LevelRenderer.renderLevel",
                "LevelRenderer",
                1180,
            ),
            (
                "net.minecraft.client.renderer.GameRenderer.renderLevel",
                "GameRenderer",
                1136,
            ),
            (
                "net.minecraft.client.renderer.GameRenderer.render",
                "GameRenderer",
                1024,
            ),
        ];
        let entities = [
            (
                "net.minecraft.client.multiplayer.ClientLevel.tickEntities",
                "ClientLevel",
                276,
            ),
            ("net.minecraft.client.Minecraft.tick", "Minecraft", 1890),
        ];
        let one = neoforge_signature(&render, &sodium);
        // Na ponta d045cd1: frame `net.minecraft.client.Minecraft.runTick` com
        // `frame_mixin=sodium-neoforge.mixins.json`, o mesmo hash para as três.
        assert_eq!(
            one.frame.as_deref(),
            Some("net.minecraft.client.renderer.GameRenderer.render")
        );
        assert!(
            !one.subject
                .iter()
                .any(|part| part.starts_with("frame_mixin=")),
            "{:?}",
            one.subject
        );
        let two = neoforge_signature(&level, &sodium);
        let three = neoforge_signature(&entities, &sodium);
        assert_eq!(
            two.frame.as_deref(),
            Some("net.minecraft.client.renderer.LevelRenderer.renderLevel")
        );
        assert_eq!(
            three.frame.as_deref(),
            Some("net.minecraft.client.multiplayer.ClientLevel.tickEntities")
        );
        let hashes: BTreeSet<&str> = [&one, &two, &three]
            .iter()
            .map(|signature| signature.hash.as_str())
            .collect();
        assert_eq!(hashes.len(), 3, "{one:?}\n{two:?}\n{three:?}");
        // Outros mods com mixin em `runTick`: a mesma queda.
        let others = neoforge_signature(
            &render,
            &[
                ("iris.mixins.json", "iris"),
                ("modernfix-common.mixins.json", "modernfix"),
            ],
        );
        assert_eq!(one, others);
        assert_eq!(one, neoforge_signature(&render, &[]));
    }

    /// Crash report do Forge antigo (1.7.10 e 1.12.2, nomes SRG `func_N_x`, sem anotação).
    fn forge_legacy_report(frames: &[&str], minecraft: &str) -> String {
        let stack = frames
            .iter()
            .map(|frame| format!("\tat {frame}\n"))
            .collect::<Vec<_>>()
            .concat();
        format!(
            "---- Minecraft Crash Report ----
// Shall we play a game?

Time: 13/10/26 21:14
Description: Unexpected error

java.lang.NullPointerException: Unexpected error
{stack}\tat net.minecraft.client.Minecraft.func_99999_d(Minecraft.java:441)
\tat net.minecraft.client.main.Main.main(SourceFile:123)
\tat sun.reflect.NativeMethodAccessorImpl.invoke0(Native Method)
\tat sun.reflect.NativeMethodAccessorImpl.invoke(NativeMethodAccessorImpl.java:62)
\tat sun.reflect.DelegatingMethodAccessorImpl.invoke(DelegatingMethodAccessorImpl.java:43)
\tat java.lang.reflect.Method.invoke(Method.java:498)
\tat net.minecraft.launchwrapper.Launch.launch(Launch.java:135)
\tat net.minecraft.launchwrapper.Launch.main(Launch.java:28)


A detailed walkthrough of the error, its code path and all known details is as follows:
---------------------------------------------------------------------------------------

-- System Details --
Details:
\tMinecraft Version: {minecraft}
\tFML: MCP 9.42 Powered by Forge 14.23.5.2860 7 mods loaded, 7 mods active
"
        )
    }

    #[test]
    fn forge_antigo_sem_frame_de_mod_quedas_em_metodos_srg_diferentes_tem_assinaturas_diferentes() {
        for minecraft in ["1.7.10", "1.12.2"] {
            let sign = |frames: &[&str]| {
                signature_of(
                    "crash-2026-10-13_21.14.07-client",
                    &forge_legacy_report(frames, minecraft),
                    LogLoader::Forge,
                    minecraft,
                )
            };
            let render = sign(&[
                "net.minecraft.client.renderer.EntityRenderer.func_78480_b(EntityRenderer.java:1180)",
                "net.minecraft.client.Minecraft.func_71411_J(Minecraft.java:1208)",
            ]);
            let world = sign(&[
                "net.minecraft.client.multiplayer.WorldClient.func_72835_b(WorldClient.java:112)",
                "net.minecraft.client.Minecraft.func_71407_l(Minecraft.java:1873)",
                "net.minecraft.client.Minecraft.func_71411_J(Minecraft.java:1208)",
            ]);
            assert_eq!(
                render.frame.as_deref(),
                Some("net.minecraft.client.renderer.EntityRenderer.func_78480_b"),
                "{minecraft}"
            );
            assert_eq!(
                world.frame.as_deref(),
                Some("net.minecraft.client.multiplayer.WorldClient.func_72835_b"),
                "{minecraft}"
            );
            assert_ne!(render.hash, world.hash, "{minecraft}");
            // Outras linhas: a mesma queda.
            let again = sign(&[
                "net.minecraft.client.renderer.EntityRenderer.func_78480_b(EntityRenderer.java:1190)",
                "net.minecraft.client.Minecraft.func_71411_J(Minecraft.java:1212)",
            ]);
            assert_eq!(render, again, "{minecraft}");
        }
    }

    #[test]
    fn fabric_intermediary_sem_frame_de_mod_quedas_diferentes_tem_assinaturas_diferentes() {
        let report = |frames: &str| {
            format!(
                "---- Minecraft Crash Report ----
// Ouch. That hurt :(

Time: 2026-10-13 21:14:07
Description: Rendering entity in world

java.lang.NullPointerException: Cannot invoke \"net.minecraft.class_1297.method_23317()\" because \"entity\" is null
{frames}\tat net.minecraft.class_310.method_1523(class_310.java:1189)
\tat net.minecraft.class_310.method_1514(class_310.java:802)
\tat net.minecraft.client.main.Main.main(Main.java:250)
\tat net.fabricmc.loader.impl.game.minecraft.MinecraftGameProvider.launch(MinecraftGameProvider.java:470)
\tat net.fabricmc.loader.impl.launch.knot.Knot.launch(Knot.java:74)
\tat net.fabricmc.loader.impl.launch.knot.KnotClient.main(KnotClient.java:23)

-- System Details --
Details:
\tMinecraft Version: 1.20.1
"
            )
        };
        let sign =
            |frames: &str| signature_of("crash", &report(frames), LogLoader::Fabric, "1.20.1");
        let render = sign("\tat net.minecraft.class_757.method_3192(class_757.java:880)\n");
        let level = sign(
            "\tat net.minecraft.class_761.method_22710(class_761.java:1226)\n\tat net.minecraft.class_757.method_3188(class_757.java:1110)\n\tat net.minecraft.class_757.method_3192(class_757.java:880)\n",
        );
        assert_eq!(
            render.frame.as_deref(),
            Some("net.minecraft.class_757.method_3192")
        );
        assert_eq!(
            level.frame.as_deref(),
            Some("net.minecraft.class_761.method_22710")
        );
        assert_ne!(render.hash, level.hash);
    }

    // ---- Casos adversários da regra do frame da queda ---------------------------------------

    #[test]
    fn adversario_queda_dentro_de_lambda_pula_a_classe_gerada_e_o_java() {
        let tail = forge_1_20_tail(&CREATE_MODERNFIX, 185, 1146);
        let lambda = |number: u32, generated: &str, method: &str| {
            format!(
                "\tat java.base/java.util.Objects.requireNonNull(Objects.java:233) ~[?:?] {{}}
\tat net.minecraft.client.renderer.LevelRenderer.lambda${method}${number}(LevelRenderer.java:1312) {CLIENT_1_20} {{re:mixin,re:classloading,pl:mixin:A}}
\tat net.minecraft.client.renderer.LevelRenderer$$Lambda${generated}.accept(Unknown Source) {CLIENT_1_20} {{re:mixin,re:classloading,pl:mixin:A}}
\tat java.base/java.util.ArrayList.forEach(ArrayList.java:1511) ~[?:?] {{}}
\tat net.minecraft.client.renderer.LevelRenderer.m_109599_(LevelRenderer.java:1150) {CLIENT_1_20} {{re:mixin,re:classloading,pl:mixin:A}}
{tail}"
            )
        };
        let sign = |frames: String| {
            signature_of(
                "crash",
                &forge_1_20_report(NPE_1_20, &frames, &["create"]),
                LogLoader::Forge,
                "1.20.1",
            )
        };
        let one = sign(lambda(4, "5123/0x000000080137e8a8", "renderLevel"));
        assert_eq!(
            one.frame.as_deref(),
            Some("net.minecraft.client.renderer.LevelRenderer.lambda$renderLevel")
        );
        // Outro número de lambda e outra classe gerada: a mesma queda.
        assert_eq!(
            one,
            sign(lambda(9, "6001/0x0000000801a2c000", "renderLevel"))
        );
        // Outra lambda: outra queda.
        assert_ne!(
            one.hash,
            sign(lambda(4, "5123/0x000000080137e8a8", "renderEntities")).hash
        );
    }

    #[test]
    fn preconditions_do_guava_no_topo_vale_o_chamador() {
        let tail = forge_1_20_tail(&CREATE_MODERNFIX, 185, 1146);
        let sign = |caller: &str| {
            let frames = format!(
                "\tat com.google.common.base.Preconditions.checkState(Preconditions.java:502) ~[guava-31.1-jre.jar%2398!/:?] {{}}
\tat {caller}(Caller.java:10) {CLIENT_1_20} {{re:mixin,re:classloading,pl:mixin:A}}
{tail}"
            );
            signature_of(
                "crash",
                &forge_1_20_report(
                    "java.lang.IllegalStateException: Not ready",
                    &frames,
                    &["create"],
                ),
                LogLoader::Forge,
                "1.20.1",
            )
        };
        let textures = sign("net.minecraft.client.renderer.texture.TextureManager.m_118506_");
        let sounds = sign("net.minecraft.client.sounds.SoundEngine.m_120313_");
        assert_eq!(
            textures.frame.as_deref(),
            Some("net.minecraft.client.renderer.texture.TextureManager.m_118506_")
        );
        assert_ne!(textures.hash, sounds.hash);
    }

    #[test]
    fn adversario_caused_by_com_n_more_usa_o_fim_da_pilha_de_fora() {
        // A causa só tem frames do Java; o resto dela é o fim da pilha de fora (`... 4 more`):
        // a queda foi em `ModelManager.m_5540_`, não no topo da pilha de fora nem em `runTick`.
        let report = |shared: &str| {
            let outer = forge_1_20_frames(
                &[
                    (
                        "net.minecraft.client.resources.model.ModelManager.m_245476_",
                        "ModelManager",
                        120,
                    ),
                    (shared, "ModelManager", 90),
                ],
                185,
            );
            let tail = forge_1_20_tail(&CREATE_MODERNFIX, 185, 1146);
            let tail: Vec<&str> = tail.lines().collect();
            // Até o `Main`: os frames de fora que a causa repete são `shared`, `runTick`, `run` e
            // `Main.main`.
            let frames = format!(
                "{outer}{}\nCaused by: {NPE_1_20}\n\tat java.base/java.util.Objects.requireNonNull(Objects.java:233) ~[?:?] {{}}\n\tat java.base/java.util.HashMap.get(HashMap.java:556) ~[?:?] {{}}\n\t... 4 more",
                tail[..3].join("\n")
            );
            forge_1_20_report(
                "java.lang.RuntimeException: Falha ao carregar os modelos",
                &frames,
                &["create"],
            )
        };
        let sign =
            |shared: &str| signature_of("crash", &report(shared), LogLoader::Forge, "1.20.1");
        let one = sign("net.minecraft.client.resources.model.ModelManager.m_5540_");
        assert_eq!(
            one.exception.as_deref(),
            Some("java.lang.NullPointerException")
        );
        assert_eq!(
            one.frame.as_deref(),
            Some("net.minecraft.client.resources.model.ModelManager.m_5540_")
        );
        assert!(
            !one.subject
                .iter()
                .any(|part| part.starts_with("frame_mixin=")),
            "{:?}",
            one.subject
        );
        assert_ne!(
            one.hash,
            sign("net.minecraft.client.resources.model.ModelManager.m_5541_").hash
        );
    }

    #[test]
    fn adversario_excecao_sem_nenhum_frame_cai_na_descricao_e_na_excecao() {
        // A JVM tira a pilha das exceções que se repetem (`OmitStackTraceInFastThrow`).
        let report = |description: &str, exception: &str| {
            format!(
                "---- Minecraft Crash Report ----
// Oops.

Time: 2026-10-13 21:14:07
Description: {description}

{exception}


A detailed walkthrough of the error, its code path and all known details is as follows:
---------------------------------------------------------------------------------------

-- Head --
Thread: Render thread
Stacktrace:

-- System Details --
Details:
\tMinecraft Version: 1.20.1
"
            )
        };
        let sign = |description: &str, exception: &str| {
            signature_of(
                "crash",
                &report(description, exception),
                LogLoader::Forge,
                "1.20.1",
            )
        };
        let npe = sign("Unexpected error", "java.lang.NullPointerException");
        assert_eq!(npe.frame, None);
        assert_eq!(npe.rule.as_deref(), Some("crash_report.description"));
        assert_eq!(
            npe.exception.as_deref(),
            Some("java.lang.NullPointerException")
        );
        assert!(
            npe.subject
                .contains(&"description=unexpected error".to_owned()),
            "{:?}",
            npe.subject
        );
        assert_ne!(
            npe.hash,
            sign("Unexpected error", "java.lang.ClassCastException").hash
        );
        assert_ne!(
            npe.hash,
            sign("Rendering overlay", "java.lang.NullPointerException").hash
        );
    }

    #[test]
    fn adversario_queda_num_handler_de_mixin_de_mod_vale_o_handler_e_so_as_configs_dele() {
        let handler = |configs: &[&str]| {
            let applied = configs
                .iter()
                .map(|config| format!(",pl:mixin:APP:{config}:MixinGameRenderer"))
                .collect::<Vec<_>>()
                .concat();
            format!(
                "\tat net.minecraft.client.renderer.GameRenderer.handler$zzb000$renderHud(GameRenderer.java:2410) {CLIENT_1_20} {{re:mixin,pl:accesstransformer:B,re:classloading{applied},pl:mixin:A}}
\tat net.minecraft.client.renderer.GameRenderer.m_109093_(GameRenderer.java:954) {CLIENT_1_20} {{re:mixin,pl:accesstransformer:B,re:classloading{applied},pl:mixin:A}}
"
            )
        };
        let sign = |configs: &[&str], below: &[&str]| {
            signature_of(
                "crash",
                &forge_1_20_report(
                    NPE_1_20,
                    &format!("{}{}", handler(configs), forge_1_20_tail(below, 185, 1146)),
                    &["oculus", "create"],
                ),
                LogLoader::Forge,
                "1.20.1",
            )
        };
        let oculus = sign(&["oculus.mixins.json"], &CREATE_MODERNFIX);
        assert_eq!(
            oculus.frame.as_deref(),
            Some("net.minecraft.client.renderer.GameRenderer.renderHud")
        );
        let configs: Vec<&String> = oculus
            .subject
            .iter()
            .filter(|part| part.starts_with("frame_mixin="))
            .collect();
        assert_eq!(configs, ["frame_mixin=oculus.mixins.json"]);
        // As configs de `runTick` não contam; as do próprio handler, sim.
        assert_eq!(
            oculus,
            sign(&["oculus.mixins.json"], &["embeddium.mixins.json"])
        );
        assert_ne!(
            oculus.hash,
            sign(&["iris.mixins.json"], &CREATE_MODERNFIX).hash
        );
    }

    #[test]
    fn adversario_pilha_truncada_no_crash_report_e_a_mesma_queda_da_pilha_inteira() {
        let top = forge_1_20_frames(
            &[(
                "net.minecraft.client.renderer.GameRenderer.m_109093_",
                "GameRenderer",
                954,
            )],
            185,
        );
        let whole = format!("{top}{}", forge_1_20_tail(&CREATE_MODERNFIX, 185, 1146));
        // O arquivo cortado no meio do frame de `runTick`.
        let cut = format!("{top}\tat net.minecraft.client.Minecraft.m_91383_(Minecraft.ja");
        let sign = |frames: &str| {
            signature_of(
                "crash",
                &forge_1_20_report(NPE_1_20, frames, &["create"]),
                LogLoader::Forge,
                "1.20.1",
            )
        };
        let full = sign(&whole);
        let truncated = sign(&cut);
        assert_eq!(
            truncated.frame.as_deref(),
            Some("net.minecraft.client.renderer.GameRenderer.m_109093_")
        );
        assert_eq!(full, truncated);
        // Só a linha da exceção sobrou: sem frame, sem inventar um.
        let only_exception = sign("");
        assert_eq!(only_exception.frame, None);
    }

    // ---- Mixin ------------------------------------------------------------------------------

    #[test]
    fn id_dos_metodos_injetados_pelo_mixin_sai_em_todas_as_formas() {
        // Fabric (com o mod) e Forge/NeoForge (sem), id decimal e hexadecimal, e sem id.
        let cases = [
            ("a.B.handler$zza000$meumod$aoTick", "a.B.meumod$aoTick"),
            ("a.B.handler$zza00a$meumod$aoTick", "a.B.meumod$aoTick"),
            ("a.B.handler$bkb1f3c$meumod$aoTick", "a.B.meumod$aoTick"),
            ("a.B.handler$meumod$aoTick", "a.B.meumod$aoTick"),
            ("a.B.redirect$zzb00f$meumod$trocar", "a.B.meumod$trocar"),
            ("a.B.modify$bca000$meumod$valor", "a.B.meumod$valor"),
            ("a.B.localvar$zhm00e$meumod$x", "a.B.meumod$x"),
            ("a.B.constant$zzc001$meumod$k", "a.B.meumod$k"),
            ("a.B.args$zzd010$meumod$a", "a.B.meumod$a"),
            (
                "a.B.wrapOperation$cfa00b$meumod$embrulhar",
                "a.B.meumod$embrulhar",
            ),
            ("a.B.wrapWithCondition$cfb000$meumod$se", "a.B.meumod$se"),
            (
                "a.B.modifyExpressionValue$zza0ff$meumod$expr",
                "a.B.meumod$expr",
            ),
            ("a.B.modifyReturnValue$zza100$meumod$ret", "a.B.meumod$ret"),
            ("a.B.modifyReceiver$zza101$meumod$rec", "a.B.meumod$rec"),
            ("a.B.wrapMethod$zza102$meumod$tudo", "a.B.meumod$tudo"),
            ("a.B.handler$zza000$aoTick", "a.B.aoTick"),
            ("a.B$Interna.redirect$abc123$aoTick", "a.B$Interna.aoTick"),
            // O id vale pela posição, com ou sem algarismo (3ª revisão): logo depois do prefixo
            // (Mixin 0.8 e o fork do Fabric) ou no fim (Mixin 0.7, do Forge 1.12).
            ("a.B.handler$zzaabc$meumod$aoTick", "a.B.meumod$aoTick"),
            ("a.B.handler$zzaabc$aoTick", "a.B.aoTick"),
            ("a.B.handler$bkbfed$aoTick", "a.B.aoTick"),
            ("a.B.handler$aoTick$zza000", "a.B.aoTick"),
            ("a.B.handler$aoTick$zzaabc", "a.B.aoTick"),
            // Só o id sai: o nome nunca fica vazio.
            ("a.B.handler$zzaabc", "a.B.zzaabc"),
            // Outros números que mudam com a compilação.
            ("a.B.lambda$tick$12", "a.B.lambda$tick"),
            ("a.B.lambda$3", "a.B.lambda"),
            ("a.B.aoTick$lambda-3", "a.B.aoTick$lambda"),
            ("a.B.access$000", "a.B.access$"),
            (
                "net.minecraftforge.eventbus.ASMEventHandler_12_Foo_bar_Evento.invoke",
                "net.minecraftforge.eventbus.ASMEventHandler_Foo_bar_Evento.invoke",
            ),
            (
                "jdk.internal.reflect.GeneratedMethodAccessor34.invoke",
                "jdk.internal.reflect.GeneratedMethodAccessor.invoke",
            ),
            (
                "com.sun.proxy.$Proxy56.chamar",
                "com.sun.proxy.$Proxy.chamar",
            ),
        ];
        for (raw, expected) in cases {
            assert_eq!(normalize_method(raw), expected, "{raw}");
        }
    }

    #[test]
    fn o_mesmo_travamento_com_ids_de_mixin_diferentes_tem_a_mesma_assinatura() {
        let crash = |id: &str| {
            format!(
                "---- Minecraft Crash Report ----
Description: Unexpected error

java.lang.NullPointerException: null
\tat net.minecraft.class_1297.handler${id}$meumod$aoTick(class_1297.java:1234)
\tat net.minecraft.class_1297.method_5773(class_1297.java:500)
"
            )
        };
        let sources = |id: &str| {
            [source(
                "crash-reports/c.txt",
                SourceKind::CrashReport,
                &crash(id),
            )]
        };
        let context = AnalysisContext {
            loader: Some(LogLoader::Fabric),
            minecraft: None,
        };
        let sign = |id: &str| {
            let sources = sources(id);
            signature(&analyze(&sources, &context, None).unwrap(), &sources)
        };
        let first = sign("zza000");
        assert_eq!(
            first.frame.as_deref(),
            Some("net.minecraft.class_1297.meumod$aoTick")
        );
        for id in ["zza00a", "bkb1f3", "zzc0a1f"] {
            assert_eq!(sign(id), first, "{id}");
        }
        // Outro mod no mesmo ponto: outra causa.
        let other = sources("zza000").map(|_| {
            source(
                "crash-reports/c.txt",
                SourceKind::CrashReport,
                &crash("zza000").replace("meumod", "outromod"),
            )
        });
        let other = signature(&analyze(&other, &context, None).unwrap(), &other);
        assert_ne!(other.hash, first.hash);
    }

    #[test]
    fn frame_de_mod_vem_da_causa_mais_profunda_e_pula_jogo_e_bibliotecas() {
        let log = "[12:00:00] [Render thread/ERROR]: Erro
java.lang.RuntimeException: Erro ao atualizar
\tat net.minecraft.client.Minecraft.tick(Minecraft.java:10)
\tat com.primeiro.Mod.run(Mod.java:3)
Caused by: java.lang.IllegalStateException: estado
\tat java.base/java.util.Objects.requireNonNull(Objects.java:233)
\tat com.google.common.Foo.bar(Foo.java:5)
\tat com.segundo.Mod$$Lambda$1234/0x0000000800c3b2a8.accept(Unknown Source)
\tat com.segundo.Mod.handle(Mod.java:9)
";
        let lines: Vec<CleanLine> = source("latest.log", SourceKind::LatestLog, log).clean_lines();
        let (exception, place) = exception_and_frame(&lines, 0, false, false);
        assert_eq!(
            exception.as_deref(),
            Some("java.lang.IllegalStateException")
        );
        assert_eq!(
            place.frame.as_deref(),
            Some("com.segundo.Mod$$Lambda.accept")
        );
    }

    #[test]
    fn modulos_e_jars_decidem_quando_a_linha_os_tem() {
        let log = "java.lang.NullPointerException
\tat TRANSFORMER/minecraft@1.20.1/net.minecraft.world.Foo.bar(Foo.java:1)
\tat TRANSFORMER/create@0.5.1.f/com.simibubi.create.Bar.tick(Bar.java:2)
";
        let lines = source("x.log", SourceKind::Text, log).clean_lines();
        assert_eq!(
            exception_and_frame(&lines, 0, false, false)
                .1
                .frame
                .as_deref(),
            Some("com.simibubi.create.Bar.tick")
        );
        // Handler de mixin numa classe do jogo é frame de mod, no Fabric (que traz o mod) e fora
        // dele.
        let fabric = "java.lang.NullPointerException
\tat net.minecraft.class_1297.handler$zza000$meumod$aoTick(class_1297.java:1234)
";
        let lines = source("x.log", SourceKind::Text, fabric).clean_lines();
        for loader_is_fabric in [true, false] {
            assert_eq!(
                exception_and_frame(&lines, 0, loader_is_fabric, false)
                    .1
                    .frame
                    .as_deref(),
                Some("net.minecraft.class_1297.meumod$aoTick")
            );
        }
    }

    #[test]
    fn sem_achado_usa_o_crash_report_e_sem_nada_so_o_hash_vazio() {
        let report = "---- Minecraft Crash Report ----
Algo estranho
java.lang.ArithmeticException: / by zero
\tat org.exemplo.Calc.div(Calc.java:12)
";
        let sources = [source(
            "crash-reports/c.txt",
            SourceKind::CrashReport,
            report,
        )];
        let analysis = Analysis {
            findings: Vec::new(),
            loader: None,
            loader_version: None,
            minecraft: None,
            sources: Vec::new(),
        };
        let found = signature(&analysis, &sources);
        assert_eq!(found.rule, None);
        assert_eq!(
            found.exception.as_deref(),
            Some("java.lang.ArithmeticException")
        );
        assert_eq!(found.frame.as_deref(), Some("org.exemplo.Calc.div"));
        let nothing = signature(
            &analysis,
            &[source("output.log", SourceKind::Output, "oi\n")],
        );
        assert_eq!(
            nothing,
            CrashSignature::new(None, None, None, Vec::new(), Vec::new())
        );
        assert_ne!(nothing.hash, found.hash);
    }

    #[test]
    fn frame_nativo_sem_endereco() {
        let text = "#
# A fatal error has been detected by the Java Runtime Environment:
#
#  EXCEPTION_ACCESS_VIOLATION (0xc0000005) at pc=0x000000006b1d99ee, pid=13332, tid=11504
#
# Problematic frame:
# J 4321 C2 net.minecraft.client.Foo.bar()V (45 bytes) @ 0x0000000003a1b2c3 [0x0000000003a1b000+0x2c3]
";
        let lines = source("hs_err_pid1.log", SourceKind::JvmCrash, text).clean_lines();
        assert_eq!(
            native_frame(&lines, 3).as_deref(),
            Some("J net.minecraft.client.Foo.bar")
        );
    }

    #[test]
    fn nomes_de_jar_e_ids_normalizados() {
        assert_eq!(jar_stem("create-1.20.1-0.5.1.f.jar"), "create");
        assert_eq!(jar_stem("mods/Botania-1.20.1-443.jar"), "botania");
        assert_eq!(
            jar_stem("sodium-fabric-0.5.8+mc1.20.1.jar"),
            "sodium-fabric"
        );
        assert_eq!(jar_stem("jei_1.12.2-4.16.jar%2390"), "jei");
        assert_eq!(jar_stem("1.12.jar"), "1.12");
        assert_eq!(
            normalize_item(&ItemRef::ModId {
                id: "Village-Employment".into()
            }),
            "mod:village_employment"
        );
        assert_eq!(
            normalize_item(&ItemRef::Metafile {
                path: "mods/a.pw.toml".into()
            }),
            "item:mods/a.pw.toml"
        );
        assert_eq!(
            normalize_method("a.B.handler$bcd123$m$md$1a2b3c$0"),
            "a.B.m$"
        );
    }

    #[test]
    fn hash_estavel_e_mods_em_ordem() {
        let one = CrashSignature::new(
            Some("r".into()),
            Some("E".into()),
            None,
            vec!["mod:b".into(), "mod:a".into(), "mod:a".into()],
            Vec::new(),
        );
        let two = CrashSignature::new(
            Some("r".into()),
            Some("E".into()),
            None,
            vec!["mod:a".into(), "mod:b".into()],
            Vec::new(),
        );
        assert_eq!(one, two);
        assert_eq!(one.mods, ["mod:a", "mod:b"]);
        // Fixo: mudar a normalização ou o formato exige subir SIGNATURE_VERSION.
        assert_eq!(one.hash, "2e9980cf0d10880b");
        let with_subject = CrashSignature::new(
            Some("r".into()),
            Some("E".into()),
            None,
            vec!["mod:a".into(), "mod:b".into()],
            vec!["description=x".into()],
        );
        assert_ne!(with_subject.hash, one.hash);
        let empty_rule = CrashSignature::new(None, Some("r".into()), None, Vec::new(), Vec::new());
        let empty_exception =
            CrashSignature::new(Some("r".into()), None, None, Vec::new(), Vec::new());
        assert_ne!(empty_rule.hash, empty_exception.hash);
    }

    // ---- Corpus adversário da 4ª revisão (relatórios reais, `tests/corpus-assinatura/`) ------

    fn corpus_assinatura_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("corpus-assinatura")
    }

    /// O loader e a versão do Minecraft pelo nome do caso (`fabric-1.21.1-…`), como o Warden
    /// passa os do pack.
    fn context_of_case(name: &str) -> AnalysisContext {
        let loader = if name.starts_with("neoforge-") {
            Some(LogLoader::NeoForge)
        } else if name.starts_with("forge-") {
            Some(LogLoader::Forge)
        } else if name.starts_with("fabric-") {
            Some(LogLoader::Fabric)
        } else {
            None
        };
        AnalysisContext {
            loader,
            minecraft: name.split('-').nth(1).map(ToOwned::to_owned),
        }
    }

    fn signature_of_files(name: &str, files: &[(String, Vec<u8>)]) -> CrashSignature {
        let sources = sources_from(files);
        let analysis = analyze(&sources, &context_of_case(name), None).unwrap();
        signature(&analysis, &sources)
    }

    /// `tests/corpus-assinatura/grupos.toml`: o grupo esperado de cada caso e, nos casos que
    /// provam uma regra, as partes.
    #[derive(Deserialize)]
    struct ExpectedGroups {
        caso: Vec<ExpectedCase>,
    }

    #[derive(Deserialize)]
    struct ExpectedCase {
        nome: String,
        grupo: String,
        #[serde(default)]
        regra: Option<String>,
        #[serde(default)]
        excecao: Option<String>,
        #[serde(default)]
        frame: Option<String>,
        #[serde(default)]
        mods: Option<Vec<String>>,
        #[serde(default)]
        sujeito: Option<Vec<String>>,
    }

    fn expected_groups() -> Vec<ExpectedCase> {
        let text = fs::read_to_string(corpus_assinatura_dir().join("grupos.toml")).unwrap();
        toml::from_str::<ExpectedGroups>(&text).unwrap().caso
    }

    fn corpus_assinatura_cases() -> Vec<PathBuf> {
        let mut cases: Vec<PathBuf> = fs::read_dir(corpus_assinatura_dir())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect();
        cases.sort();
        cases
    }

    #[test]
    fn corpus_adversario_cada_caso_tem_as_partes_e_o_grupo_esperados() {
        let expected = expected_groups();
        let cases = corpus_assinatura_cases();
        let names: Vec<String> = cases
            .iter()
            .map(|case| case.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        let listed: Vec<&str> = expected.iter().map(|case| case.nome.as_str()).collect();
        assert_eq!(
            names, listed,
            "todo caso do corpus está no grupos.toml, na ordem"
        );

        let mut signatures: Vec<(&ExpectedCase, CrashSignature)> = Vec::new();
        for (case, path) in expected.iter().zip(&cases) {
            let found = signature_of_files(&case.nome, &files_of(path));
            if let Some(rule) = &case.regra {
                assert_eq!(found.rule.as_deref(), Some(rule.as_str()), "{}", case.nome);
            }
            if let Some(exception) = &case.excecao {
                assert_eq!(
                    found.exception.as_deref(),
                    Some(exception.as_str()),
                    "{}",
                    case.nome
                );
            }
            if let Some(frame) = &case.frame {
                let frame = (!frame.is_empty()).then_some(frame.as_str());
                assert_eq!(found.frame.as_deref(), frame, "{}", case.nome);
            }
            if let Some(mods) = &case.mods {
                assert_eq!(&found.mods, mods, "{}", case.nome);
            }
            if let Some(subject) = &case.sujeito {
                assert_eq!(&found.subject, subject, "{}", case.nome);
            }
            signatures.push((case, found));
        }
        // Mesmo grupo ⇔ mesmo hash: nenhum par de travamentos diferentes junto, nenhum par do
        // mesmo travamento separado.
        for (index, (one, first)) in signatures.iter().enumerate() {
            for (other, second) in &signatures[index + 1..] {
                let same_group = one.grupo == other.grupo;
                assert_eq!(
                    same_group,
                    first.hash == second.hash,
                    "{} × {}\n{first:?}\n{second:?}",
                    one.nome,
                    other.nome
                );
            }
        }
    }

    #[test]
    fn corpus_adversario_o_mesmo_travamento_com_ruido_diferente_tem_a_mesma_assinatura() {
        for path in corpus_assinatura_cases() {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let files = files_of(&path);
            let original = signature_of_files(&name, &files);
            let changed: Vec<(String, Vec<u8>)> = files
                .iter()
                .map(|(file, bytes)| {
                    let text = String::from_utf8_lossy(bytes);
                    (file.clone(), another_run(file, &text).into_bytes())
                })
                .collect();
            assert_eq!(original, signature_of_files(&name, &changed), "{name}");
            assert!(original.rule.is_some(), "{name}: todo caso tem achado");
        }
    }

    // ---- Casos adversários da 4ª revisão (cada um falhava na ponta f6b6b0d) ------------------

    /// Crash report do Fabric com a descrição, a exceção e a pilha dadas, terminando no `main`.
    fn fabric_report(description: &str, exception: &str, frames: &str) -> String {
        format!(
            "---- Minecraft Crash Report ----
// Oops.

Time: 2026-10-13 12:00:01
Description: {description}

{exception}
{frames}\tat net.minecraft.client.main.Main.main(Main.java:250)
\tat net.fabricmc.loader.impl.game.minecraft.MinecraftGameProvider.launch(MinecraftGameProvider.java:514)
\tat net.fabricmc.loader.impl.launch.knot.Knot.launch(Knot.java:72)
\tat net.fabricmc.loader.impl.launch.knot.KnotClient.main(KnotClient.java:23)

-- System Details --
Details:
\tMinecraft Version: 1.20.1
"
        )
    }

    fn fabric_signature(description: &str, exception: &str, frames: &str) -> CrashSignature {
        signature_of(
            "crash",
            &fabric_report(description, exception, frames),
            LogLoader::Fabric,
            "1.20.1",
        )
    }

    const NPE_MOUSE: &str = "java.lang.NullPointerException: Cannot invoke \"net.minecraft.class_4174.method_19234()\" because the return value of \"net.minecraft.class_1792.method_19264()\" is null";

    #[test]
    fn adversario_metodo_fundido_pelo_mixin_tem_id_de_sessao_que_muda_a_cada_execucao() {
        // Fabric 1.20.1 real: a lambda de um mixin fundida na classe do jogo chama-se
        // `md<sessão>$<mod>$lambda$<método>$<n>$<índice>`; a sessão é o fim de um UUID que o
        // Mixin sorteia a cada execução (`MixinProcessor.sessionId`). Na ponta f6b6b0d o frame
        // nem contava como código de mod, e o nome inteiro (com a sessão) ia para a assinatura.
        let frames = |session: &str, mod_id: &str| {
            format!(
                "\tat net.minecraft.class_2370.md{session}${mod_id}$lambda$addAutomaticTagEntries$0$1(class_2370.java:1579)
\tat java.base/java.util.AbstractList$RandomAccessSpliterator.forEachRemaining(Unknown Source)
\tat net.minecraft.class_2370.localvar$bao000${mod_id}$addAutomaticTagEntries(class_2370.java:1556)
\tat knot//net.minecraft.class_2370.method_40257(class_2370.java)
\tat net.minecraft.class_310.method_1507(class_310.java:1080)
"
            )
        };
        let sign = |session: &str, mod_id: &str| {
            fabric_signature(
                "mouseClicked event handler",
                NPE_MOUSE,
                &frames(session, mod_id),
            )
        };
        let one = sign("6558fa", "autotag");
        assert_eq!(
            one.frame.as_deref(),
            Some("net.minecraft.class_2370.md$autotag$lambda$addAutomaticTagEntries")
        );
        // Outra sessão: o mesmo travamento.
        assert_eq!(one, sign("0c2b1d", "autotag"));
        // Outro mod com a mesma lambda: outro travamento.
        assert_ne!(one.hash, sign("6558fa", "outromod").hash);
        // Forge (sem o mod no nome) e o prefixo preservado, com e sem o id da config.
        for (raw, expected) in [
            (
                "net.minecraft.client.renderer.blockentity.BlockEntityRenderers.md23f78d$lambda$setEmf$Model$0$0",
                "net.minecraft.client.renderer.blockentity.BlockEntityRenderers.md$lambda$setEmf$Model",
            ),
            ("a.B.foo_$md$create$1a2b3c$0", "a.B.foo_$"),
            ("a.B.foo_$md$1a2b3c$a", "a.B.foo_$"),
            ("a.B.md1a2b3c$create$helper$2", "a.B.md$create$helper"),
        ] {
            assert_eq!(normalize_method(raw), expected, "{raw}");
        }
    }

    #[test]
    fn adversario_handler_com_id_de_mod_com_hifen_e_frame_de_mod() {
        // Fabric 1.21.1 real: `handler$ddo000$fabric-registry-sync-v0$disconnectAfter`. O
        // hífen do id do mod fazia a linha nem ser lida como frame: o travamento ficava com o
        // frame de outro mod mais abaixo (`owo`), e dois handlers de mods diferentes no mesmo
        // ponto davam o mesmo hash.
        let frames = |id: &str, mod_id: &str, handler: &str| {
            format!(
                "\tat knot//com.google.common.collect.Iterators$6.transform(Iterators.java:829)
\tat knot//com.google.common.collect.TransformedIterator.next(TransformedIterator.java:52)
\tat net.minecraft.class_2370.method_40257(class_2370.java:1279)
\tat knot//net.fabricmc.fabric.impl.registry.sync.RegistrySyncManager.unmap(RegistrySyncManager.java:367)
\tat net.minecraft.class_310.handler${id}${mod_id}${handler}(class_310.java:16191)
\tat net.minecraft.class_310.method_18096(class_310.java:2269)
\tat net.minecraft.class_310.wrapOperation$abc000$owo$passModifiersToBraid$mouseClicked(class_310.java:551)
"
            )
        };
        let sign = |id: &str, mod_id: &str, handler: &str| {
            fabric_signature(
                "mouseClicked event handler",
                "java.lang.NullPointerException: mouseClicked event handler",
                &frames(id, mod_id, handler),
            )
        };
        let one = sign("ddo000", "fabric-registry-sync-v0", "disconnectAfter");
        assert_eq!(
            one.frame.as_deref(),
            Some("net.minecraft.class_310.fabric-registry-sync-v0$disconnectAfter")
        );
        assert!(
            one.subject
                .contains(&"where=net.minecraft.class_2370.method_40257".to_owned()),
            "{:?}",
            one.subject
        );
        assert_eq!(
            one,
            sign("zzb001", "fabric-registry-sync-v0", "disconnectAfter")
        );
        assert_ne!(
            one.hash,
            sign("ddo000", "fabric-screen-api-v1", "beforeInit").hash
        );
    }

    #[test]
    fn adversario_excecao_sem_pilha_usa_a_pilha_do_head() {
        // Forge 1.20.1 real: `java.lang.ClassCastException: null` sem nenhum frame (a JVM omite
        // a pilha das exceções repetidas, `OmitStackTraceInFastThrow`); a pilha da thread está
        // na seção `-- Head --`. Na ponta f6b6b0d toda queda assim tinha o mesmo hash.
        let report = |top: &str| {
            format!(
                "---- Minecraft Crash Report ----
// You're mean.

Time: 2026-01-29 17:35:45
Description: Rendering overlay

java.lang.ClassCastException: null


A detailed walkthrough of the error, its code path and all known details is as follows:
---------------------------------------------------------------------------------------

-- Head --
Thread: Render thread
Suspected Mods: NONE
Stacktrace:
{top}\tat net.minecraft.client.Minecraft.m_91383_(Minecraft.java:1146) {CLIENT_1_20} {{re:mixin,re:classloading,pl:mixin:A}}
\tat net.minecraft.client.main.Main.main(Main.java:218) ~[forge-47.2.0.jar:?] {{re:classloading}}

-- Overlay render details --
Details:
\tOverlay name: net.minecraftforge.client.loading.ForgeLoadingOverlay

-- System Details --
Details:
\tMinecraft Version: 1.20.1
"
            )
        };
        let sign = |frames: &[(&str, &str, u32)], address: u32| {
            signature_of(
                "crash",
                &report(&forge_1_20_frames(frames, address)),
                LogLoader::Forge,
                "1.20.1",
            )
        };
        let render = sign(
            &[(
                "net.minecraft.client.renderer.GameRenderer.m_109093_",
                "GameRenderer",
                957,
            )],
            358,
        );
        let level = sign(
            &[(
                "net.minecraft.client.renderer.LevelRenderer.m_109599_",
                "LevelRenderer",
                1150,
            )],
            358,
        );
        assert_eq!(
            render.frame.as_deref(),
            Some("net.minecraft.client.renderer.GameRenderer.m_109093_")
        );
        assert_ne!(render.hash, level.hash);
        // Outra linha e outro endereço: o mesmo travamento.
        assert_eq!(
            render,
            sign(
                &[(
                    "net.minecraft.client.renderer.GameRenderer.m_109093_",
                    "GameRenderer",
                    961,
                )],
                400,
            )
        );
    }

    #[test]
    fn adversario_classe_escondida_do_java_8_nao_e_frame_de_mod() {
        // Java 8 (Forge 1.7.10–1.16.5): a classe de uma lambda imprime como
        // `Classe$$Lambda$12/1234567890.run(Unknown Source)`, com o identity hash depois da
        // barra, outro a cada execução. Lida como prefixo de módulo, o método virava
        // `1234567890.run`: um "frame de mod" diferente a cada vez. O mesmo vale para
        // `java.lang.invoke.LambdaForm$MH/0x…`.
        let sign = |hash: &str, handle: &str, mod_class: &str| {
            signature_of(
                "crash",
                &forge_legacy_report(
                    &[
                        "net.minecraft.client.renderer.EntityRenderer.func_78480_b(EntityRenderer.java:1180)",
                        &format!(
                            "net.minecraft.client.Minecraft$$Lambda$12/{hash}.run(Unknown Source)"
                        ),
                        &format!(
                            "java.lang.invoke.LambdaForm$MH/{handle}.invoke_MT(LambdaForm$MH)"
                        ),
                        &format!("{mod_class}.aoTick(Tick.java:10)"),
                    ],
                    "1.12.2",
                ),
                LogLoader::Forge,
                "1.12.2",
            )
        };
        let one = sign(
            "1234567890",
            "0x00007f3a1c2b3d40",
            "com.example.meumod.Tick",
        );
        assert_eq!(one.frame.as_deref(), Some("com.example.meumod.Tick.aoTick"));
        assert_eq!(
            one,
            sign("987654321", "0x00007f3a1c2b0000", "com.example.meumod.Tick")
        );
        assert_ne!(
            one.hash,
            sign(
                "1234567890",
                "0x00007f3a1c2b3d40",
                "com.example.outromod.Tick"
            )
            .hash
        );
    }

    #[test]
    fn adversario_contador_do_mixinextras_sai() {
        // Fabric 1.21.1 real: `wrapOperation$cjj000$fabric-registry-sync-v0$wrapIsServerCall$36`
        // e `mixinextras$bridge$method_25402$57`: o número conta as pontes geradas pelo
        // `MixinExtras`, na ordem em que as classes carregam.
        for (raw, expected) in [
            (
                "net.minecraft.class_310.wrapOperation$cjj000$fabric-registry-sync-v0$wrapIsServerCall$36",
                "net.minecraft.class_310.fabric-registry-sync-v0$wrapIsServerCall",
            ),
            (
                "net.minecraft.class_312.mixinextras$bridge$method_25402$57",
                "net.minecraft.class_312.mixinextras$bridge$method_25402",
            ),
            (
                "net.minecraft.class_757.wrapOperation$cfo000$iris$renderLevel$mixinextras$bridge$93",
                "net.minecraft.class_757.iris$renderLevel$mixinextras$bridge",
            ),
        ] {
            assert_eq!(normalize_method(raw), expected, "{raw}");
        }
        let frames = |counter: u32| {
            format!(
                "\tat net.minecraft.class_7655.wrapOperation$cjj000$fabric-registry-sync-v0$wrapIsServerCall${counter}(class_7655.java:111)
\tat net.minecraft.class_7655.method_45121(class_7655.java:111)
"
            )
        };
        let sign = |counter| {
            fabric_signature(
                "mouseClicked event handler",
                "java.lang.IllegalStateException: Failed to load registries due to above errors",
                &frames(counter),
            )
        };
        assert_eq!(sign(36), sign(41));
    }

    #[test]
    fn adversario_frame_do_launcher_depois_do_main_nao_e_frame_de_mod() {
        // Fabric 1.21.1 real (Modrinth App): a pilha termina em
        // `com.modrinth.theseus.MinecraftLaunch.relaunch`. Sem frame de mod, era ele o "frame
        // de mod" de todo travamento do pack, e quedas diferentes caíam juntas.
        const THESEUS: &str = "\tat com.modrinth.theseus.MinecraftLaunch.relaunch(MinecraftLaunch.java:63)\n\tat com.modrinth.theseus.MinecraftLaunch.main(MinecraftLaunch.java:28)\n";
        const PRISM: &str = "\tat org.prismlauncher.launcher.impl.StandardLauncher.launch(StandardLauncher.java:115)\n\tat org.prismlauncher.EntryPoint.listen(EntryPoint.java:129)\n";
        let report = |top: &str, launcher: &str| {
            format!(
                "---- Minecraft Crash Report ----
Description: mouseClicked event handler

java.lang.IllegalStateException: Failed to load registries due to above errors
{top}\tat net.minecraft.class_310.method_1507(class_310.java:1080)
\tat net.minecraft.client.main.Main.main(Main.java:250)
\tat net.fabricmc.loader.impl.launch.knot.KnotClient.main(KnotClient.java:23)
{launcher}
-- System Details --
Details:
\tMinecraft Version: 1.21.1
"
            )
        };
        let sign = |top: &str, launcher: &str| {
            signature_of("crash", &report(top, launcher), LogLoader::Fabric, "1.21.1")
        };
        let registries = sign(
            "\tat net.minecraft.class_7655.method_45121(class_7655.java:111)\n",
            THESEUS,
        );
        let worlds = sign(
            "\tat net.minecraft.class_32.method_27002(class_32.java:50)\n",
            THESEUS,
        );
        assert_eq!(
            registries.frame.as_deref(),
            Some("net.minecraft.class_7655.method_45121")
        );
        assert_ne!(registries.hash, worlds.hash);
        // Outro launcher, ou nenhum: o mesmo travamento.
        assert_eq!(
            registries,
            sign(
                "\tat net.minecraft.class_7655.method_45121(class_7655.java:111)\n",
                PRISM
            )
        );
        assert_eq!(
            registries,
            sign(
                "\tat net.minecraft.class_7655.method_45121(class_7655.java:111)\n",
                ""
            )
        );
    }

    #[test]
    fn adversario_biblioteca_com_jar_anotado_nao_e_frame_de_mod_e_a_queda_acima_do_embrulho_conta()
    {
        // Forge 1.20.1 real: `it.unimi.dsi.fastutil….removeFirstLong ~[fastutil-8.5.9.jar…]` no
        // topo e, embaixo, `MinecraftServer.wrapOperation$zee000$managedBlock` de um mod que
        // embrulha o laço do servidor. O jar anotado fazia da biblioteca um frame de mod, e sem
        // isso o embrulho seria o frame de toda queda do laço: quedas diferentes do jogo caíam
        // juntas.
        let report = |game: &str, address: u32| {
            let frames = format!(
                "\tat it.unimi.dsi.fastutil.longs.LongLinkedOpenHashSet.removeFirstLong(LongLinkedOpenHashSet.java:547) ~[fastutil-8.5.9.jar%23{address}!/:?] {{}}
{}\tat net.minecraft.server.MinecraftServer.wrapOperation$zee000$managedBlock(MinecraftServer.java:1856) {CLIENT_1_20} {{re:mixin,pl:accesstransformer:B,re:classloading,pl:accesstransformer:B,pl:mixin:A}}
\tat net.minecraft.server.MinecraftServer.m_130011_(MinecraftServer.java:661) {CLIENT_1_20} {{re:mixin,pl:accesstransformer:B}}
\tat java.base/java.lang.Thread.run(Thread.java:840) ~[?:?] {{}}",
                forge_1_20_frames(&[(game, "Level", 29)], address)
            );
            format!(
                "---- Minecraft Crash Report ----
Description: Exception in server tick loop

java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length 0
{frames}

-- System Details --
Details:
\tMinecraft Version: 1.20.1
"
            )
        };
        let sign = |game: &str, address: u32| {
            signature_of("crash", &report(game, address), LogLoader::Forge, "1.20.1")
        };
        let lighting = sign(
            "net.minecraft.world.level.lighting.LeveledPriorityQueue.m_278178_",
            122,
        );
        let distance = sign("net.minecraft.server.level.DistanceManager.m_140805_", 122);
        assert_eq!(
            lighting.frame.as_deref(),
            Some("net.minecraft.server.MinecraftServer.managedBlock")
        );
        assert!(
            lighting.subject.contains(
                &"where=net.minecraft.world.level.lighting.LeveledPriorityQueue.m_278178_"
                    .to_owned()
            ),
            "{:?}",
            lighting.subject
        );
        assert!(lighting.mods.is_empty(), "{:?}", lighting.mods);
        assert_ne!(lighting.hash, distance.hash);
        assert_eq!(
            lighting,
            sign(
                "net.minecraft.world.level.lighting.LeveledPriorityQueue.m_278178_",
                300
            )
        );
    }

    #[test]
    fn adversario_excecao_com_nome_intermediary_e_reconhecida() {
        // Fabric com nomes intermediary: `net.minecraft.class_151: Non [a-z0-9/._-] character in
        // path of location: minecraft:Foo Bar` (`InvalidIdentifierException`). O nome não termina
        // em `Exception`: sem a linha, a assinatura ficava só com a descrição, e quedas de mods
        // diferentes caíam juntas.
        let sign = |frame: &str| {
            fabric_signature(
                "Initializing game",
                "net.minecraft.class_151: Non [a-z0-9/._-] character in path of location: minecraft:Foo Bar",
                &format!(
                    "\tat net.minecraft.class_2960.<init>(class_2960.java:40)\n\tat {frame}\n"
                ),
            )
        };
        let alpha = sign("com.alpha.AlphaItems.register(AlphaItems.java:12)");
        let beta = sign("com.beta.BetaBlocks.register(BetaBlocks.java:7)");
        assert_eq!(alpha.exception.as_deref(), Some("net.minecraft.class_151"));
        assert_eq!(
            alpha.frame.as_deref(),
            Some("com.alpha.AlphaItems.register")
        );
        assert_ne!(alpha.hash, beta.hash);
        assert_eq!(
            alpha,
            sign("com.alpha.AlphaItems.register(AlphaItems.java:30)")
        );
    }

    #[test]
    fn adversario_falha_de_mixin_sem_achado_proprio_leva_a_config_no_sujeito() {
        // Fabric 1.21.1 real (`Description: Bootstrap`): `ExceptionInInitializerError` →
        // `RuntimeException: Mixin transformation of net.minecraft.class_1754 failed` →
        // `MixinTransformerError` → `InjectionError: Critical injection failure: … in
        // create_dragons_plus.mixins.json:minecraft.BottleItemMixin from mod … failed injection
        // check`. Nenhuma linha `MixinApplyError` nem `Mixin apply failed` para o catálogo; os
        // frames são do transformador: falhas de configs diferentes davam o mesmo hash.
        let report = |target: &str, deepest: &str| {
            format!(
                "---- Minecraft Crash Report ----
Description: Bootstrap

java.lang.ExceptionInInitializerError
\tat net.minecraft.class_2246.<clinit>(class_2246.java:150)
\tat net.minecraft.class_310.method_1507(class_310.java:1080)
\tat net.minecraft.client.main.Main.main(Main.java:250)
Caused by: java.lang.RuntimeException: Mixin transformation of {target} failed
\tat net.fabricmc.loader.impl.launch.knot.KnotClassDelegate.getPostMixinClassByteArray(KnotClassDelegate.java:427)
\tat net.fabricmc.loader.impl.launch.knot.KnotClassDelegate.tryLoadClass(KnotClassDelegate.java:323)
\t... 3 more
Caused by: org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError: An unexpected critical error was encountered
\tat org.spongepowered.asm.mixin.transformer.MixinProcessor.applyMixins(MixinProcessor.java:392)
\t... 5 more
Caused by: {deepest}
\tat org.spongepowered.asm.mixin.injection.struct.InjectionInfo.postInject(InjectionInfo.java:531)
\tat org.spongepowered.asm.mixin.transformer.MixinTargetContext.applyInjections(MixinTargetContext.java:1370)
\t... 7 more

-- System Details --
Details:
\tMinecraft Version: 1.21.1
"
            )
        };
        let injection = |config: &str| {
            format!(
                "org.spongepowered.asm.mixin.injection.throwables.InjectionError: Critical injection failure: Callback method use$checkDragonBreathFluid(Ljava/lang/Object;)Z in {config}:minecraft.BottleItemMixin from mod meumod failed injection check, (0/1) succeeded. Scanned 0 target(s). No refMap loaded."
            )
        };
        let sign = |target: &str, deepest: &str| {
            signature_of(
                "crash",
                &report(target, deepest),
                LogLoader::Fabric,
                "1.21.1",
            )
        };
        let dragons = sign(
            "net.minecraft.class_1754",
            &injection("create_dragons_plus.mixins.json"),
        );
        let sodium = sign("net.minecraft.class_1754", &injection("sodium.mixins.json"));
        assert_eq!(dragons.rule.as_deref(), Some("crash_report.description"));
        assert!(
            dragons
                .subject
                .contains(&"mixin_config=create_dragons_plus.mixins.json".to_owned()),
            "{:?}",
            dragons.subject
        );
        assert_ne!(dragons.hash, sodium.hash);
        // Mensagem sem a config: o alvo da transformação.
        let shadow = "org.spongepowered.asm.mixin.transformer.throwables.InvalidMixinException: @Shadow field statTabIndex was not located in the target class";
        let bottle = sign("net.minecraft.class_1754", shadow);
        let buffer = sign("net.minecraft.class_287", shadow);
        assert!(
            bottle
                .subject
                .contains(&"mixin_target=net.minecraft.class_1754".to_owned()),
            "{:?}",
            bottle.subject
        );
        assert_ne!(bottle.hash, buffer.hash);
    }

    #[test]
    fn adversario_suppressed_fica_fora_da_causa_e_das_secoes() {
        // Fabric 1.21.1 real: `RuntimeException: Could not execute entrypoint…` lista um
        // `Suppressed: NoSuchMethodError` por mod que falhou (com os frames dele) antes do
        // `Caused by:`. Quando a causa mais profunda da cadeia tem uma exceção suprimida com a
        // própria causa (`\tCaused by:`, indentada, depois dela), essa causa aninhada não é a
        // causa mais profunda, e os frames das suprimidas não são o frame do travamento. Na ponta
        // f6b6b0d o último `Caused by:` do texto (o aninhado) era a causa.
        let report = |suppressed_mod: &str, cause_frame: &str| {
            format!(
                "---- Minecraft Crash Report ----
Description: Initializing game

java.lang.RuntimeException: Could not execute entrypoint stage 'main' due to errors, provided by 'alpha' at 'com.alpha.Alpha'!
\tat net.fabricmc.loader.impl.FabricLoaderImpl.lambda$invokeEntrypoints$2(FabricLoaderImpl.java:411)
\tat net.minecraft.client.main.Main.main(Main.java:250)
\tSuppressed: java.lang.NoSuchMethodError: 'void x.Y.z()'
\t\tat knot//{suppressed_mod}.onInitialize(Outro.java:32)
\t\tat net.fabricmc.loader.impl.FabricLoaderImpl.invokeEntrypoints(FabricLoaderImpl.java:405)
\t\t... 3 more
Caused by: java.lang.IllegalStateException: estado
\tat knot//{cause_frame}(Alpha.java:5)
\tat knot//com.alpha.Alpha.onInitialize(Alpha.java:20)
\t... 2 more
\tSuppressed: java.io.IOException: ao fechar o recurso
\t\tat knot//{suppressed_mod}.fechar(Outro.java:40)
\t\t... 4 more
\tCaused by: java.net.SocketException: dentro da suprimida
\t\tat knot//{suppressed_mod}.ler(Outro.java:9)
\t\t... 5 more

-- System Details --
Details:
\tMinecraft Version: 1.21.1
"
            )
        };
        let sign = |suppressed_mod: &str, cause_frame: &str| {
            signature_of(
                "crash",
                &report(suppressed_mod, cause_frame),
                LogLoader::Fabric,
                "1.21.1",
            )
        };
        let one = sign("io.github.outro.OutroMod", "com.alpha.Alpha.init");
        assert_eq!(
            one.exception.as_deref(),
            Some("java.lang.IllegalStateException")
        );
        assert_eq!(one.frame.as_deref(), Some("com.alpha.Alpha.init"));
        // Outro mod nas suprimidas: o mesmo travamento. Outro frame na causa: outro.
        assert_eq!(
            one,
            sign("io.github.terceiro.TerceiroMod", "com.alpha.Alpha.init")
        );
        assert_ne!(
            one.hash,
            sign("io.github.outro.OutroMod", "com.alpha.Alpha.registrar").hash
        );
    }

    #[test]
    fn adversario_falta_de_memoria_nao_depende_do_mod_que_alocou_por_ultimo() {
        // Forge 1.12.2 real: `OutOfMemoryError: GC overhead limit exceeded` com o OptiFine no
        // topo da pilha. Quem alocou por último muda; a causa (memória) é a mesma.
        let log = |frame: &str| {
            format!(
                "[12:00:00] [Client thread/ERROR]: Erro\njava.lang.OutOfMemoryError: GC overhead limit exceeded\n\tat {frame}\n\tat net.minecraft.client.Minecraft.run(Minecraft.java:10)\n"
            )
        };
        let sign = |frame: &str| of(&[source("latest.log", SourceKind::LatestLog, &log(frame))]);
        let optifine = sign("net.optifine.shaders.Shaders.loadShaderPack(Shaders.java:1)");
        let game =
            sign("net.minecraft.client.renderer.RenderGlobal.loadRenderers(RenderGlobal.java:2)");
        assert_eq!(optifine.rule.as_deref(), Some("memory.out_of_memory"));
        assert_eq!(optifine.frame, None);
        assert_eq!(optifine, game);
    }

    #[test]
    fn frame_nativo_com_endereco_sem_modulo_e_stub() {
        let lines = |frame: &str| {
            source(
                "hs_err_pid1.log",
                SourceKind::JvmCrash,
                &format!("#\n# A fatal error has been detected by the Java Runtime Environment:\n#\n# Problematic frame:\n# {frame}\n"),
            )
            .clean_lines()
        };
        // Endereço sem módulo (chamada por ponteiro inválido): outro a cada execução.
        assert_eq!(
            native_frame(&lines("C  0x00007ff8a1b2c3d4"), 3).as_deref(),
            Some("C #")
        );
        assert_eq!(
            native_frame(&lines("v  ~StubRoutines::jbyte_disjoint_arraycopy"), 3).as_deref(),
            Some("v ~StubRoutines::jbyte_disjoint_arraycopy")
        );
    }
}
