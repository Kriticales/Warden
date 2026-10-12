//! Envio direto do relato pela API do GitHub (UI-03; ADR-0059).
//!
//! Ordem: labels que faltam (`relato-do-dono` e a da categoria), a branch dos prints (criada
//! de um commit órfão se faltar) e cada print, e por fim a issue, com o corpo da
//! pré-visualização e o endereço de cada print no lugar do aviso. Um 401 no meio renova o
//! token uma vez e repete tudo: as labels e a branch já existem, e os prints, com o nome tirado
//! do conteúdo, caem no mesmo arquivo. Nada é repetido depois de a issue ser criada.

use serde::Serialize;
use warden_core::{Progress, ProgressSink};
use warden_github::{CreatedIssue, GithubApi, Label, SecretString};

use super::attachments::StoredImage;
use super::body::{ReportParts, fit_direct};
use crate::commands::github_login::GithubSession;
use crate::error::AppError;

/// A issue criada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProblemReportSent {
    /// Número da issue.
    pub(crate) number: u32,
    /// Página da issue no GitHub.
    pub(crate) url: String,
}

/// O relato pronto para enviar: o texto já redigido, as labels e os prints.
#[derive(Debug, Clone)]
pub(crate) struct Prepared {
    /// Título, corpo e prints (sem endereço ainda).
    pub(crate) parts: ReportParts,
    /// Labels da issue, criadas se faltarem.
    pub(crate) labels: Vec<Label>,
    /// Os prints, na ordem de `parts.prints`.
    pub(crate) images: Vec<StoredImage>,
}

/// Envia o relato. Erros chegam com o código do GitHub (`github.*`) ou de rede (`core.*`).
pub(crate) async fn send(
    session: &GithubSession,
    prepared: &Prepared,
    progress: &dyn ProgressSink,
) -> Result<ProblemReportSent, AppError> {
    let token = session.token().await?;
    let issue = match attempt(session, &token, prepared, progress).await {
        Err(warden_github::Error::SessionExpired { .. }) => {
            tracing::info!("GitHub recusou o token no envio: renovando e repetindo uma vez");
            let token = session.refresh_now().await?;
            attempt(session, &token, prepared, progress).await
        }
        other => other,
    }
    .map_err(|error| AppError::from_domain(&error))?;
    Ok(ProblemReportSent {
        number: u32::try_from(issue.number).unwrap_or(u32::MAX),
        url: issue.url.to_string(),
    })
}

