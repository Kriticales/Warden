//! O que o raio-x lê de cada jar (camadas A e B da R5A §5; ARCHITECTURE §9.6).
//!
//! Tudo aqui depende só dos bytes do jar e do loader do pack, então pode ficar no cache
//! ([`crate::cache`]). O cruzamento entre mods ([`crate::overlap`]) usa estes dados mais o que
//! é do pack (configs, lista curada, último travamento).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Tipo de alteração: o injetor do Mixin ou do `MixinExtras` que a classe usa.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    /// `@Overwrite`: troca o método inteiro.
    Overwrite,
    /// `@Inject`: acrescenta código num ponto.
    Inject,
    /// `@Redirect`: troca uma chamada ou um acesso a campo.
    Redirect,
    /// `@ModifyArg`: muda um argumento de uma chamada.
    ModifyArg,
    /// `@ModifyArgs`: muda todos os argumentos de uma chamada.
    ModifyArgs,
    /// `@ModifyVariable`: muda uma variável local.
    ModifyVariable,
    /// `@ModifyConstant`: troca um valor fixo do código.
    ModifyConstant,
    /// `@WrapOperation` (MixinExtras): envolve uma chamada; vários se encadeiam.
    WrapOperation,
    /// `@ModifyExpressionValue` (MixinExtras): muda o resultado de uma expressão.
    ModifyExpressionValue,
    /// `@ModifyReturnValue` (MixinExtras): muda o valor devolvido.
    ModifyReturnValue,
    /// `@WrapWithCondition` (MixinExtras): pode cancelar uma chamada.
    WrapWithCondition,
    /// `@ModifyReceiver` (MixinExtras): muda o objeto de uma chamada.
    ModifyReceiver,
    /// `@WrapMethod` (MixinExtras): envolve o método inteiro.
    WrapMethod,
}

impl ChangeKind {
    /// Todos, na ordem do enum.
    pub const ALL: [Self; 13] = [
        Self::Overwrite,
        Self::Inject,
        Self::Redirect,
        Self::ModifyArg,
        Self::ModifyArgs,
        Self::ModifyVariable,
        Self::ModifyConstant,
        Self::WrapOperation,
        Self::ModifyExpressionValue,
        Self::ModifyReturnValue,
        Self::WrapWithCondition,
        Self::ModifyReceiver,
        Self::WrapMethod,
    ];

    /// O nome da anotação, como o autor escreve (`Redirect`).
    #[must_use]
    pub fn annotation(self) -> &'static str {
        match self {
            Self::Overwrite => "Overwrite",
            Self::Inject => "Inject",
            Self::Redirect => "Redirect",
            Self::ModifyArg => "ModifyArg",
            Self::ModifyArgs => "ModifyArgs",
            Self::ModifyVariable => "ModifyVariable",
            Self::ModifyConstant => "ModifyConstant",
            Self::WrapOperation => "WrapOperation",
            Self::ModifyExpressionValue => "ModifyExpressionValue",
            Self::ModifyReturnValue => "ModifyReturnValue",
            Self::WrapWithCondition => "WrapWithCondition",
            Self::ModifyReceiver => "ModifyReceiver",
            Self::WrapMethod => "WrapMethod",
        }
    }

    /// Posição em [`ChangeKind::ALL`] (formato do cache).
    pub(crate) fn code(self) -> u8 {
        // `ALL` tem 13 itens: sempre cabe.
        u8::try_from(Self::ALL.iter().position(|k| *k == self).unwrap_or(0)).unwrap_or(0)
    }

    pub(crate) fn from_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code)).copied()
    }

    /// O injetor pelo nome interno da anotação (`org/spongepowered/asm/mixin/Overwrite`,
    /// `com/llamalad7/mixinextras/injector/wrapoperation/WrapOperation`). Os do `MixinExtras`
    /// valem também embutidos com outro pacote (`.../mixinextras/...` depois de um prefixo).
    #[must_use]
    pub fn from_annotation(type_name: &str) -> Option<Self> {
        let simple = type_name.rsplit('/').next().unwrap_or(type_name);
        let sponge = type_name.starts_with("org/spongepowered/asm/mixin/");
        let extras = type_name.contains("mixinextras/");
        let kind = match simple {
            "Overwrite" if sponge => Self::Overwrite,
            "Inject" if sponge => Self::Inject,
            "Redirect" if sponge => Self::Redirect,
            "ModifyArg" if sponge => Self::ModifyArg,
            "ModifyArgs" if sponge => Self::ModifyArgs,
            "ModifyVariable" if sponge => Self::ModifyVariable,
            "ModifyConstant" if sponge => Self::ModifyConstant,
            "WrapOperation" if extras => Self::WrapOperation,
            "ModifyExpressionValue" if extras => Self::ModifyExpressionValue,
            "ModifyReturnValue" if extras => Self::ModifyReturnValue,
            "WrapWithCondition" if extras => Self::WrapWithCondition,
            "ModifyReceiver" if extras => Self::ModifyReceiver,
            "WrapMethod" if extras => Self::WrapMethod,
            _ => return None,
        };
        Some(kind)
    }

    /// Troca código do jogo (em vez de só acrescentar): `@Overwrite`, `@Redirect`,
    /// `@ModifyConstant`. Dois destes no mesmo ponto: só um vale.
    #[must_use]
    pub fn replaces(self) -> bool {
        matches!(
            self,
            Self::Overwrite | Self::Redirect | Self::ModifyConstant
        )
    }

    /// Muda um valor escolhido por seletor (`@ModifyVariable`, `@ModifyArg`, `@ModifyArgs`).
    #[must_use]
    pub fn modifies_value(self) -> bool {
        matches!(
            self,
            Self::ModifyVariable | Self::ModifyArg | Self::ModifyArgs
        )
    }
}

