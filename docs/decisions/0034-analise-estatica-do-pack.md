# ADR-0034 — Análise estática do pack: raio-x de mixins, grafo de dependências e nota de saúde

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** técnica + decisão do dono (tarefa D4, pesquisa R5A §5 e §6)

## Contexto

O diagnóstico P0 já lê os metadados dos jars. A R5A mostrou que o mesmo passo pode, com custo baixo, mapear pacotes e configs de mixin para cada mod, ler o que cada mixin altera no jogo (protótipo em Rust: 1.989 classes de 14 mods reais em 25 a 67 ms por conjunto) e montar o grafo de dependências. Também mostrou que **a maioria das sobreposições de mixin é compatibilidade intencional** (Sodium × Iris, Lithium × ModernFix): um detector ingênuo assusta sem motivo.

## Decisão

- **Índice pacote → mod e config de mixin → mod** (P0), montado na passagem completa do diagnóstico, com cache por hash do jar. Serve ao console agrupado, à dependência inferida da busca do culpado e ao padrão "falha de Mixin" do catálogo.
- **Raio-x de mixins, camadas A e B** (P1): inventário por mod ("altera 214 partes do jogo") e sobreposições classificadas por regras explicáveis (R5A §5.5), com rebaixadores (plugin de mixin, compatibilidade intencional, lista curada `mixin-known-compatible.toml`) e elevadores (conflito conhecido, travamento que citou a config). **Só avisos, nunca bloqueio.** A interface diz "alteram o mesmo ponto do jogo", nunca "são incompatíveis". Leitor de `.class`: crate `cafebabe` (0BSD). O intermed (Rust, MIT) é avaliado no spike S-R5-1 antes; se servir, é usado como dependência ou portado com atribuição.
- **Grafo de dependências:** consultas (quem depende, por que está no pack, bibliotecas órfãs) são P0, porque o diálogo de remover já precisa delas; o desenho ("Mods → Ver como: Grafo") é P1, focado num mod, com a explicação em texto ao lado.
- **Nota de saúde** (P1): começa em 100 e cada categoria tira pontos até um teto (R5A §6.2), com pesos em dados versionados (`health-score.toml`) e teste de propriedade (um erro a mais nunca aumenta a nota). Aparece em Problemas e na coluna Saúde de Meus packs, sempre com "O que tirou pontos" e o texto "Resumo dos problemas conhecidos. Não garante que o pack funciona."
- Fora da v1 (P2): tradução de nomes sem refmap (camada D) e aplicação a seco do Mixin (camada E).

## Alternativas consideradas

- Bloquear o teste por sobreposição de mixin: falsos positivos frequentes; o usuário aprenderia a ignorar.
- Grafo inteiro como visão padrão: ilegível com 300 nós; vira opção.
- Nota calculada pela IA: não explicável e instável.

## Consequências

- Nova crate `warden-mixin`; índice de pacotes em `warden-jarmeta`; grafo, nota e assinatura de travamento em `warden-diagnostics` (ARCHITECTURE §9.6).
- Os resultados viram ferramentas da IA (ADR-0030).
