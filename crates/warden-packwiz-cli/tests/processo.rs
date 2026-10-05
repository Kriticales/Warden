//! Execução de processos com o executável falso (`src/bin/packwiz_fake.rs`): argv e ambiente
//! recebidos, chave da CurseForge, entrada padrão, árvore de processos no cancelamento e no
//! tempo-limite, filhos órfãos, código de saída e pós-condições.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use common::{CapturedLogs, fake_binary, fake_pack, packwiz};
use secrecy::SecretString;
use warden_core::{
    CancellationToken, CoreErrorCode, DomainCode, DomainError, Progress, ProgressSink,
};
use warden_packwiz_cli::{
    BINARY_NAME, CURSEFORGE_KEY_ENV, CurseForgeTarget, Error, ExportSide, INHERITED_ENV, Packwiz,
    PackwizCliErrorCode, PackwizCommand, RunContext, Timeouts,
};

/// Chave falsa, com cara de chave da CurseForge, só destes testes.
const FAKE_KEY: &str = "$2a$10$chaveFalsaDosTestesDaP102xyzABCDEFGHIJKLMNOPQRSTUV";

fn dump(pack: &Path) -> Vec<String> {
    fs::read_to_string(pack.join("fake-dump.txt"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// O filho do executável falso já morreu? No Windows, o filho segura `fake-lock.txt` sem
/// compartilhamento: só dá para apagá-lo depois que o processo acabou. No Linux, confere
/// `/proc/<pid>`.
fn child_is_dead(pack: &Path) -> bool {
    if cfg!(windows) {
        fs::remove_file(pack.join("fake-lock.txt")).is_ok()
    } else {
        let pid = fs::read_to_string(pack.join("fake-ready.txt")).unwrap();
        // Sem `/proc/<pid>` ou zumbi (`State: Z`) conta como morto.
        fs::read_to_string(format!("/proc/{}/status", pid.trim()))
            .map_or(true, |status| status.contains("State:\tZ"))
    }
}

/// Espera o filho morrer, por no máximo `limit` (a morte pelo job é assíncrona no sistema).
fn wait_child_dead(pack: &Path, limit: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        if child_is_dead(pack) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[tokio::test]
async fn ca_5_chave_nunca_no_argv_nem_nos_registros() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    // O falso imprime a chave (como um programa descuidado faria) para provar a redação.
    fake_pack(
        &pack,
        &format!("mode=dump\nprint=eco: {FAKE_KEY}\nfile=mods/falso.pw.toml\n"),
    );
    let packwiz = packwiz(fake_binary(), temp.path());
    let key = SecretString::from(FAKE_KEY.to_owned());
    let logs = CapturedLogs::default();
    let _guard = logs.install();
    let seen = Mutex::new(Vec::new());
    let on_line = |line: &str| seen.lock().unwrap().push(line.to_owned());
    let cancel = CancellationToken::new();
    let report = packwiz
        .curseforge_add(
            &pack,
            &CurseForgeTarget::Ids {
                addon_id: 238_222,
                file_id: 9_009_995,
            },
            Some(&key),
            RunContext::new(&cancel).with_lines(&on_line),
        )
        .await
        .unwrap();
    assert_eq!(report.metafiles, ["mods/falso.pw.toml"]);

    let dump = dump(&pack);
    let args: Vec<&str> = dump
        .iter()
        .filter_map(|line| line.strip_prefix("arg="))
        .collect();
    let config = temp.path().join("dados/packwiz/packwiz.toml");
    let cache = temp.path().join("dados/cache/packwiz");
    assert_eq!(
        args,
        [
            "--config",
            config.to_str().unwrap(),
            "--cache",
            cache.to_str().unwrap(),
            "--pack-file",
            "pack.toml",
            "curseforge",
            "add",
            "--addon-id",
            "238222",
            "--file-id",
            "9009995",
        ]
    );
    // A chave chegou só pelo ambiente.
    assert!(
        dump.contains(&format!("env={CURSEFORGE_KEY_ENV}={FAKE_KEY}")),
        "{dump:?}"
    );
    assert!(!args.iter().any(|arg| arg.contains(FAKE_KEY)));
    // Resposta `n` à pergunta das dependências.
    assert!(dump.contains(&"stdin=n\\n".to_owned()), "{dump:?}");
    // Nada da chave nos registros, nas linhas guardadas nem nas entregues ao contexto.
    let logs = logs.text();
    assert!(logs.contains("iniciando o packwiz"), "{logs}");
    assert!(logs.contains("eco: [chave omitida]"), "{logs}");
    assert!(!logs.contains(FAKE_KEY));
    assert!(!logs.contains("chaveFalsa"));
    assert!(
        report
            .output
            .lines
            .contains(&"eco: [chave omitida]".to_owned())
    );
    assert!(
        !report
            .output
            .lines
            .iter()
            .any(|line| line.contains(FAKE_KEY))
    );
    assert!(
        !seen
            .lock()
            .unwrap()
            .iter()
            .any(|line| line.contains(FAKE_KEY))
    );
}

#[tokio::test]
async fn ambiente_limpo_e_sem_chave_fora_da_curseforge() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    fake_pack(&pack, "mode=dump\n");
    let packwiz = packwiz(fake_binary(), temp.path());
    let key = SecretString::from(FAKE_KEY.to_owned());
    let cancel = CancellationToken::new();
    let output = packwiz
        .run(
            &pack,
            &PackwizCommand::Refresh { build: true },
            Some(&key),
            RunContext::new(&cancel),
        )
        .await
        .unwrap();
    assert_eq!(output.exit_code, Some(0));
    let dump = dump(&pack);
    let args: Vec<&str> = dump
        .iter()
        .filter_map(|line| line.strip_prefix("arg="))
        .collect();
    assert_eq!(args[args.len() - 2..], ["refresh", "--build"]);
    // Só as variáveis permitidas. Com a cobertura (`cargo llvm-cov`), o próprio executável
    // falso instrumentado cria `__LLVM_PROFILE_RT_INIT_ONCE` ao iniciar: não vem do Warden.
    for line in dump.iter().filter_map(|line| line.strip_prefix("env=")) {
        let name = line.split('=').next().unwrap();
        let allowed = INHERITED_ENV
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(name))
            || name.starts_with("__LLVM_PROFILE");
        assert!(allowed, "variável inesperada no packwiz: {name}");
        assert_ne!(name, CURSEFORGE_KEY_ENV, "refresh não recebe a chave");
    }
    assert!(
        dump.contains(&"stdin=".to_owned()),
        "stdin fechado: {dump:?}"
    );
}

#[tokio::test]
async fn chave_vazia_nao_vai_para_o_ambiente() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    fake_pack(&pack, "mode=dump\n");
    let packwiz = packwiz(fake_binary(), temp.path());
    let key = SecretString::from("   ".to_owned());
    let cancel = CancellationToken::new();
    packwiz
        .run(
            &pack,
            &PackwizCommand::CurseForgeAddUrl {
                url: "https://www.curseforge.com/minecraft/mc-mods/jei/files/1".into(),
            },
            Some(&key),
            RunContext::new(&cancel),
        )
        .await
        .unwrap();
    let dump = dump(&pack);
    assert!(
        !dump
            .iter()
            .any(|line| line.starts_with(&format!("env={CURSEFORGE_KEY_ENV}=")))
    );
    assert!(dump.contains(&"arg=--".to_owned()));
}

