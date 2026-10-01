# ADR-0011 — Materialização do pack na instância em Rust, com conformidade ao packwiz-installer

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1)

## Contexto

O pack precisa ser montado na instância de teste antes de cada teste. O packwiz-installer (Java, MIT) faz isso, mas está praticamente parado desde 2024, dá pouco controle de progresso e de erros, e a captura das mudanças do teste precisa do mesmo manifesto de estado (R2 §7.2).

## Decisão

Implementar em `warden-instance` a **mesma semântica** do packwiz-installer (lado, opcionais, `preserve`, remoção do que saiu, verificação de hash), com downloads pelo `warden-http` e cache por hash compartilhado. Um **teste de conformidade** instala os mesmos packs com o packwiz-installer real e exige árvores idênticas.

## Alternativas consideradas

- Usar o packwiz-installer: dependência Java extra no fluxo, sem progresso fino, sem manifesto para a captura.
- Cópia simples sem manifesto: não remove o que saiu do pack nem respeita `preserve`.

## Consequências

- O que o Warden testa é o que o jogador receberia pelo packwiz-installer (provado pelo teste).
- O manifesto de estado (`state/manifest.json`) serve à captura (SPEC T15).
