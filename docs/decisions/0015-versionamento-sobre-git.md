# ADR-0015 — Versionamento simples sobre git embutido

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (versionamento simples, git escondido, GitHub) + decisão técnica (A1) (implementação)

## Contexto

Os apps anteriores tinham painel git completo (R4). O dono quer só "Salvar versão", changelog automático, histórico para voltar e enviar ao GitHub, com git escondido.

## Decisão

- Git embutido via **`git2`** (libgit2 compilada junto), sem exigir git instalado.
- Modelo para packs criados pelo Warden: branch única `main` contendo só o ponto inicial e as versões salvas (commit + tag anotada `vX.Y.Z` com o changelog). "Alterações não salvas" = diferença para o último commit. Packs importados que já têm histórico git mantêm a branch e o histórico; a última versão salva é a tag `v<SemVer>` mais recente (ARCHITECTURE §11).
- **Pontos de segurança** como commits fora da branch (`refs/warden/safety/*`) antes de ações destrutivas.
- "Voltar para versão" restaura a árvore de trabalho de forma transacional, sem reescrever histórico.
- Changelog automático a partir dos metafiles (adicionados, removidos, atualizados) e dos configs alterados; sugestão SemVer por regras.
- **GitHub:** repositório privado, criado pela API REST; push com token guardado no cofre, passado só em memória. Um repositório por pack e token colado são as recomendações das decisões pendentes D2 e D3 da SPEC; se o dono escolher diferente, este ADR é substituído.

## Alternativas consideradas

- `git` do sistema + Git Credential Manager: dependência externa para um usuário leigo.
- `gix` (gitoxide): push ainda não maduro.
- Snapshots em zip: duplicariam o que o git já faz e complicariam o envio ao GitHub.

## Consequências

- Painel git completo fica fora da v1; "trazer mudanças do GitHub" é P2.
- Testes de push usam servidor git local; teste com GitHub real só sob demanda.