#[tokio::test]
async fn ca_3_cancelar_mata_o_processo_e_os_filhos() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    fake_pack(&pack, "mode=sleep\n");
    let packwiz = packwiz(fake_binary(), temp.path());
    let cancel = CancellationToken::new();
    let cancelled_at = Mutex::new(None);
    let on_line = |line: &str| {
        if line == "filho pronto" {
            *cancelled_at.lock().unwrap() = Some(Instant::now());
            cancel.cancel();
        }
    };
    let error = packwiz
        .run(
            &pack,
            &PackwizCommand::Refresh { build: false },
            None,
            RunContext::new(&cancel).with_lines(&on_line),
        )
        .await
        .unwrap_err();
    let took = cancelled_at.lock().unwrap().unwrap().elapsed();
    assert!(matches!(error, Error::Cancelled { .. }), "{error}");
    assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Cancelled));
    assert!(took < Duration::from_secs(2), "levou {took:?}");
    // Evidência para o relatório (`--no-capture` mostra).
    #[allow(clippy::print_stderr)]
    {
        eprintln!("cancelamento até o processo morrer: {took:?}");
    }
    assert!(
        wait_child_dead(&pack, Duration::from_secs(2)),
        "o filho do packwiz sobreviveu ao cancelamento"
    );
}

