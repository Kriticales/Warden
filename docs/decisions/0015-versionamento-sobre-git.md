# ADR-0015 — Versionamento simples sobre git embutido

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (versionamento simples, git escondido, GitHub) + decisão técnica (A1) (implementação)

## Contexto

Os apps anteriores tinham painel git completo (R4). O dono quer só "Salvar versão", changelog automático, histórico para voltar e enviar ao GitHub, com git escondido.

## Decisão

- Git embutido via **`git2`** (libgit2 compilada junto), sem exigir git instalado.
- Modelo: branch única `main` contendo só o ponto inicial e as versões salvas (commit + tag anotada `vX.Y.Z` com o changelog). "Alterações não salvas" = diferença para o último commit.
- **Pontos de segurança** como commits fora da branch (`refs/warden/safety/*`) antes de ações destrutivas.
- "Voltar para versão" restaura a árvore de trabalho de forma transacional, sem reescrever histórico.
- Changelog automático a partir dos metafiles (adicionados, removidos, atualizados) e dos configs alterados; sugestão SemVer por regras.
- **GitHub:** repositório privado por pack (D2), criado pela API REST; push com token guardado no cofre (D3), passado só em memória.

## Alternativas consideradas

- `git` do sistema + Git Credential Manager: dependência externa para um usuário leigo.
- `gix` (gitoxide): push ainda não maduro.
- Snapshots em zip: duplicariam o que o git já faz e complicariam o envio ao GitHub.

## Consequências

- Painel git completo e "trazer do GitHub" ficam fora da v1 (o segundo é P1 a confirmar).
- Testes de push usam servidor git local; teste com GitHub real só sob demanda.
