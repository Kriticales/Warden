# ADR-0031 — Busca do culpado por rodadas (bisseção respeitando dependências)

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono + técnica (tarefa D4, pesquisa R5A §3)

## Contexto

Quando nem as regras determinísticas nem o log apontam a causa de um travamento, a comunidade tira metade dos mods e testa de novo, à mão. É lento, fácil de errar (tirar uma biblioteca troca o erro por outro) e ninguém anota as rodadas. O Warden conhece o grafo de dependências, controla o launcher e lê o log ao vivo, então pode automatizar isso.

## Decisão

- **Algoritmo:** busca por **prefixos de uma ordem topológica** do grafo de dependências obrigatórias (todo prefixo já contém as dependências dos seus mods), com rodada 0 (pack inteiro, até 3 tentativas para confirmar que o problema se repete), rodada de controle sem mods, busca binária no tamanho do prefixo, minimização para combinações de 2 ou mais mods e confirmação final (R5A §3.2).
- **Veredito automático por assinatura** do problema (regra do catálogo + tipo de exceção + primeiro frame de mod normalizado); resultado "travou diferente" é inconclusivo e não conta; `NoClassDefFoundError` vira dependência inferida no grafo.
- **Isolamento:** instância própria da busca (`instances/<pack-id>/bisect/`), mundos sempre em cópias descartáveis, pack e instância de teste nunca alterados. spark e Crash Assistant ficam desligados durante a busca, a menos que sejam suspeitos.
- **Tempo:** pode levar de 20 a 40 minutos num pack grande (decisão do dono D23). O Warden mostra a estimativa antes, explica o que está fazendo a cada rodada e permite **pausar** (o estado fica em `state/bisect.json`) e **cancelar** a qualquer momento, guardando o resultado parcial ("o culpado está entre estes N mods").
- **Modo assistido** para problemas que exigem uma ação no jogo: cada rodada termina com "Aconteceu · Não aconteceu · Não deu para testar".
- **Entrada no mundo:** cliente 1.20+ por Quick Play; versões anteriores ou problemas de servidor usam o servidor local (ADR-0032), só depois de explicar e pedir ao usuário.
- Fora da v1 (P2): bisseção de configs e scripts e bisseção por desempenho.

## Alternativas consideradas

- Metades arbitrárias com fechamento de dependências: a "metade" vira quase o pack inteiro quando muitos mods dependem das mesmas bibliotecas.
- Só busca guiada (o usuário julga cada rodada, como o mod-bisect-tool): mais lento e sujeito a erro; fica como modo assistido.
- Reaproveitar código de ferramentas existentes: nenhuma tem licença e linguagem compatíveis; o algoritmo é pequeno (R5A §3.9).

## Consequências

- Nova crate `warden-bisect` (algoritmo puro e estado) e orquestração na `warden-app` (ARCHITECTURE §9.7).
- O spike S-R5-3 fixa os marcadores de sucesso e falha por faixa de versão antes da implementação.
- A busca ocupa o launcher ("um jogo por vez"); o botão do cabeçalho mostra "Buscando o culpado: ver progresso".
