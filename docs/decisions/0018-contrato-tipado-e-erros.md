# ADR-0018 — Contrato tipado entre interface e backend e modelo de erros

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1)

## Contexto

No manager anterior, funcionalidades ficaram órfãs entre backend e frontend, e ~90 erros viravam `INTERNAL_ERROR` com mensagem crua em inglês (R4 §2.4). O modelo `Result<T>` com código estável funcionou bem (R4 §2.5).

## Decisão

- Comandos Tauri e tipos gerados para TypeScript com **`specta` + `tauri-specta`** (`bindings.ts` versionado, CI confere que está atualizado). Nada de tipos escritos à mão dos dois lados.
- Todo comando retorna `Result<T, AppError>`; `AppError` tem `code` (`{ domain, code }` estável por crate), `params`, `detail` (evidência técnica), `retryable` e `operationId`.
- Cada crate é dona do seu enum de códigos; o frontend tem um `Record<Código, frase>` por domínio, que não compila se faltar tradução.
- Guarda de contrato: todo comando registrado precisa ter uso em `src/features/`.

## Alternativas consideradas

- `ts-rs` só para tipos + wrappers escritos à mão: mais código manual. É o **plano B** se o `tauri-specta` (ainda em versão RC em 2026-10-01) der problema; a troca fica restrita a `warden-app` e `lib/ipc`.
- Erros como strings: impossível traduzir e testar.

## Consequências

- Versão do `tauri-specta` fixada exatamente; atualização é tarefa revisada.
- Conflitos em `bindings.ts` na integração são resolvidos regenerando o arquivo.
