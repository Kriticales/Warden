# ADR-0055 — IA: modelo padrão Flash-Lite e resposta final pela ferramenta `responder`

- **Status:** aceita · **Data:** 2026-10-04 · **Origem:** decisão técnica (spike S-R5-4; registrada na D7) · **Complementa:** [ADR-0030](0030-ia-com-ferramentas-e-conversas.md)

## Contexto

A [ADR-0030](0030-ia-com-ferramentas-e-conversas.md) previa a resposta final em JSON por `responseFormat`, e a ARCHITECTURE §9.5 escolhia como padrão o Flash estável mais novo. O spike S-R5-4 (`docs/spikes/S-R5-4-laco-do-gemini.md`, §8, §9, §13 e §16) rodou o laço com a chave real em 5 modelos e mostrou:

- com ferramentas ligadas, o `responseFormat` foi ignorado em 7 de 7 tentativas; a resposta final só sai confiável como chamada de uma ferramenta própria;
- o `gemini-3.5-flash-lite` acertou 8 de 8 travamentos, tem cota gratuita folgada, ~1,7 s por requisição e custa US$ 0,005 a 0,04 por conversa; os Flash ficaram sem cota (20 requisições por dia no plano gratuito) e com muitos 503;
- o `retryDelay` do limite diário é enganoso (prometeu liberação em 2 h e o limite voltou em 24 h);
- a conferência de evidência pega ids e valores inventados, mas uma linha de chat do log (escrita por um jogador) existe no log sem provar nada.

## Decisão

- **Modelo padrão:** o **Flash-Lite estável mais novo** resolvido pelo `models.list` (em 04/10/2026, `gemini-3.5-flash-lite`). Opção **"Análise mais cuidadosa"**: o Flash estável mais novo (`gemini-3.8-flash`), com o aviso de que no plano gratuito ele permite poucas conversas por dia. Nenhum nome de modelo fixo no código.
- **Resposta final pela ferramenta `responder`**, com o esquema da resposta em `parametersJsonSchema` e sempre presente em `allowedFunctionNames`. Reserva: quando o modelo responde em texto ou ao estourar 8 rodadas, uma requisição com `mode: NONE` e `responseFormat` (`APPLICATION_JSON` com o esquema).
- **Novas tentativas:** 503, 500 e tempo-limite de rede → até 4, com espera exponencial; `MALFORMED_FUNCTION_CALL`, `UNEXPECTED_TOOL_CALL`, `MALFORMED_RESPONSE`, resposta sem `content`, texto vazio e resposta final vazia → até 2, sem mexer no histórico. 429 por minuto → `AI_RATE_LIMITED` (espera o tempo pedido, até 60 s, com aviso na tela); 429 diário → `AI_QUOTA_EXCEEDED` na hora, **sem prometer horário de liberação**, sugerindo trocar de modelo.
- **Evidência:** uma linha de chat do log (`<Jogador> …`) **não prova sozinha** e ganha selo próprio ("baseado em mensagem de jogador"). O selo de evidência diz **de onde veio o trecho**, nunca que a conclusão está certa; a interface deixa isso claro, e o medidor de confiança não repete a confiança que o modelo declara.

## Alternativas consideradas

- **Manter o Flash como padrão:** mais caro, sem cota no plano gratuito e com 503 frequentes; a qualidade a mais não foi demonstrada (§15 do spike).
- **`gemini-3.1-flash-lite`:** números parecidos, mas tem desligamento anunciado (05/2027) e aceitou assinatura adulterada em silêncio.
- **Só `responseFormat`:** ignorado com ferramentas.

## Consequências

- O plano gratuito vira o caso comum: a interface trata os limites como situação normal (contador de requisições da conversa, espera visível).
- A D-04 implementa os 12 itens da §16 do spike (ROADMAP D-04; ARCHITECTURE §9.5).
