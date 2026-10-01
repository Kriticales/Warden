# ADR-0019 — Concorrência, travas por pack e cancelamento

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1)

## Contexto

No manager anterior, todo comando agia sobre um "pack atual" global e dois comandos do packwiz podiam rodar ao mesmo tempo no mesmo pack, sem tempo-limite nem cancelamento (R4 §2.3–§2.4).

## Decisão

- Todo comando recebe o `PackId` explicitamente; não existe "pack atual" no backend.
- Trava `RwLock` por pack: leituras em paralelo; escritas exclusivas e em fila (visíveis em Tarefas).
- Toda escrita no pack passa pela `PackTransaction` (ARCHITECTURE §6.5).
- Operações longas registradas com `OperationId` e `CancellationToken`; tempos-limite definidos (ARCHITECTURE §15).
- Um jogo de teste por vez no app; a trava do pack não fica presa durante o jogo.
- Uma única instância do app (`tauri-plugin-single-instance`).

## Alternativas consideradas

- Fila global única: simples, mas bloquearia packs diferentes sem motivo.
- Sem trava, confiando na interface: já falhou antes.

## Consequências

- Testes de concorrência obrigatórios para mutações.
- Comandos que esperam a trava informam o estado "aguardando outra operação".
