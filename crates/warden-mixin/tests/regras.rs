//! Regras de risco, rebaixadores e elevadores com jars sintéticos (CA-T07-05; ARCHITECTURE §9.6).

mod common;

use common::{
    Ann, EXTRAS_WRAP, JarBuilder, MixinClass, SPONGE, Value, at, fabric_mod_json, injector,
    mixin_config, s, strs,
};
use warden_jarmeta::{Limits, Loader};
use warden_mixin::model::JarScan;
use warden_mixin::overlap::{
    CrashCitation, CrossContext, CuratedPair, Demoter, Elevator, Overlap, Risk, Rule,
};
use warden_mixin::pack::{PackScan, PackXray, ScanStats};
use warden_mixin::scan::scan_jar_bytes;

const TARGET: &str = "net/minecraft/class_1";
const CALL: &str = "Lnet/minecraft/class_2;method_3()V";

/// Um mod Fabric com as classes dadas (nomes simples, no pacote `x/<id>/mixin`).
struct Mod {
    id: &'static str,
    classes: Vec<MixinClass>,
    plugin: bool,
    depends: Vec<&'static str>,
    version: &'static str,
    extra: Vec<MixinClass>,
}

impl Mod {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            classes: Vec::new(),
            plugin: false,
            depends: Vec::new(),
            version: "1.0",
            extra: Vec::new(),
        }
    }

    fn package(&self) -> String {
        format!("x/{}/mixin", self.id)
    }

    fn class(mut self, simple: &str, build: impl FnOnce(MixinClass) -> MixinClass) -> Self {
        let class = MixinClass::new(&format!("{}/{simple}", self.package()), TARGET);
        self.classes.push(build(class));
        self
    }

    fn plugin(mut self) -> Self {
        self.plugin = true;
        self
    }

    fn depends(mut self, id: &'static str) -> Self {
        self.depends.push(id);
        self
    }

    /// Classe comum do mod (fora das configs), como um plugin ou uma config.
    fn extra(mut self, class: MixinClass) -> Self {
        self.extra.push(class);
        self
    }

    fn jar(&self) -> Vec<u8> {
        let config = format!("{}.mixins.json", self.id);
        let package = format!("{}/", self.package());
        let simple: Vec<String> = self
            .classes
            .iter()
            .map(|c| c.name.trim_start_matches(&package).replace('/', "."))
            .collect();
        let simple: Vec<&str> = simple.iter().map(String::as_str).collect();
        let plugin = self.plugin.then(|| format!("x.{}.Plugin", self.id));
        let mut jar = JarBuilder::new()
            .file(
                "fabric.mod.json",
                fabric_mod_json(self.id, self.version, &[&config], &self.depends, &[]),
            )
            .file(
                &config,
                mixin_config(
                    &self.package().replace('/', "."),
                    &simple,
                    plugin.as_deref(),
                ),
            );
        for class in self.classes.iter().chain(&self.extra) {
            jar = jar.class(class);
        }
        jar.build()
    }

    fn scan(&self) -> (String, JarScan) {
        let item = format!("mods/{}.pw.toml", self.id);
        let scan = scan_jar_bytes(
            &self.jar(),
            &format!("{}.jar", self.id),
            Loader::Fabric,
            &Limits::default(),
        );
        (item, scan)
    }
}

fn xray(mods: &[Mod], context: &CrossContext) -> PackXray {
    let scan = PackScan {
        items: mods.iter().map(Mod::scan).collect(),
        stats: ScanStats::default(),
    };
    PackXray::build(scan, context)
}

fn only(xray: &PackXray) -> &Overlap {
    assert_eq!(xray.overlaps.len(), 1, "{:#?}", xray.overlaps);
    &xray.overlaps[0]
}

fn redirect(c: MixinClass) -> MixinClass {
    c.method(
        "r",
        "()V",
        injector("Redirect", "method_9", at("INVOKE", CALL)),
    )
}

fn inject(at_value: &str) -> impl Fn(MixinClass) -> MixinClass {
    let at_value = at_value.to_owned();
    move |c| {
        let target = if at_value == "INVOKE" { CALL } else { "" };
        c.method(
            "i",
            "()V",
            injector("Inject", "method_9", at(&at_value, target)),
        )
    }
}

