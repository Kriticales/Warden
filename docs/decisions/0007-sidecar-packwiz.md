# ADR-0007 — packwiz como sidecar compilado de commit fixado, com chave da CurseForge em tempo de execução

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (sidecar de commit fixado) + decisão técnica (A1) (patch da chave)

## Contexto

O packwiz não publica releases (artefatos da CI expiram em 90 dias) e embute a chave da CurseForge do próprio autor, que pede a derivados que usem a deles (R3 §1.12). No código (`curseforge/request.go`, verificado em 2026-10-01), a variável `cfApiKey` só pode ser trocada em tempo de compilação (`-ldflags -X`) e, vazia, cai na chave embutida. No Warden, a chave é digitada pelo usuário e fica no cofre do sistema (ADR-0017), então não pode ser fixada no build.

## Decisão

- Compilar o packwiz (MIT) a partir do commit registrado em `third_party/packwiz/COMMIT`, para Linux e Windows, com `cargo xtask build-packwiz`, e embutir como sidecar do Tauri (`bundle.externalBin`).
- Aplicar patches mínimos versionados em `third_party/packwiz/patches/`. O primeiro faz a chave vir da variável de ambiente `WARDEN_CURSEFORGE_API_KEY` e **remove o recuo para a chave embutida**.
- O Rust executa o binário diretamente (`tokio::process`), sem expor o plugin `shell` à interface.

## Alternativas consideradas

- Embutir a chave do dono via `-ldflags`: coloca a chave dentro do executável e contraria "chaves digitadas pelo usuário".
- Usar o binário do PATH do sistema: versão imprevisível.
- Baixar artefatos da CI do packwiz: expiram e não são reprodutíveis.
- Fork no GitHub: possível no futuro; por ora os patches no repositório bastam e são auditáveis.

## Consequências

- Go ≥ 1.24 é pré-requisito de desenvolvimento e da CI.
- Atualizar o commit do packwiz é uma tarefa própria, com toda a suíte de integração.
- Sem assinatura digital, antivírus podem estranhar o `.exe` (issue #374 do packwiz); aceitável para uso pessoal (decisão D4).
