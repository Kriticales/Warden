# ADR-0013 — Editor de configs com preservação de formato no backend

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (editor preservando comentários) + decisão técnica (A1) (onde e como)

## Contexto

Configs de mods vêm em muitos formatos (TOML, JSON, JSON5, `.properties`, `.cfg` antigo, `options.txt`). Reserializar perde comentários e ordem (R4 §1.4). O jogo também reescreve alguns arquivos (R3 §5.2).

## Decisão

- Toda edição estruturada acontece no Rust (`warden-configs`), com bibliotecas/parsers que preservam o arquivo (`toml_edit`, `jsonc-parser` cst, parsers próprios por linha); a interface envia só "chave → novo valor".
- O editor de texto (CodeMirror 6) cobre todos os arquivos de texto; o formulário estruturado é P1.
- Concorrência otimista por hash; gravação atômica; sem `.bak` no pack (o histórico guarda versões).
- YAML só como texto na v1.

## Alternativas consideradas

- Editar no frontend com bibliotecas JS: duplicaria lógica e testes nos dois lados.
- Reserializar o arquivo inteiro: destrói comentários e formatação.

## Consequências

- Corpus de configs reais e testes de ida e volta obrigatórios por formato.
- Avisos na interface sobre arquivos que o próprio jogo reescreve.