#[test]
fn dois_redirect_no_mesmo_at_e_alto() {
    let x = xray(
        &[
            Mod::new("a").class("M", redirect),
            Mod::new("b").class("M", redirect),
        ],
        &CrossContext::default(),
    );
    let o = only(&x);
    assert_eq!(
        (o.rule, o.base, o.risk),
        (Rule::TwoReplacements, Risk::High, Risk::High)
    );
    assert_eq!(o.participants.len(), 2);
    assert_eq!(o.participants[0].config, "a.mixins.json");
    assert_eq!(o.participants[0].at.value, "INVOKE");
    assert!(o.demoters.is_empty() && o.elevators.is_empty());
}

#[test]
fn redirect_em_chamadas_diferentes_nao_disputa() {
    let other = |c: MixinClass| {
        c.method(
            "r",
            "()V",
            injector("Redirect", "method_9", at("INVOKE", "La;b()V")),
        )
    };
    let x = xray(
        &[
            Mod::new("a").class("M", redirect),
            Mod::new("b").class("M", other),
        ],
        &CrossContext::default(),
    );
    assert_eq!(only(&x).risk, Risk::Low);
}

#[test]
fn dois_overwrite_e_alto() {
    let x = xray(
        &[
            Mod::new("a").class("M", |c| c.overwrite("method_9")),
            Mod::new("b").class("M", |c| c.overwrite("method_9")),
        ],
        &CrossContext::default(),
    );
    assert_eq!(
        (only(&x).rule, only(&x).risk),
        (Rule::TwoOverwrites, Risk::High)
    );
}

#[test]
fn overwrite_mais_injecao_no_corpo_e_medio() {
    let x = xray(
        &[
            Mod::new("a").class("M", |c| c.overwrite("method_9")),
            Mod::new("b").class("M", inject("INVOKE")),
        ],
        &CrossContext::default(),
    );
    let o = only(&x);
    assert_eq!((o.rule, o.risk), (Rule::OverwriteAndBody, Risk::Medium));
    // `defaultRequire = 1` na config da injeção: fica como evidência, sem subir o risco.
    assert!(
        matches!(&o.elevators[..], [Elevator::Require { config }] if config == "b.mixins.json")
    );
}

#[test]
fn overwrite_mais_inject_no_comeco_e_baixo() {
    let x = xray(
        &[
            Mod::new("a").class("M", |c| c.overwrite("method_9")),
            Mod::new("b").class("M", inject("HEAD")),
        ],
        &CrossContext::default(),
    );
    assert_eq!(
        (only(&x).rule, only(&x).risk),
        (Rule::OverwriteAndEdge, Risk::Low)
    );
}

#[test]
fn so_inject_e_baixo() {
    let x = xray(
        &[
            Mod::new("a").class("M", inject("HEAD")),
            Mod::new("b").class("M", inject("HEAD")),
            Mod::new("c").class("M", inject("INVOKE")),
        ],
        &CrossContext::default(),
    );
    let o = only(&x);
    assert_eq!((o.rule, o.risk), (Rule::Coexist, Risk::Low));
    assert_eq!(o.items.len(), 3);
}

#[test]
fn redirect_mais_inject_no_mesmo_ponto_e_baixo_e_com_mixinextras_e_medio() {
    let x = xray(
        &[
            Mod::new("a").class("M", redirect),
            Mod::new("b").class("M", inject("INVOKE")),
        ],
        &CrossContext::default(),
    );
    assert_eq!(
        (only(&x).rule, only(&x).risk),
        (Rule::RedirectAndInject, Risk::Low)
    );

    let wrap = |c: MixinClass| {
        c.method(
            "w",
            "()V",
            Ann::new(EXTRAS_WRAP)
                .with("method", strs(&["method_9"]))
                .with("at", at("INVOKE", CALL)),
        )
    };
    let x = xray(
        &[
            Mod::new("a").class("M", redirect),
            Mod::new("b").class("M", wrap),
        ],
        &CrossContext::default(),
    );
    assert_eq!(
        (only(&x).rule, only(&x).risk),
        (Rule::RedirectAndOther, Risk::Medium)
    );
}

