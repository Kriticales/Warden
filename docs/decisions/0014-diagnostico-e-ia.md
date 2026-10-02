# ADR-0014 — Diagnóstico determinístico em duas camadas e IA com consentimento

- **Status:** aceita; **substituída em parte pela [ADR-0030](0030-ia-com-ferramentas-e-conversas.md)** (consentimento por conversa e IA com ferramentas) · **Data:** 2026-10-01 · **Origem:** decisão do dono

## Contexto

O dono quer entender por que o pack não abre. Grande parte dos problemas é detectável por regras sobre metadados e logs (R1 §5, R2 §6.3, R3 §6.4). IA é útil quando as regras não explicam, mas logs têm dados pessoais.

## Decisão

- **Camada determinística** (P0): regras sobre metadados (pack, APIs, jars), versões, Java, duplicatas e conflitos conhecidos, executadas ao clicar em "Testar" (passagem rápida antes de baixar e completa antes de abrir o jogo) e análise de logs após crash. Todo achado tem **evidência**.
- **IA Gemini** (P1) com a chave do próprio usuário, só quando ele pede. **Antes de cada envio** o app mostra o texto exato, já sem dados pessoais, e pergunta. Sem opção "não perguntar de novo".
- Dados curados (conflitos, categorias exclusivas, obsoletos) e padrões de log em arquivos de dados versionados; porte do codex-minecraft (MIT) com atribuição.

## Alternativas consideradas

- Só IA: caro, lento, sujeito a erro e envia dados sem necessidade.
- Enviar logs a serviços como mclo.gs: terceiros a mais sem necessidade.

## Consequências

- Corpus de logs reais e packs quebrados de teste viram parte da suíte.
- A redação de dados pessoais é testada com casos positivos e negativos (ARCHITECTURE §9.4).
