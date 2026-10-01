# ADR-0012 — Gestão de Java

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1)

## Contexto

Cada faixa do Minecraft exige um Java (8, 16/17, 21, 25) e o Forge antigo só roda em Java 8 (R2 §2). O runtime "legacy" da Mojang é um Java 8u51 de 2015. A pesquisa recomendou Temurin (R2 §2.5, §9 item 6).

## Decisão

- Fonte padrão: **Adoptium Temurin JRE** (8, 17, 21, 25), validado por SHA-256. Alternativa: runtime oficial da Mojang.
- Política de seleção (ARCHITECTURE §7.3): escolha manual → Forge ≤ 1.12.2 = 8 → Forge 1.16.5 < 36.2.26 = 8 ≤ u312 → `javaVersion` do JSON (16 vira 17) → 8.
- Javas instalados em `shared/runtimes/`, instalação atômica, validação executando o binário. Só 64 bits.
- Gestão de Java fica no Warden (`warden-java`), não no motor do launcher, que recebe o Java pronto.

## Alternativas consideradas

- Só o runtime da Mojang: Java 8 desatualizado; dependência de endpoint não documentado.
- Azul Zulu (Modrinth App) ou só Java do sistema: menos previsível para um usuário leigo.

## Consequências

- Primeiro teste de cada faixa baixa ~50 MB de Java.
- Java moderno para 1.7.10/1.12.2 (lwjgl3ify/Cleanroom) fica no backlog.