#[test]
fn modify_variable_cruza_so_no_mesmo_seletor() {
    let modify = |ordinal: i32| {
        move |c: MixinClass| {
            c.method(
                "v",
                "(I)I",
                injector("ModifyVariable", "method_9", at("HEAD", ""))
                    .with("ordinal", Value::Int(ordinal)),
            )
        }
    };
    let x = xray(
        &[
            Mod::new("a").class("M", modify(0)),
            Mod::new("b").class("M", modify(0)),
        ],
        &CrossContext::default(),
    );
    assert_eq!(
        (only(&x).rule, only(&x).risk),
        (Rule::TwoValueChanges, Risk::Medium)
    );
    let x = xray(
        &[
            Mod::new("a").class("M", modify(0)),
            Mod::new("b").class("M", modify(1)),
        ],
        &CrossContext::default(),
    );
    assert_eq!(only(&x).risk, Risk::Low, "variáveis diferentes");

    // `@ModifyArgs` mexe em todos os argumentos: cruza com qualquer `@ModifyArg` da mesma chamada.
    let arg = |c: MixinClass| {
        c.method(
            "a",
            "(I)I",
            injector("ModifyArg", "method_9", at("INVOKE", CALL)).with("index", Value::Int(2)),
        )
    };
    let args = |c: MixinClass| {
        c.method(
            "b",
            "(Ljava/lang/Object;)V",
            injector("ModifyArgs", "method_9", at("INVOKE", CALL)),
        )
    };
    let x = xray(
        &[
            Mod::new("a").class("M", arg),
            Mod::new("b").class("M", args),
        ],
        &CrossContext::default(),
    );
    assert_eq!(only(&x).risk, Risk::Medium);
}

#[test]
fn plugin_em_todos_desce_um_nivel_e_diz_o_motivo() {
    let x = xray(
        &[
            Mod::new("a").plugin().class("M", redirect),
            Mod::new("b").plugin().class("M", redirect),
        ],
        &CrossContext::default(),
    );
    let o = only(&x);
    assert_eq!((o.base, o.risk), (Risk::High, Risk::Medium));
    assert_eq!(o.demoters, vec![Demoter::AllPlugin]);

    // Só um com plugin: não vale.
    let x = xray(
        &[
            Mod::new("a").plugin().class("M", redirect),
            Mod::new("b").class("M", redirect),
        ],
        &CrossContext::default(),
    );
    assert_eq!(only(&x).risk, Risk::High);
}

#[test]
fn mixin_que_referencia_classes_do_outro_e_compatibilidade_intencional() {
    let b = Mod::new("b")
        .class("M", redirect)
        .extra(MixinClass::plain("x/b/api/Api"));
    let a = Mod::new("a").class("M", |c| redirect(c).refs("x/b/api/Api"));
    let x = xray(&[a, b], &CrossContext::default());
    let o = only(&x);
    assert_eq!(o.risk, Risk::Medium);
    assert!(
        matches!(&o.demoters[..], [Demoter::ReferencesOther { item, other }]
        if item == "mods/a.pw.toml" && other == "mods/b.pw.toml")
    );
}

#[test]
fn pacote_compat_desce() {
    let a = Mod::new("a");
    let compat = MixinClass::new("x/a/mixin/compat/b/M", TARGET);
    let a = Mod {
        classes: vec![redirect(compat)],
        ..a
    };
    let x = xray(
        &[a, Mod::new("b").class("M", redirect)],
        &CrossContext::default(),
    );
    assert!(
        matches!(&only(&x).demoters[..], [Demoter::CompatPackage { mixin_class, .. }] if mixin_class == "x/a/mixin/compat/b/M")
    );
}

#[test]
fn dependencia_declarada_desce_mas_nao_quando_e_biblioteca() {
    let x = xray(
        &[
            Mod::new("a").depends("b").class("M", redirect),
            Mod::new("b").class("M", redirect),
        ],
        &CrossContext::default(),
    );
    assert!(matches!(
        &only(&x).demoters[..],
        [Demoter::DeclaresDependency { .. }]
    ));
    assert_eq!(only(&x).risk, Risk::Medium);

    // Debugify × Fabric API (S-R5-1 §5.3): depender da biblioteca não é convivência prevista.
    let x = xray(
        &[
            Mod::new("debugify")
                .depends("fabric-api")
                .class("M", redirect),
            Mod::new("fabric-api").class("M", redirect),
        ],
        &CrossContext::default(),
    );
    assert!(only(&x).demoters.is_empty());
    assert_eq!(only(&x).risk, Risk::High);
}

#[test]
fn plugin_ou_config_que_cita_o_outro_mod_desce() {
    let a = Mod::new("aaa")
        .class("M", redirect)
        .extra(MixinClass::plain("x/aaa/AaaConfig").string("bbb"));
    let x = xray(
        &[a, Mod::new("bbb").class("M", redirect)],
        &CrossContext::default(),
    );
    assert!(
        matches!(&only(&x).demoters[..], [Demoter::PluginCitesOther { item, .. }] if item == "mods/aaa.pw.toml")
    );
}

