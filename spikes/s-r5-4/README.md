# Spike S-R5-4: laço de ferramentas do Gemini (código descartável)

Relatório: `docs/spikes/S-R5-4-laco-do-gemini.md`.

## Estrutura

| Arquivo | O que faz |
|---|---|
| `src/pack.rs` | Pack fictício "Vale Sombrio" (Forge 1.20.1, 21 mods, 3 sessões, crash do Steam 'n' Rails com o Create 6). |
| `src/tools.rs` | 17 ferramentas de leitura (as da ARCHITECTURE §9.5) e o registro do que foi enviado. |
| `src/laco.rs` | Laço `generateContent` sem estado, três modos de resposta final, novas tentativas, cotas, custo. |
| `src/evidence.rs` | Esquema da resposta final e conferência das evidências e dos mods. |
| `src/mock.rs` | Servidor simulado que exige as assinaturas e recusa histórico alterado, como a API real. |
| `src/env.rs` | Leitura do `.env` com aspas simples. |
| `src/testes_laco.rs` | Testes do laço contra o simulado, inclusive com as gravações reais. |
| `fixtures/` | Respostas reais gravadas (sem chave nem cabeçalhos) e corpos reais de erro. |

`execucoes/` (fora do git) guarda cada execução real completa, com o histórico enviado.

## Comandos

```powershell
cargo test                                               # 17 testes, sem rede externa
cargo run -- simulado                                    # cenários no simulado
cargo run -- contar gemini-3.8-flash                     # countTokens (sem custo)
cargo run -- real gemini-3.8-flash travamento --ferramenta   # conversa real (consome cota)
cargo run -- reproduzir fixtures/travamento-gemini-3.8-flash-ferramenta.json
cargo run -- assinaturas gemini-3.8-flash                # NÃO executado no spike (cota)
```

A chave vem de `GEMINI_API_KEY` no `.env` do repositório principal (ou do arquivo em `WARDEN_ENV`),
vai só no cabeçalho `x-goog-api-key` e nunca é gravada: antes de escrever qualquer arquivo, o
programa confere que o valor não aparece nele.

## Fixtures

| Arquivo | Origem |
|---|---|
| `travamento-gemini-3.8-flash.json` | Conversa real, `responseFormat` junto com as ferramentas; o modelo respondeu em Markdown e o laço pediu o JSON com `mode: NONE`. |
| `travamento-gemini-3.8-flash-ferramenta.json` | Conversa real, resposta final pela ferramenta `responder`. |
| `interrompida-por-cota-gemini-3.8-flash.json` | Conversa real interrompida pela cota diária do plano gratuito. |
| `erros-http-reais.json` | Corpos reais do 503 "high demand" e dos 429 de cota por minuto e por dia. |

Todas gravadas em 2026-10-04 com o pack fictício; não há dado pessoal.
