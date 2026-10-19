# Fixtures da `warden-ai`

Respostas **reais** da API do Gemini gravadas pelo spike S-R5-4 em 2026-10-04
(`docs/spikes/S-R5-4-laco-do-gemini.md`), copiadas sem alteração de
`spikes/s-r5-4/fixtures/` da branch `Kriticales/spike-s-r5-4-laco-gemini` (commit `7985f07`),
como pede a §16, item 9, do spike. Só os corpos das respostas: sem cabeçalhos e sem a chave.
O pack das conversas é o fictício "Vale Sombrio" (Forge 1.20.1), reproduzido para os testes em
`tests/support/vale_sombrio.rs`.

| Arquivo | O que tem | Usado em |
|---|---|---|
| `travamento-gemini-3.8-flash.json` | Conversa de travamento, resposta final em texto (forma "Junto" do spike) | `tests/laco.rs` (reprodução: a reserva `NONE` + `responseFormat`) |
| `travamento-gemini-3.8-flash-ferramenta.json` | Conversa de travamento com a resposta pela ferramenta `responder` | `tests/laco.rs` (reprodução) |
| `seguimento-gemini-3.1-flash-lite-preview.json` | Seguimento em dois turnos (duas perguntas na mesma conversa) | `tests/laco.rs` (reprodução, segundo turno com o histórico intacto) |
| `injecao-gemini-3.5-flash-lite.json` | Conversa que leu as linhas de chat com instruções falsas | `tests/laco.rs` (reprodução: as linhas de chat ficam "baseado em mensagem de jogador") |
| `assinaturas-gemini-*.json` | Reação da API a históricos com assinaturas alteradas (4 modelos) | `tests/laco.rs` (o servidor simulado recusa o que a API real recusa) |
| `erros-http-reais.json` | Corpos reais do 503 "high demand" e dos 429 por minuto e diário | `src/gemini.rs` e `tests/laco.rs` |
| `respostas-com-problema-reais.json` | `MALFORMED_FUNCTION_CALL`, `MALFORMED_RESPONSE`, texto vazio e JSON final vazio | `tests/laco.rs` |
| `corpos-gemini-d04.json` | Gravado na D-15, antes de mexer no laço: o SHA-256 de cada corpo que o código da D-04 (commit `04f2d21`) mandou ao simulado nas quatro gravações acima e na continuação da conversa abaixo, nos dois jeitos de o `serde_json` ordenar as chaves | `tests/corpo_identico.rs` (o Gemini migrado manda os mesmos bytes, CA-T14-22) |
| `conversa-v1-seguimento.json` | Uma conversa no formato 1 (D-04, com o `contents` da API), gravada pelo código da D-04 ao reproduzir `seguimento-gemini-3.1-flash-lite-preview.json` no pack fictício | `tests/corpo_identico.rs` (abre igual e continua com o Gemini) |

A conversa "interrompida por cota" do spike não foi copiada: os corpos do 429 estão em
`erros-http-reais.json`.