#[test]
fn par_da_lista_curada_desce_com_a_origem() {
    let context = CrossContext {
        known_compatible: vec![CuratedPair {
            first: "b".into(),
            second: "a".into(),
            label: "S-R5-1".into(),
        }],
        ..CrossContext::default()
    };
    let x = xray(
        &[
            Mod::new("a").class("M", redirect),
            Mod::new("b").class("M", redirect),
        ],
        &context,
    );
    assert_eq!(
        only(&x).demoters,
        vec![Demoter::KnownCompatible {
            origin: "S-R5-1".into()
        }]
    );
    assert_eq!(only(&x).risk, Risk::Medium);
}

#[test]
fn cada_rebaixador_desce_um_nivel() {
    // Lithium × ModernFix (S-R5-1 §5.3): todos com plugin e o plugin cita o outro → baixo.
    let a = Mod::new("modernfix")
        .plugin()
        .class("M", |c| c.overwrite("method_9"))
        .extra(MixinClass::plain("x/modernfix/Plugin").string("lithium"));
    let b = Mod::new("lithium")
        .plugin()
        .class("M", |c| c.overwrite("method_9"));
    let x = xray(&[a, b], &CrossContext::default());
    let o = only(&x);
    assert_eq!(o.demoters.len(), 2, "{:?}", o.demoters);
    assert_eq!((o.base, o.risk), (Risk::High, Risk::Low));
}