/// O ponto dentro do método: o `@At` (tipo e alvo), já traduzido pelo refmap.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct At {
    /// Tipo do ponto (`HEAD`, `RETURN`, `INVOKE`, `FIELD`…); `OVERWRITE`, `CONSTANT` e
    /// `WRAP_METHOD` para os injetores sem `@At`.
    pub value: String,
    /// Alvo do ponto (`Lnet/minecraft/class_1;method_2()V`) ou, no `@ModifyConstant`, a
    /// constante; vazio quando não há.
    pub target: String,
}

impl At {
    /// O ponto fica dentro do corpo do método (não no começo nem no fim): uma chamada, um
    /// campo, uma constante, um `new` ou um salto. Um `@Overwrite` de outro mod pode tirá-lo.
    #[must_use]
    pub fn in_body(&self) -> bool {
        matches!(
            self.value.as_str(),
            "INVOKE" | "INVOKE_ASSIGN" | "INVOKE_STRING" | "FIELD" | "CONSTANT" | "NEW" | "JUMP"
        )
    }
}

/// Uma alteração: um injetor (ou `@Overwrite`) aplicado a um método de uma classe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// Tipo.
    pub kind: ChangeKind,
    /// Classe alterada, nome interno (`net/minecraft/class_1`), como o jogo a chama em tempo de
    /// execução (já traduzida pelo refmap).
    pub target_class: String,
    /// Método alterado, sem dono nem descritor, como em tempo de execução (`method_2`).
    pub method: String,
    /// O método como o autor escreveu (antes do refmap): o nome legível quando o mod foi
    /// escrito com nomes do Yarn ou da Mojang. Vazio quando é igual a [`Change::method`].
    pub method_named: String,
    /// O ponto.
    pub at: At,
    /// Que variável ou argumento o `@ModifyVariable`/`@ModifyArg` escolhe (`ordinal`, `index`,
    /// `name`, `argsOnly` e o tipo); `*` no `@ModifyArgs` (todos); vazio nos outros.
    pub selector: String,
    /// `require` do injetor, se declarado.
    pub require: Option<i32>,
}

