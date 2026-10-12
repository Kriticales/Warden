# ADR-0058 — Patch 0002 do packwiz: endereço da API da CurseForge trocável, só para testes

- **Status:** aceita · **Data:** 2026-10-12 · **Origem:** decisão do orquestrador (revisão da P1-10) · **Exceção a:** [ADR-0049](0049-copia-fixa-do-packwiz-e-patches.md)

## Contexto

A ADR-0049 diz que um patch novo no packwiz só entra pela tarefa dona da chamada ao packwiz (P1-02) ou por uma tarefa própria do orquestrador. A P1-10 precisava provar com o packwiz real o link da CurseForge (`packwiz curseforge add`, CA-T08-03), mas os termos da CurseForge proíbem guardar respostas dela no repositório e a CI não tem chave: o packwiz original só fala com `https://api.curseforge.com/v1`. Para isso, a P1-10 criou o patch `third_party/packwiz/patches/0002-endereco-da-api-da-curseforge-para-testes.patch`.

## Decisão

- O patch 0002 fica, como exceção registrada à ADR-0049. Ele faz o packwiz ler `WARDEN_API_BASE_CURSEFORGE` (o mesmo nome e formato que o Warden já usa em build de debug, ARCHITECTURE §17.1) e, vazia ou ausente, usar o endereço oficial, como antes.
- O Warden só repassa a variável ao packwiz por `Packwiz::with_curseforge_api`: o processo do packwiz roda com o ambiente limpo, e o app só chama esse método em build de debug (`#[cfg(debug_assertions)]` em `commands/add_curseforge.rs`). O instalador nunca troca o endereço, de modo que a chave da CurseForge nunca vai para outro servidor.
- O `cargo xtask build-packwiz` continua aplicando os patches em ordem com `git apply --whitespace=error-all` e conferindo que a chave embutida do packwiz não está no executável.

## Consequências

- Os testes `link::*` de `crates/warden-project/tests/add_curseforge.rs` e o E2E `add-curseforge.e2e.ts` exercitam o packwiz real contra uma CurseForge simulada. O teste `endereco_da_curseforge_so_com_with_curseforge_api` (`crates/warden-packwiz-cli/tests/processo.rs`) prova que a variável não chega ao packwiz sem a chamada explícita.
- O registro do patch está no `third_party/packwiz/README.md`, como pede a ADR-0049. Quando o commit do packwiz for atualizado (ADR-0007), o patch é reaplicado ou refeito junto com o 0001.
