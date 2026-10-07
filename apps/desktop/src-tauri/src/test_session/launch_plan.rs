//! Como o jogo do teste abre (SPEC T13 "Memória automática"; ARCHITECTURE §7.7): memória,
//! argumentos da JVM do teste e as opções da linha de comando. Funções puras: os golden tests
//! deste módulo passam a saída pelo montador da `warden-launcher` e comparam a linha final.

use std::path::PathBuf;

use serde::Serialize;
use warden_launcher::{JavaRuntime, LaunchOptions, OfflineProfile};

use crate::settings::TestMemory;

/// Coletor padrão do perfil do teste.
///
/// Motivo (S-R5-2 §5.3; ARCHITECTURE §7.7): no Java 25 com os coletores Parallel ou Serial, o
/// contador de memória usada da JVM só muda nas coletas, e a faixa de desempenho mostraria um
/// valor velho entre elas. Com o G1 a leitura é contínua em todos os Javas do teste (8 a 25).
/// Quando os argumentos do usuário já escolhem um coletor, o Warden não acrescenta nada.
pub(crate) const DEFAULT_GC: &str = "-XX:+UseG1GC";

/// Memória automática: abaixo de 100 mods.
const AUTO_SMALL_MB: u32 = 4096;
/// Memória automática: de 100 a 199 mods.
const AUTO_MEDIUM_MB: u32 = 6144;
/// Memória automática: 200 mods ou mais.
const AUTO_LARGE_MB: u32 = 8192;
/// Parte da memória do computador que a memória automática nunca passa.
const AUTO_MAX_SHARE_PERCENT: u64 = 60;
/// Menor memória automática, mesmo num computador com pouca memória.
const AUTO_FLOOR_MB: u32 = 1024;
/// Acima disto, o Java 8 costuma pausar demais nas coletas (aviso do SPEC T13).
pub(crate) const JAVA8_WARN_ABOVE_MB: u32 = 8192;

/// A memória escolhida para o teste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryChoice {
    /// `-Xmx`, em MB.
    pub(crate) mb: u32,
    /// Se foi o Warden que escolheu (Automático).
    pub(crate) auto: bool,
}

/// A memória do teste: os Ajustes do teste do pack mandam; com "Automático" vale a de
/// Configurações; com as duas automáticas, o Warden escolhe pela quantidade de mods, sem
/// passar de 60% da memória do computador (quando ela é conhecida).
pub(crate) fn choose_memory(
    pack: TestMemory,
    global: TestMemory,
    mods: usize,
    total_mb: Option<u64>,
) -> MemoryChoice {
    for choice in [pack, global] {
        if let TestMemory::Fixed { mb } = choice {
            return MemoryChoice { mb, auto: false };
        }
    }
    let wanted = match mods {
        0..100 => AUTO_SMALL_MB,
        100..200 => AUTO_MEDIUM_MB,
        _ => AUTO_LARGE_MB,
    };
    let mb = total_mb.map_or(wanted, |total| {
        let share = total.saturating_mul(AUTO_MAX_SHARE_PERCENT) / 100;
        // Múltiplo de 256 MB, para o valor ficar redondo na tela.
        let cap = u32::try_from(share / 256 * 256).unwrap_or(u32::MAX);
        wanted.min(cap).max(AUTO_FLOOR_MB)
    });
    MemoryChoice { mb, auto: true }
}

/// Separa os argumentos como o shell faria: espaços separam, aspas (`"` ou `'`) agrupam.
/// Igual ao `splitArgs` do diálogo Ajustes do teste (P1-08); aspas sem par fecham no fim.
pub(crate) fn split_jvm_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for character in text.chars() {
        if let Some(open) = quote {
            if character == open {
                quote = None;
            } else {
                current.push(character);
            }
            continue;
        }
        if character == '"' || character == '\'' {
            quote = Some(character);
            started = true;
        } else if character.is_whitespace() {
            if started {
                args.push(std::mem::take(&mut current));
            }
            started = false;
        } else {
            current.push(character);
            started = true;
        }
    }
    if started {
        args.push(current);
    }
    args
}

/// Se o argumento escolhe o coletor de memória (`-XX:+UseZGC`, `-XX:+UseParallelGC`…).
fn selects_collector(arg: &str) -> bool {
    arg.strip_prefix("-XX:+Use")
        .is_some_and(|rest| rest.ends_with("GC") && rest.len() > 2)
}

/// Os argumentos da JVM do teste: o coletor padrão (quando o usuário não escolhe outro)
/// seguido dos argumentos do usuário, na ordem em que ele escreveu.
pub(crate) fn test_jvm_args(user_args: &str) -> Vec<String> {
    let user = split_jvm_args(user_args);
    let mut args = Vec::with_capacity(user.len() + 1);
    if !user.iter().any(|arg| selects_collector(arg)) {
        args.push(DEFAULT_GC.to_owned());
    }
    args.extend(user);
    args
}