#[test]
fn opcao_desligada_por_outro_mod_some_do_cruzamento() {
    // Krypton × Lithium (S-R5-1 §5.3): o Krypton desliga `mixin.world.player_chunk_tick`.
    let lithium = Mod::new("lithium");
    let class =
        MixinClass::new("x/lithium/mixin/world/player_chunk_tick/M", TARGET).overwrite("method_9");
    let lithium = Mod {
        classes: vec![class],
        ..lithium
    };
    let krypton = Mod::new("krypton").class("M", |c| c.overwrite("method_9"));
    // O descritor do Krypton de verdade, com `lithium:options`.
    let real = std::fs::read_to_string(common::corpus(
        "fabric-1.20.1/krypton-0.2.3/fabric.mod.json",
    ))
    .expect("corpus");
    assert!(real.contains("mixin.world.player_chunk_tick"));
    let real = real.replace(r#""mixins": ["#, r#""mixins": ["krypton.mixins.json","#);
    let krypton_jar = JarBuilder::new()
        .file("fabric.mod.json", real)
        .file(
            "krypton.mixins.json",
            mixin_config("x.krypton.mixin", &["M"], None),
        )
        .class(&krypton.classes[0])
        .build();
    let scan = PackScan {
        items: vec![
            lithium.scan(),
            (
                "mods/krypton.pw.toml".into(),
                scan_jar_bytes(
                    &krypton_jar,
                    "krypton.jar",
                    Loader::Fabric,
                    &Limits::default(),
                ),
            ),
        ],
        stats: ScanStats::default(),
    };
    let x = PackXray::build(scan, &CrossContext::default());
    assert!(x.overlaps.is_empty(), "{:#?}", x.overlaps);
    assert!(x.disabled.contains("lithium:mixin.world.player_chunk_tick"));
}

#[test]
fn opcao_desligada_pela_config_do_pack_some() {
    let lithium = Mod::new("lithium");
    let class = MixinClass::new("x/lithium/mixin/ai/nearby_entity_tracking/M", TARGET)
        .overwrite("method_9");
    let lithium = Mod {
        classes: vec![class],
        ..lithium
    };
    let other = Mod::new("b").class("M", |c| c.overwrite("method_9"));
    let config_dir = common::corpus("fabric-1.20.1/config");
    let text = std::fs::read_to_string(config_dir.join("lithium.properties")).expect("corpus");
    assert!(
        text.contains("mixin.ai.nearby_entity_tracking=false"),
        "{text}"
    );
    let context = CrossContext {
        disabled: warden_mixin::options::disabled_by_pack_configs(
            &config_dir,
            warden_mixin::options::options_data(),
        ),
        ..CrossContext::default()
    };
    assert!(xray(&[lithium, other], &context).overlaps.is_empty());
}

#[test]
fn travamento_que_cita_a_config_sobe_para_alto_com_a_linha() {
    let citation = CrashCitation {
        config: "b.mixins.json".into(),
        file: "output.log".into(),
        line: 120,
        excerpt: "Mixin [b.mixins.json:M] from phase [DEFAULT] in config [b.mixins.json] FAILED"
            .into(),
    };
    let context = CrossContext {
        crash: vec![citation.clone()],
        ..CrossContext::default()
    };
    let x = xray(
        &[
            Mod::new("a").plugin().class("M", redirect),
            Mod::new("b").plugin().class("M", redirect),
        ],
        &context,
    );
    let o = only(&x);
    assert_eq!(
        (o.base, o.risk),
        (Risk::High, Risk::High),
        "o rebaixador não vence o travamento"
    );
    assert_eq!(o.elevators, vec![Elevator::Crash(citation)]);

    // Travamento que cita outra config não muda nada.
    let context = CrossContext {
        crash: vec![CrashCitation {
            config: "z.mixins.json".into(),
            file: "output.log".into(),
            line: 1,
            excerpt: String::new(),
        }],
        ..CrossContext::default()
    };
    let x = xray(
        &[
            Mod::new("a").class("M", inject("HEAD")),
            Mod::new("b").class("M", inject("HEAD")),
        ],
        &context,
    );
    assert_eq!(only(&x).risk, Risk::Low);
}

#[test]
fn conflito_conhecido_sobe_para_alto() {
    let context = CrossContext {
        known_conflicts: vec![CuratedPair {
            first: "a".into(),
            second: "b".into(),
            label: "a-b".into(),
        }],
        ..CrossContext::default()
    };
    let x = xray(
        &[
            Mod::new("a").class("M", inject("HEAD")),
            Mod::new("b").class("M", inject("HEAD")),
        ],
        &context,
    );
    assert_eq!(only(&x).risk, Risk::High);
    assert_eq!(
        only(&x).elevators,
        vec![Elevator::KnownConflict { id: "a-b".into() }]
    );
}

#[test]
fn prioridade_ordena_os_envolvidos() {
    let x = xray(
        &[
            Mod::new("a").class("M", |c| redirect(c).priority(1000)),
            Mod::new("b").class("M", |c| redirect(c).priority(700)),
        ],
        &CrossContext::default(),
    );
    let o = only(&x);
    assert_eq!(o.participants[0].mod_id, "b", "menor aplica antes");
    assert_eq!(o.participants[0].priority, 700);
}

#[test]
fn mesmo_mod_nunca_sobrepoe_a_si_mesmo() {
    let x = xray(
        &[Mod::new("a").class("M", redirect).class("N", redirect)],
        &CrossContext::default(),
    );
    assert!(x.overlaps.is_empty());
}

#[test]
fn alvo_por_texto_e_traduzido_pelo_refmap() {
    // `@Mixin(targets = "…")` e `method` com o nome legível, traduzidos pelo refmap do jar.
    let mut class = MixinClass::new("x/a/mixin/M", TARGET);
    class.targets.clear();
    class.string_targets = vec!["net/minecraft/client/Keyboard".into()];
    let class = class.method("r", "()V", injector("Inject", "onKey", at("HEAD", "")));
    let refmap = r#"{"mappings": {"x/a/mixin/M": {"net/minecraft/client/Keyboard": "net/minecraft/class_309", "onKey": "method_1466(JIIII)V"}}}"#;
    let jar = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json("a", "1", &["a.mixins.json"], &[], &[]),
        )
        .file(
            "a.mixins.json",
            r#"{"package": "x.a.mixin", "mixins": ["M"], "refmap": "a-refmap.json"}"#,
        )
        .file("a-refmap.json", refmap)
        .class(&class)
        .build();
    let scan = scan_jar_bytes(&jar, "a.jar", Loader::Fabric, &Limits::default());
    let change = &scan.mods[0].classes[0].changes[0];
    assert_eq!(change.target_class, "net/minecraft/class_309");
    assert_eq!(change.method, "method_1466");
    assert_eq!(change.method_named, "onKey");
    assert!(scan.mods[0].names.contains(&(
        "net/minecraft/class_309".into(),
        "net/minecraft/client/Keyboard".into()
    )));
    assert!(
        scan.mods[0]
            .names
            .contains(&("method_1466".into(), "onKey".into()))
    );

    // No cruzamento, a parte do jogo sai com o nome legível.
    let other = Mod::new("b");
    let other_class = MixinClass::new("x/b/mixin/M", "net/minecraft/class_309").method(
        "i",
        "()V",
        injector("Inject", "method_1466", at("HEAD", "")),
    );
    let other = Mod {
        classes: vec![other_class],
        ..other
    };
    let x = PackXray::build(
        PackScan {
            items: vec![("mods/a.pw.toml".into(), scan), other.scan()],
            stats: ScanStats::default(),
        },
        &CrossContext::default(),
    );
    assert_eq!(only(&x).label(), "Keyboard.onKey");
    let _ = (s("x"), SPONGE);
}
