//! Propriedades da redação (ARCHITECTURE §9.4) e da análise com entradas aleatórias.
//!
//! - Nenhum texto aleatório faz um segredo conhecido sobreviver: nome de usuário, jogador,
//!   computador (4+ caracteres, em qualquer lugar, sem diferenciar maiúsculas), chave do cofre,
//!   UUID do jogador, IP e e-mail inseridos em posições aleatórias.
//! - A redação nunca entra em pânico e é idempotente nos dados pessoais.
//! - A análise de qualquer texto nunca entra em pânico e todo achado tem evidência que aponta uma
//!   linha existente.

use proptest::prelude::*;
use secrecy::SecretString;
use warden_diagnostics::redact::offline_uuid;
use warden_diagnostics::{AnalysisContext, Evidence, RedactionProfile, Redactor, analyze_text};

/// Nome com letras acentuadas, 4 a 12 caracteres, que não aparece nos marcadores.
fn name() -> impl Strategy<Value = String> {
    "[A-Za-zÁÉÍÓÚÃÕÇáéíóúãõç][a-záéíóúãõç0-9]{3,11}".prop_filter(
        "não pode ser parte de um marcador",
        |name| {
            let lower = name.to_lowercase();
            ![
                "<usuário>",
                "<jogador>",
                "<computador>",
                "<segredo>",
                "<email>",
                "<uuid>",
                "<ip>",
            ]
            .iter()
            .any(|marker| marker.contains(&lower))
        },
    )
}

fn noise() -> impl Strategy<Value = String> {
    proptest::collection::vec(any::<char>(), 0..40).prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn nenhum_segredo_conhecido_sobrevive(
        user in name(),
        player in name(),
        computer in name(),
        secret in "[A-Za-z0-9$./_-]{16,40}",
        octets in (1u8..=223, 0u8..=255, 0u8..=255, 1u8..=254),
        pieces in proptest::collection::vec(noise(), 8),
        upper in any::<bool>(),
    ) {
        prop_assume!(octets.0 != 127);
        let ip = format!("{}.{}.{}.{}", octets.0, octets.1, octets.2, octets.3);
        let uuid = offline_uuid(&player);
        let email = format!("{}@exemplo.com.br", user.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "x"));
        let shown_user = if upper { user.to_uppercase() } else { user.clone() };
        let text = format!(
            "{}C:\\Users\\{shown_user}\\AppData{}Setting user: {player}{} {computer} {}{secret}{} {ip} {}{uuid}{} {email} {}",
            pieces[0], pieces[1], pieces[2], pieces[3], pieces[4], pieces[5], pieces[6], pieces[7],
        );
        let profile = RedactionProfile::new()
            .with_username(&user)
            .with_player(&player)
            .with_computer_name(&computer)
            .with_secret(&SecretString::from(secret.clone()));
        let output = Redactor::new(profile).redact(&text).text;
        let lower = output.to_lowercase();
        for known in [&user, &player, &computer] {
            prop_assert!(!lower.contains(&known.to_lowercase()), "{known:?} sobrou em {output:?}");
        }
        prop_assert!(!output.contains(&secret), "segredo sobrou em {output:?}");
        prop_assert!(!lower.contains(&uuid), "uuid sobrou em {output:?}");
        prop_assert!(!output.contains(&format!(" {ip} ")), "ip sobrou em {output:?}");
        prop_assert!(!output.contains(&email), "e-mail sobrou em {output:?}");
    }

    #[test]
    fn redacao_nunca_falha_e_e_idempotente(text in noise(), user in name()) {
        let redactor = Redactor::new(RedactionProfile::new().with_username(&user));
        let once = redactor.redact(&text).text;
        let twice = redactor.redact(&once).text;
        prop_assert!(!once.to_lowercase().contains(&user.to_lowercase()));
        prop_assert!(!twice.to_lowercase().contains(&user.to_lowercase()));
    }

    #[test]
    fn analise_de_texto_qualquer_tem_evidencia_valida(
        lines in proptest::collection::vec(
            prop_oneof![
                noise(),
                Just("---- Minecraft Crash Report ----".to_owned()),
                Just("Description: Ticking entity".to_owned()),
                Just("java.lang.OutOfMemoryError: Java heap space".to_owned()),
                Just("\t - Mod 'A' (a) 1.0 requires any version of b, which is missing!".to_owned()),
                Just("Caused by: java.lang.NullPointerException".to_owned()),
                Just("\tat TRANSFORMER/x@1/net.X.m(X.java:1)".to_owned()),
                Just("Mod a requires b 1.0 or above".to_owned()),
                Just("Currently, b is not installed".to_owned()),
                Just("<log4j:Message><![CDATA[Couldn't place player in world]]></log4j:Message>".to_owned()),
            ],
            0..60,
        ),
    ) {
        let text = lines.join("\n");
        let analysis = analyze_text("x.log", &text, &AnalysisContext::default()).unwrap();
        let count = text.split('\n').count();
        for entry in &analysis.findings {
            prop_assert!(!entry.finding.evidence().is_empty());
            for evidence in entry.finding.evidence() {
                let Evidence::Log { file, line, .. } = evidence else {
                    return Err(TestCaseError::fail("evidência sem log"));
                };
                prop_assert_eq!(file.as_str(), "x.log");
                prop_assert!(*line >= 1 && (*line as usize) <= count);
            }
        }
    }
}

#[test]
fn ca_t14_04_maria_jogador_ip_e_chave_nao_aparecem() {
    let key = "$2a$10$abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0";
    let log = format!(
        "[main/INFO]: Loading C:\\Users\\Maria\\AppData\\Roaming\\.minecraft\n\
         [Render thread/INFO]: Setting user: Steve_BR\n\
         [Server thread/INFO]: Steve_BR[/192.168.0.15:51234] logged in with entity id 7\n\
         [main/INFO]: chave {key}\n"
    );
    // Sem perfil: tudo vem do próprio texto (como num log de outro computador).
    let output = Redactor::new(RedactionProfile::new()).redact(&log).text;
    for secret in ["Maria", "Steve_BR", "192.168.0.15", "$2a$10$"] {
        assert!(!output.contains(secret), "{secret} sobrou: {output}");
    }
}