/// O que é preciso para montar as opções da abertura.
#[derive(Debug, Clone)]
pub(crate) struct PlanInput {
    /// `instances/<id>/minecraft`.
    pub(crate) game_dir: PathBuf,
    /// `instances/<id>/state`.
    pub(crate) state_dir: PathBuf,
    /// O Java.
    pub(crate) java: JavaRuntime,
    /// Memória.
    pub(crate) memory_mb: u32,
    /// Argumentos da JVM dos Ajustes do teste, como o usuário escreveu.
    pub(crate) user_jvm_args: String,
    /// Jogador offline.
    pub(crate) player: OfflineProfile,
}

/// As opções da abertura do teste normal: sem entrada direta (os perfis com Quick Play são da
/// L-08) e sem propriedades extras (o `fabric.noGui` é só da busca do culpado e da matriz: no
/// teste normal a janela de erro do Fabric ajuda quem está olhando).
pub(crate) fn launch_options(input: PlanInput) -> LaunchOptions {
    LaunchOptions {
        game_dir: input.game_dir,
        state_dir: input.state_dir,
        java: input.java,
        memory_mb: input.memory_mb,
        extra_jvm_args: test_jvm_args(&input.user_jvm_args),
        system_props: Vec::new(),
        quick_play: None,
        quick_play_path: None,
        player: input.player,
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::path::Path;

    use warden_launcher::InstalledGame;
    use warden_launcher::command::{TargetOs, build};

    use super::*;

    #[test]
    fn memoria_automatica_pela_quantidade_de_mods() {
        let auto = TestMemory::Auto;
        let big = Some(64 * 1024);
        assert_eq!(choose_memory(auto, auto, 0, big).mb, 4096);
        assert_eq!(choose_memory(auto, auto, 99, big).mb, 4096);
        assert_eq!(choose_memory(auto, auto, 100, big).mb, 6144);
        assert_eq!(choose_memory(auto, auto, 199, big).mb, 6144);
        assert_eq!(choose_memory(auto, auto, 200, big).mb, 8192);
        assert_eq!(choose_memory(auto, auto, 900, None).mb, 8192);
        assert!(choose_memory(auto, auto, 10, big).auto);
    }

    #[test]
    fn memoria_automatica_nunca_passa_de_60_por_cento() {
        let auto = TestMemory::Auto;
        // 8 GB de RAM: 60% = 4915 MB → 4864 (múltiplo de 256).
        assert_eq!(choose_memory(auto, auto, 300, Some(8192)).mb, 4864);
        // 16 GB: 9830 → 9728; o pack grande pede 8192, que cabe.
        assert_eq!(choose_memory(auto, auto, 300, Some(16_384)).mb, 8192);
        // Computador minúsculo: o piso de 1 GB.
        assert_eq!(choose_memory(auto, auto, 5, Some(1024)).mb, 1024);
    }

    #[test]
    fn valor_fixo_do_pack_vence_o_de_configuracoes() {
        let pack = TestMemory::Fixed { mb: 3000 };
        let global = TestMemory::Fixed { mb: 10_000 };
        let choice = choose_memory(pack, global, 500, Some(4096));
        assert_eq!(
            choice,
            MemoryChoice {
                mb: 3000,
                auto: false
            }
        );
        let choice = choose_memory(TestMemory::Auto, global, 1, Some(4096));
        assert_eq!(
            choice,
            MemoryChoice {
                mb: 10_000,
                auto: false
            },
            "valor fixo não é limitado: a pessoa escolheu"
        );
    }

    #[test]
    fn separa_argumentos_com_aspas() {
        assert_eq!(
            split_jvm_args(r#"  -Xss2M "-Dcaminho=C:\Meus Jogos" '-Da=b c'  -XX:+UseZGC "#),
            vec![
                "-Xss2M",
                r"-Dcaminho=C:\Meus Jogos",
                "-Da=b c",
                "-XX:+UseZGC"
            ]
        );
        assert!(split_jvm_args("   ").is_empty());
        assert_eq!(split_jvm_args(r#""""#), vec![String::new()]);
        assert_eq!(split_jvm_args(r#""sem fim"#), vec!["sem fim"]);
    }

    #[test]
    fn g1_so_quando_o_usuario_nao_escolhe_coletor() {
        assert_eq!(test_jvm_args(""), vec![DEFAULT_GC]);
        assert_eq!(
            test_jvm_args("-Xss4M -Dx=1"),
            vec![DEFAULT_GC, "-Xss4M", "-Dx=1"]
        );
        for collector in [
            "-XX:+UseZGC",
            "-XX:+UseParallelGC",
            "-XX:+UseSerialGC",
            "-XX:+UseShenandoahGC",
            "-XX:+UseConcMarkSweepGC",
            "-XX:+UseG1GC",
        ] {
            let args = test_jvm_args(&format!("-Xss4M {collector}"));
            assert_eq!(args, vec!["-Xss4M", collector], "{collector}");
        }
        // Desligar um coletor não é escolher outro.
        assert_eq!(
            test_jvm_args("-XX:-UseG1GC"),
            vec![DEFAULT_GC, "-XX:-UseG1GC"]
        );
        // `-XX:+UseStringDeduplication` não é coletor.
        assert_eq!(
            test_jvm_args("-XX:+UseStringDeduplication"),
            vec![DEFAULT_GC, "-XX:+UseStringDeduplication"]
        );
    }

    // ---- Golden tests da linha de comando (critério da L-04) ----

    /// Plano gravado de uma instalação real (fixtures da L-02/L-05, com `<SHARED>`).
    fn installed(name: &str) -> InstalledGame {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/warden-launcher/tests/fixtures/matriz")
            .join(format!("{name}.json"));
        let text = std::fs::read_to_string(&path).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    fn input(major: u32, user_jvm_args: &str) -> PlanInput {
        PlanInput {
            game_dir: PathBuf::from(r"C:\Warden\instances\PACK\minecraft"),
            state_dir: PathBuf::from(r"C:\Warden\instances\PACK\state"),
            java: JavaRuntime {
                java: PathBuf::from(format!(
                    r"C:\Warden\shared\runtimes\temurin-{major}\bin\java.exe"
                )),
                launcher: PathBuf::from(format!(
                    r"C:\Warden\shared\runtimes\temurin-{major}\bin\javaw.exe"
                )),
                major,
            },
            memory_mb: 6144,
            user_jvm_args: user_jvm_args.to_owned(),
            player: OfflineProfile::from_name("WardenTest").unwrap(),
        }
    }

    fn render(game: &InstalledGame, major: u32, user_jvm_args: &str) -> String {
        let command = build(
            game,
            &launch_options(input(major, user_jvm_args)),
            TargetOs::Windows,
        )
        .unwrap();
        let mut text = String::new();
        writeln!(text, "# {} (Java {major})", game.spec).unwrap();
        writeln!(text, "program: {}", command.program.display()).unwrap();
        writeln!(text, "cwd: {}", command.cwd.display()).unwrap();
        for arg in &command.args {
            writeln!(text, "{arg}").unwrap();
        }
        text
    }

    fn check_golden(file: &str, actual: &str) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/test_session/golden")
            .join(file);
        if std::env::var_os("WARDEN_TEST_REGRAVAR").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, actual).unwrap();
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| {
                panic!(
                    "{} ausente; rode com WARDEN_TEST_REGRAVAR=1",
                    path.display()
                )
            })
            .replace("\r\n", "\n");
        assert_eq!(actual, expected, "linha de comando diferente de {file}");
    }

    /// Perfil padrão: `-XX:+UseG1GC` entra depois dos argumentos do perfil do jogo e antes da
    /// classe principal (Fabric 1.20.1 com Java 17 e Forge 1.12.2 com Java 8).
    #[test]
    fn golden_perfil_padrao_com_g1() {
        let fabric = render(&installed("fabric-1.20.1"), 17, "");
        assert_eq!(fabric.matches("-XX:+UseG1GC").count(), 1);
        check_golden("fabric-1.20.1-padrao.txt", &fabric);
        let forge = render(&installed("forge-1.12.2"), 8, "");
        assert_eq!(forge.matches("-XX:+UseG1GC").count(), 1);
        check_golden("forge-1.12.2-padrao.txt", &forge);
    }

    /// Com um coletor escolhido nos argumentos do usuário, o G1 não entra.
    #[test]
    fn golden_coletor_do_usuario_sem_g1() {
        let fabric = render(
            &installed("fabric-1.20.1"),
            21,
            "-XX:+UseZGC -XX:+ZGenerational -Dteste=\"com espaço\"",
        );
        assert!(!fabric.contains("-XX:+UseG1GC"));
        assert!(fabric.contains("\n-XX:+UseZGC\n-XX:+ZGenerational\n-Dteste=com espaço\n"));
        check_golden("fabric-1.20.1-zgc.txt", &fabric);
        let forge = render(&installed("forge-1.12.2"), 8, "-XX:+UseParallelGC");
        assert!(!forge.contains("-XX:+UseG1GC"));
        check_golden("forge-1.12.2-parallel.txt", &forge);
    }
}