#[tokio::test]
async fn tempo_limite_mata_o_processo_e_os_filhos() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    fake_pack(&pack, "mode=sleep\n");
    let packwiz =
        packwiz(fake_binary(), temp.path()).with_timeouts(Timeouts::all(Duration::from_secs(3)));
    let cancel = CancellationToken::new();
    let started = Instant::now();
    let error = packwiz
        .list(&pack, RunContext::new(&cancel))
        .await
        .unwrap_err();
    assert!(
        matches!(error, Error::Timeout { seconds: 3, .. }),
        "{error}"
    );
    assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Timeout));
    assert!(error.retryable());
    assert!(
        started.elapsed() < Duration::from_secs(6),
        "{:?}",
        started.elapsed()
    );
    assert!(wait_child_dead(&pack, Duration::from_secs(2)));
}

#[tokio::test]
async fn filho_orfao_nao_segura_a_execucao() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    // O processo principal sai na hora; o filho herda a saída e dormiria 120 s.
    fake_pack(&pack, "mode=orphan\n");
    let packwiz = packwiz(fake_binary(), temp.path());
    let cancel = CancellationToken::new();
    let started = Instant::now();
    let output = packwiz
        .run(&pack, &PackwizCommand::List, None, RunContext::new(&cancel))
        .await
        .unwrap();
    assert_eq!(output.exit_code, Some(0));
    assert!(
        output
            .lines
            .contains(&"saindo sem esperar o filho".to_owned())
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert!(wait_child_dead(&pack, Duration::from_secs(2)));
}

#[tokio::test]
async fn codigo_de_saida_e_linhas_dos_dois_canais() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    fake_pack(
        &pack,
        "print=\\e[31mLoading modpack...\\e[0m\nstderr=panic: algo quebrou\nexit=3\n",
    );
    let packwiz = packwiz(fake_binary(), temp.path());
    let cancel = CancellationToken::new();
    let error = packwiz
        .refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap_err();
    let Error::CommandFailed {
        exit_code, tail, ..
    } = &error
    else {
        panic!("{error}")
    };
    assert_eq!(*exit_code, 3);
    assert!(tail.contains(&"Loading modpack...".to_owned()), "{tail:?}");
    assert!(tail.contains(&"panic: algo quebrou".to_owned()), "{tail:?}");
    assert_eq!(
        error.code(),
        DomainCode::Domain(PackwizCliErrorCode::CommandFailed)
    );
    assert!(error.detail().unwrap().contains("panic: algo quebrou"));
}

#[tokio::test]
async fn codigo_zero_sem_resultado_reprova_na_pos_condicao() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    // "Deu certo", mas o pack não tem índice: o refresh não aconteceu.
    fake_pack(&pack, "print=Index refreshed!\n");
    let packwiz = packwiz(fake_binary(), temp.path());
    let cancel = CancellationToken::new();
    let error = packwiz
        .refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Postcondition { .. }), "{error}");
    assert_eq!(
        error.code(),
        DomainCode::Domain(PackwizCliErrorCode::PostconditionFailed)
    );

    let error = packwiz
        .curseforge_add(
            &pack,
            &CurseForgeTarget::Url("https://www.curseforge.com/minecraft/mc-mods/x/files/1".into()),
            None,
            RunContext::new(&cancel),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::Postcondition { reason, .. } if reason.contains("nenhum metafile")),
        "{error}"
    );

    fs::write(pack.join("saida.mrpack"), "arquivo velho").unwrap();
    let error = packwiz
        .modrinth_export(
            &pack,
            Path::new("saida.mrpack"),
            None,
            RunContext::new(&cancel),
        )
        .await
        .unwrap_err();
    // O arquivo velho foi apagado antes: não passa por resultado.
    assert!(
        matches!(&error, Error::Postcondition { reason, .. } if reason.contains("não foi gerado")),
        "{error}"
    );
    let error = packwiz
        .curseforge_export(
            &pack,
            ExportSide::Client,
            Path::new("saida.zip"),
            None,
            RunContext::new(&cancel),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Postcondition { .. }), "{error}");
}