/// Uma classe de mixin, com o que ela altera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinClass {
    /// Nome interno (`me/jellysquid/mods/lithium/mixin/world/X`).
    pub name: String,
    /// Config de mixin que a declara (`lithium.mixins.json`).
    pub config: String,
    /// A config tem `plugin` (o próprio mod pode desligar a classe).
    pub plugin: bool,
    /// Prioridade (da `@Mixin` ou da config; padrão 1000). Menor aplica antes.
    pub priority: i32,
    /// `injectors.defaultRequire` da config.
    pub default_require: Option<i32>,
    /// Classes-alvo (nomes internos, em tempo de execução).
    pub targets: Vec<String>,
    /// Alterações.
    pub changes: Vec<Change>,
    /// Classes citadas pela classe (constantes e descritores), para o rebaixador "referencia
    /// classes do outro mod".
    pub refs: Vec<String>,
}

/// Tipo de relação declarada, só o que o raio-x usa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DepKind {
    /// Obrigatória.
    Required,
    /// Opcional, recomendada ou sugerida (convivência prevista pelo autor).
    Optional,
}

/// Relação declarada com outro mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredDep {
    /// Id do outro mod.
    pub id: String,
    /// Tipo.
    pub kind: DepKind,
}

/// O que deu errado ao ler um jar. Nada disso interrompe a análise.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum IssueKind {
    /// O jar não é um zip legível.
    UnreadableJar,
    /// Config declarada que não está neste jar (costuma estar em outro; o loader acha).
    MissingConfig,
    /// Config com JSON inválido.
    InvalidConfig,
    /// Classe listada na config que não está no jar.
    MissingClass,
    /// Classe ilegível (corrompida, ofuscada de um jeito que o leitor não entende).
    UnreadableClass,
    /// Classe listada que não tem `@Mixin`.
    NotAMixin,
    /// Algum limite de tamanho passou (arquivo, classe ou jar embutido grande demais).
    TooLarge,
}

/// Um problema de leitura, com o arquivo dentro do jar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanIssue {
    /// Tipo.
    pub kind: IssueKind,
    /// Arquivo dentro do jar (`lithium.mixins.json`, `a/b/C.class`).
    pub file: String,
}

/// Um mod lido: o jar de cima ou um embutido.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedMod {
    /// Caminho dentro do jar de cima: vazio para ele; `META-INF/jars/a.jar` para um embutido;
    /// `a.jar!/b.jar` para embutido de embutido.
    pub path: String,
    /// Id do mod para o loader do pack; sem descritor, `?<nome do arquivo>`.
    pub mod_id: String,
    /// Versão declarada.
    pub version: String,
    /// Mod Fabric num pack Forge/NeoForge (Sinytra Connector): os nomes são *intermediary* e não
    /// se comparam com os dos mods nativos (camada D, P2).
    pub connector: bool,
    /// Relações declaradas.
    pub deps: Vec<DeclaredDep>,
    /// Pacotes Java com classes neste jar (`a/b/c`).
    pub packages: Vec<String>,
    /// Configs de mixin lidas.
    pub configs: Vec<String>,
    /// Classes de mixin.
    pub classes: Vec<MixinClass>,
    /// Anotações contadas que não são alterações (`Accessor`, `Invoker`, `Shadow`).
    pub other_annotations: BTreeMap<String, u32>,
    /// Problemas de leitura.
    pub issues: Vec<ScanIssue>,
    /// Textos das classes de plugin, config e compatibilidade do mod (para "o plugin ou a config
    /// do mod cita o outro mod").
    pub plugin_strings: Vec<String>,
    /// Opções de outros mods que este desliga no descritor (`lithium:mixin.chunk.palette`).
    pub disables: Vec<String>,
    /// Quantos jars embutidos este jar traz (desempate entre cópias repetidas).
    pub nested_count: u32,
    /// O jar traz o `MixinSquared` (um mod pode cancelar mixins de outro).
    pub mixin_squared: bool,
    /// Nomes traduzidos que os refmaps deste jar ensinam: nome em tempo de execução →
    /// nome legível (classes `net/minecraft/class_1` e métodos `method_2`/`m_2_`).
    pub names: Vec<(String, String)>,
}

impl ScannedMod {
    /// Quantas alterações o mod faz.
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.classes.iter().map(|c| c.changes.len()).sum()
    }
}

/// O que o raio-x leu de um jar do pack.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JarScan {
    /// O jar de cima (o primeiro, caminho vazio) e os embutidos, em profundidade.
    pub mods: Vec<ScannedMod>,
}
