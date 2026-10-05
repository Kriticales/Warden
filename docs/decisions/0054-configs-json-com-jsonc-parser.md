# ADR-0054 — Configs JSON, JSONC e JSON5 com `jsonc-parser` 0.34

- **Status:** aceita · **Data:** 2026-10-04 · **Origem:** decisão técnica (tarefa C-01; registrada na D7) · **Complementa:** [ADR-0013](0013-editor-de-configs.md)

## Contexto

A ARCHITECTURE §10 deixava o JSON5 para "`json-five` (avaliar na C-01)", com recuo por faixa de bytes. A C-01 construiu a `warden-configs` com um corpus de 158 configs reais e precisava de leitura sem perda e edição mínima para JSON, JSONC e JSON5.

## Decisão

- JSON, JSONC e JSON5 usam o mesmo crate, `jsonc-parser` 0.34 (árvore concreta, que preserva comentários, vírgulas e formatação); o `json-five` não entra.
- A edição estruturada **só altera chaves que já existem** no arquivo; acrescentar ou remover chaves fica para o editor de texto.
- Arquivos acima de **2 MB** ou que não são UTF-8 válido não passam pelo parser: abrem só no editor de texto, com o motivo (SPEC T12).

## Alternativas consideradas

- **`json-five`:** um parser a mais para manter, sem ganho no corpus.
- **Edição por faixa de bytes para JSON5:** duplicaria a lógica que o `jsonc-parser` já oferece.

## Consequências

- Uma dependência a menos na lista da ADR-0024.
- Formatos JSON5 que o `jsonc-parser` não aceitar caem no "só texto" pela leitura de teste (ARCHITECTURE §10.1), sem perda.