async fn attempt(
    session: &GithubSession,
    token: &SecretString,
    prepared: &Prepared,
    progress: &dyn ProgressSink,
) -> Result<CreatedIssue, warden_github::Error> {
    let api = GithubApi::new(session.http().clone(), session.endpoints().clone(), token);
    progress.stage("labels", "relato:etapas.labels");
    for label in &prepared.labels {
        api.ensure_label(label).await?;
    }
    let mut parts = prepared.parts.clone();
    if !prepared.images.is_empty() {
        progress.stage("prints", "relato:etapas.prints");
        api.ensure_attachments_branch().await?;
        let total = prepared.images.len() as u64;
        for (index, (image, print)) in prepared.images.iter().zip(&mut parts.prints).enumerate() {
            progress.progress(Progress::items(index as u64, Some(total)));
            let url = api.upload_attachment(&image.file_name(), &image.bytes).await?;
            print.url = Some(url.to_string());
        }
        progress.progress(Progress::items(total, Some(total)));
    }
    progress.stage("issue", "relato:etapas.issue");
    let fitted = fit_direct(&parts);
    let labels: Vec<String> = prepared.labels.iter().map(|label| label.name.clone()).collect();
    api.create_issue(&fitted.title, &fitted.body, &labels).await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::{Value, json};
    use warden_core::Progress;
    use warden_secrets::SecretKind;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    use super::*;
    use crate::commands::github_login::tests::{
        CLIENT_ID, access, mount_refresh, refresh, session, signed_in, value,
    };
    use crate::commands::report::attachments::{ImageKind, StoredImage};
    use crate::commands::report::body::PrintRef;

    const REPO: &str = "/api/repos/Kriticales/Warden";

    #[derive(Default)]
    struct Steps(Mutex<Vec<String>>);

    impl ProgressSink for Steps {
        fn stage(&self, stage: &str, _label_key: &str) {
            self.0.lock().unwrap().push(stage.to_owned());
        }
        fn progress(&self, progress: Progress) {
            self.0
                .lock()
                .unwrap()
                .push(format!("{}/{}", progress.current, progress.total.unwrap_or(0)));
        }
    }

    fn image(id: &str, seed: u8) -> StoredImage {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.push(seed);
        StoredImage {
            id: id.into(),
            name: format!("tela {seed}.png"),
            kind: ImageKind::Png,
            bytes,
        }
    }

    fn prepared(images: Vec<StoredImage>) -> Prepared {
        Prepared {
            parts: ReportParts {
                title: "O Testar travou".into(),
                category: "Erro".into(),
                labels: vec!["relato-do-dono".into(), "categoria: erro".into()],
                description: "Nada abriu".into(),
                prints: images
                    .iter()
                    .map(|image| PrintRef {
                        name: image.name.clone(),
                        url: None,
                    })
                    .collect(),
                environment: vec![("Warden".into(), "0.1.0".into())],
                log: None,
            },
            labels: vec![
                Label {
                    name: "relato-do-dono".into(),
                    color: "1d76db".into(),
                    description: "d".into(),
                },
                Label {
                    name: "categoria: erro".into(),
                    color: "d73a4a".into(),
                    description: "d".into(),
                },
            ],
            images,
        }
    }

    /// O GitHub simulado: labels, branch, conteúdo e issue, aceitando só o token `token`.
    async fn github(server: &MockServer, token: &str) {
        let auth = format!("Bearer {token}");
        Mock::given(method("GET"))
            .and(path(format!("{REPO}/labels/relato-do-dono")))
            .and(header("authorization", auth.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "name": "relato-do-dono" })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{REPO}/labels/categoria:%20erro")))
            .and(header("authorization", auth.as_str()))
            .respond_with(ResponseTemplate::new(404))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("{REPO}/labels")))
            .and(header("authorization", auth.as_str()))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{REPO}/git/ref/heads/relatos-anexos")))
            .and(header("authorization", auth.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "ref": "refs/heads/relatos-anexos",
                "object": { "sha": "c0ffee" }
            })))
            .mount(server)
            .await;
        Mock::given(method("PUT"))
            .and(header("authorization", auth.as_str()))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("{REPO}/issues")))
            .and(header("authorization", auth.as_str()))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "number": 7 })))
            .mount(server)
            .await;
        // Qualquer outro token: 401.
        Mock::given(wiremock::matchers::path_regex("^/api/"))
            .respond_with(ResponseTemplate::new(401))
            .with_priority(10)
            .mount(server)
            .await;
    }

    fn issue_body(requests: &[Request]) -> Value {
        let issue = requests
            .iter()
            .rfind(|request| request.url.path() == format!("{REPO}/issues"))
            .unwrap();
        serde_json::from_slice(&issue.body).unwrap()
    }

    /// CA-UI03-05 e CA-UI03-09: labels que faltam, prints na branch e issue com os endereços;
    /// nenhum token no corpo enviado.
    #[tokio::test]
    async fn ca_ui03_05_envia_labels_prints_e_issue() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        github(&server, &access(1)).await;
        let (session, secrets) = session(dir.path(), &server, CLIENT_ID);
        signed_in(dir.path(), &secrets, 3_600);
        let steps = Steps::default();
        let sent = send(
            &session,
            &prepared(vec![image("aaaa", 1), image("bbbb", 2)]),
            &steps,
        )
        .await
        .unwrap();
        assert_eq!(sent.number, 7);
        assert_eq!(
            sent.url,
            format!("{}/site/Kriticales/Warden/issues/7", server.uri())
        );
        assert_eq!(
            *steps.0.lock().unwrap(),
            ["labels", "prints", "0/2", "1/2", "2/2", "issue"]
        );
        let requests = server.received_requests().await.unwrap();
        let uploads: Vec<_> = requests
            .iter()
            .filter(|request| request.method.as_str() == "PUT")
            .map(|request| request.url.path().to_owned())
            .collect();
        assert_eq!(
            uploads,
            [
                format!("{REPO}/contents/anexos/aaaa.png"),
                format!("{REPO}/contents/anexos/bbbb.png")
            ]
        );
        let created: Vec<_> = requests
            .iter()
            .filter(|request| request.url.path() == format!("{REPO}/labels"))
            .map(|request| serde_json::from_slice::<Value>(&request.body).unwrap()["name"].clone())
            .collect();
        assert_eq!(created, [json!("categoria: erro")]);
        let issue = issue_body(&requests);
        assert_eq!(issue["title"], "O Testar travou");
        assert_eq!(issue["labels"], json!(["relato-do-dono", "categoria: erro"]));
        let body = issue["body"].as_str().unwrap();
        assert!(body.starts_with("**Categoria:** Erro\n\n### Descrição\n\nNada abriu"), "{body}");
        assert!(body.contains(&format!(
            "![Print 1: tela 1.png]({}/site/Kriticales/Warden/blob/relatos-anexos/anexos/aaaa.png?raw=true)",
            server.uri()
        )), "{body}");
        assert!(body.contains("![Print 2: tela 2.png]("), "{body}");
        assert!(!body.contains(super::super::body::PRINT_PENDING));
        for request in &requests {
            let text = String::from_utf8_lossy(&request.body);
            assert!(!text.contains(&access(1)) && !text.contains(&refresh(1)));
        }
    }

    /// CA-UI03-04: um 401 no meio renova uma vez e repete; os tokens novos ficam no cofre.
    #[tokio::test]
    async fn ca_ui03_04_401_renova_e_repete() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        github(&server, &access(2)).await;
        mount_refresh(&server, 1, 2).await;
        let (session, secrets) = session(dir.path(), &server, CLIENT_ID);
        signed_in(dir.path(), &secrets, 3_600);
        let sent = send(&session, &prepared(Vec::new()), &Steps::default())
            .await
            .unwrap();
        assert_eq!(sent.number, 7);
        assert_eq!(value(&secrets, SecretKind::Github), Some(access(2)));
    }

    /// CA-UI03-08: renovação recusada depois de um 401 = login vencido, e o login é desfeito.
    #[tokio::test]
    async fn ca_ui03_08_login_vencido() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        github(&server, &access(9)).await;
        Mock::given(method("POST"))
            .and(path("/site/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "error": "bad_refresh_token" })))
            .mount(&server)
            .await;
        let (session, secrets) = session(dir.path(), &server, CLIENT_ID);
        signed_in(dir.path(), &secrets, 3_600);
        let error = send(&session, &prepared(Vec::new()), &Steps::default())
            .await
            .unwrap_err();
        assert_eq!(error.code.to_string(), "github.SESSION_EXPIRED");
        assert_eq!(value(&secrets, SecretKind::Github), None);
    }

    /// CA-UI03-08: sem login, nada sai; repositório não encontrado e sem permissão têm o
    /// próprio código.
    #[tokio::test]
    async fn ca_ui03_08_sem_login_e_erros_do_repositorio() {
        let dir = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        let (session, secrets) = session(dir.path(), &server, CLIENT_ID);
        let error = send(&session, &prepared(Vec::new()), &Steps::default())
            .await
            .unwrap_err();
        assert_eq!(error.code.to_string(), "github.NOT_SIGNED_IN");
        assert!(server.received_requests().await.unwrap().is_empty());

        signed_in(dir.path(), &secrets, 3_600);
        for (status, expected) in [
            (404, "github.REPOSITORY_NOT_FOUND"),
            (403, "github.PERMISSION_DENIED"),
        ] {
            server.reset().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;
            let error = send(&session, &prepared(Vec::new()), &Steps::default())
                .await
                .unwrap_err();
            assert_eq!(error.code.to_string(), expected);
        }
    }
}
