# ADR-0029 — Java: o mais novo que funciona, com o motivo à vista

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (comentário na página da estrutura, tarefa D2) · **Substitui em parte:** ADR-0012 (regra de escolha da versão)

## Contexto

A ADR-0012 escolhia o Java pela exigência da versão do Minecraft (`javaVersion` do JSON) e pelas regras dos loaders. O dono pediu "sempre o mais recente; se não for possível, explique o porquê". Usar sempre o Java mais novo quebraria packs antigos: o Forge 1.7.10/1.12.2 trava a partir do Java 9 (R2 §2.1).

## Decisão

- O Java automático de um pack é **o major mais novo que funciona** com a versão do Minecraft e o loader, segundo uma tabela de compatibilidade em dados versionados (`crates/warden-java/data/compatibility.toml`). Ponto de partida = R2 §2.1: Forge ≤ 1.12.2 → 8 (Forge 1.16.5 < 36.2.26 → 8 ≤ u312); até 1.16.5 → 8; 1.17–1.20.4 → 17; 1.20.5–1.21.x → 21; 26.x → 25. Uma faixa só passa a usar um major mais novo depois que a matriz de versões (ROADMAP L-05) passar com ele.
- Dentro do major, **sempre a atualização mais recente** do Temurin.
- A escolha vem com um **motivo** (código traduzido pela interface), mostrado em Ajustes do teste ("Por que não o Java 25?") e na tabela de Java de Configurações (cada Java, os packs que o usam e o porquê).
- O resto da ADR-0012 continua: Temurin como fonte, runtime da Mojang como alternativa, escolha manual do usuário prevalece, instalação atômica, só 64 bits.

## Alternativas consideradas

- Sempre o Java mais novo: packs antigos não abrem.
- Só a exigência mínima (ADR-0012): funciona, mas não usa um Java mais novo quando ele já foi provado e não explica a escolha.

## Consequências

- `warden-java` expõe a escolha com o motivo e verifica atualizações dos Javas instalados (ROADMAP L-01); novos critérios CA-T11-03 e CA-T21-04.
