# Spike S-R5-4: laço de ferramentas do Gemini (código descartável)

Relatório: `docs/spikes/S-R5-4-laco-do-gemini.md`.

## Estrutura

| Arquivo | O que faz |
|---|---|
| `src/pack.rs` | Pack fictício "Vale Sombrio" (Forge 1.20.1, 21 mods, 3 sessões, crash do Steam 'n' Rails com o Create 6). |
| `src/tools.rs` | 17 ferramentas de leitura (as da ARCHITECTURE §9.5) e o registro do que foi enviado. |
| `src/laco.rs` | Laço `generateContent` sem estado, três modos de resposta final, novas tentativas, cotas, parada em sinal de cobrança, custo. |
| `src/avaliacao.rs` | Gabarito dos cenários: acerto da causa, seguimento, lentidão, injeção. |
| `src/evidence.rs` | Esquema da resposta final e conferência das evidências e dos mods. |
| `src/mock.rs` | Servidor simulado que exige as assinaturas e recusa histórico alterado, como a API real. |
| `src/env.rs` | Leitura do `.env` com aspas simples. |
| `src/testes_laco.rs` | Testes do laço contra o simulado, inclusive com as gravações reais. |
| `scripts/agregar.py` | Tabelas por modelo e cenário a partir de `execucoes/`. |
| `fixtures/` | Respostas reais gravadas (sem chave nem cabeçalhos) e corpos reais de erro. |

`execucoes/` (fora do git) guarda cada execução real completa, com o histórico enviado.

## Comandos

```powershell
cargo test                                               # 20 testes, sem rede externa
cargo run -- simulado                                    # cenários no simulado
cargo run -- contar gemini-3.8-flash                     # countTokens (sem custo)
cargo run -- real gemini-3.5-flash-lite travamento      # uma conversa real (consome cota)
cargo run -- lote gemini-3.5-flash-lite travamento,seguimento,lentidao,injecao,injecao-sem-defesa,malformada,malformada-validated
python scripts/agregar.py                                # tabelas a partir de execucoes/
cargo run -- reproduzir fixtures/travamento-gemini-3.8-flash-ferramenta.json
cargo run -- assinaturas gemini-3.6-flash                # reação da API a assinaturas adulteradas
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
| `seguimento-gemini-3.1-flash-lite-preview.json` | Conversa real em dois turnos (travamento e "e se eu voltar o Create?"). |
| `injecao-gemini-3.5-flash-lite.json` | Conversa real em que o modelo lê as linhas de chat com instruções falsas. |
| `assinaturas-gemini-*.json` | Resposta real com chamadas paralelas e a reação da API a 8 versões adulteradas do histórico (4 modelos). |
| `respostas-com-problema-reais.json` | `MALFORMED_FUNCTION_CALL` (com e sem chamadas), `MALFORMED_RESPONSE`, texto vazio e JSON final vazio. |

Opções de `real` e `lote`: a resposta final é pela ferramenta `responder` por padrão; `--junto` ou `--separado` usam `responseFormat`; `--fixture` grava a fixture; `--thinking <nível>`; `--sem-escalonar`.

Todas gravadas em 2026-10-04 com o pack fictício; não há dado pessoal.
