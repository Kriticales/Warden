# ADR-0023 — Registros (logging) e privacidade

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1), seguindo a decisão do dono sobre dados pessoais

## Contexto

Registros são essenciais para diagnosticar problemas do próprio Warden, mas não podem vazar segredos. O dono decidiu que logs só vão à IA com pergunta e sem dados pessoais.

## Decisão

- `tracing` com arquivo diário local (14 dias), spans com `operation_id` e `pack_id`; registros do frontend pelo `tauri-plugin-log` no mesmo arquivo.
- Proibido registrar segredos, cabeçalhos de autorização, ambiente de processos filhos e corpos de resposta da CurseForge.
- Caminhos locais podem aparecer nos registros (arquivo local); qualquer envio a terceiros passa pela redação (ARCHITECTURE §9.4) e por consentimento.
- Sem telemetria.

## Alternativas consideradas

- Registros em JSON: mais difíceis de ler para quem vai abrir o arquivo; o texto estruturado basta.
- Redigir também os registros locais: atrapalharia a depuração sem ganho real de privacidade.

## Consequências

- Teste de varredura procura padrões de segredo nos registros gerados pela suíte.