#[tokio::test]
async fn chave_ausente_vira_erro_proprio() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    fake_pack(
        &pack,
        "print=Failed to search for project: failed to retrieve search results: WARDEN_CURSEFORGE_API_KEY ausente: a chave da CurseForge não foi informada.\nexit=1\n",
    );
    let packwiz = packwiz(fake_binary(), temp.path());
    let cancel = CancellationToken::new();
    let error = packwiz
        .curseforge_add(
            &pack,
            &CurseForgeTarget::Url("https://www.curseforge.com/minecraft/mc-mods/x/files/1".into()),
            None,
            RunContext::new(&cancel),
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(PackwizCliErrorCode::CurseForgeKeyMissing)
    );
}

struct Percents(Mutex<Vec<u64>>);

impl ProgressSink for Percents {
    fn stage(&self, _stage: &str, _label_key: &str) {}

    fn progress(&self, progress: Progress) {
        assert_eq!(progress.total, Some(100));
        self.0.lock().unwrap().push(progress.current);
    }
}

#[tokio::test]
async fn barra_de_progresso_vira_progresso() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    fake_pack(
        &pack,
        "print=Loading modpack...\nprint=Refreshing index... 40 % [==>---] 0s\nprint=\\e[1A\\e[JRefreshing index... 100 % [======] done\nprint=Index refreshed!\n",
    );
    let packwiz = packwiz(fake_binary(), temp.path());
    let cancel = CancellationToken::new();
    let percents = Percents(Mutex::new(Vec::new()));
    let output = packwiz
        .run(
            &pack,
            &PackwizCommand::Refresh { build: false },
            None,
            RunContext::new(&cancel).with_progress(&percents),
        )
        .await
        .unwrap();
    assert_eq!(*percents.0.lock().unwrap(), [40, 100]);
    assert_eq!(output.lines, ["Loading modpack...", "Index refreshed!"]);
}

#[tokio::test]
async fn erros_antes_de_criar_o_processo() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    let cancel = CancellationToken::new();
    let command = PackwizCommand::List;

    // Sem pack.toml.
    fs::create_dir_all(&pack).unwrap();
    let error = packwiz(fake_binary(), temp.path())
        .run(&pack, &command, None, RunContext::new(&cancel))
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(PackwizCliErrorCode::PackFileMissing)
    );

    // Sem executável.
    fake_pack(&pack, "mode=dump\n");
    let error = packwiz(temp.path().join("nao-existe.exe"), temp.path())
        .run(&pack, &command, None, RunContext::new(&cancel))
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(PackwizCliErrorCode::BinaryNotFound)
    );

    // Já cancelado: nem cria o processo.
    cancel.cancel();
    let error = packwiz(fake_binary(), temp.path())
        .run(&pack, &command, None, RunContext::new(&cancel))
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Cancelled { .. }));
    assert!(!pack.join("fake-dump.txt").exists());
    // A config vazia e o cache foram criados para isolar o packwiz.
    assert_eq!(
        fs::read(temp.path().join("dados/packwiz/packwiz.toml")).unwrap(),
        b""
    );
    assert!(temp.path().join("dados/cache/packwiz").is_dir());
}

#[test]
fn localizacao_do_executavel() {
    // Sem `WARDEN_PACKWIZ_BIN`, procura junto do executável atual (onde o teste não tem um).
    match Packwiz::locate_binary() {
        Ok(path) => assert!(std::env::var_os("WARDEN_PACKWIZ_BIN").is_some(), "{path:?}"),
        Err(Error::BinaryNotFound { path }) => {
            assert!(path.ends_with(BINARY_NAME), "{path:?}");
        }
        Err(error) => panic!("{error}"),
    }
}
