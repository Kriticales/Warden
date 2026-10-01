# ADR-0022 — Estratégia de testes e CI

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1)

## Contexto

Os dois apps anteriores passavam em centenas de testes enquanto fluxos básicos (remover mod, datapacks, restaurar snapshot) estavam quebrados contra o packwiz real (R4 §2.4). O dono exige funcionamento real.

## Decisão

- Testes de integração contra o **packwiz real** (compilado do commit fixado) em toda mudança; na CI, ausência do binário é falha.
- **Conformidade** contra o packwiz-installer real (materialização e exportação).
- **Jogo real** na matriz de versões em Linux com Xvfb e Mesa (semanal/manual e por marco); Windows por roteiro manual por marco.
- Testes de rede contra APIs reais em workflow noturno/manual com a chave nos *secrets*.
- Dourados (`insta`) para arquivos packwiz, linhas de comando e changelogs; corpus reais para configs, jars e logs.
- E2E com o app real (WebdriverIO + tauri-driver), APIs simuladas por servidor local de fixtures.
- CI: Linux em toda mudança; Windows na `main`, em PRs marcados e à noite (decisão pendente D9).
- Cobertura mínima por crate (QUALITY §4.2).

## Alternativas consideradas

- Só testes unitários com mocks: já se mostrou insuficiente.
- Runner Windows com GPU para smoke do jogo: custo e manutenção; fica para depois se os roteiros manuais não bastarem.

## Consequências

- CI mais longa e com dependências (Go, Java); mitigada com cache e divisão em workflows.
- Packs quebrados de propósito e logs reais viram parte do repositório de testes.
