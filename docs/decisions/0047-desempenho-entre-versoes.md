# ADR-0047 — Desempenho entre versões

- **Status:** aceita · **Data:** 2026-10-02 · **Origem:** técnica (tarefa D5, decisão D32)

## Contexto

O dono quer saber se o pack ficou mais pesado de uma versão para outra. Os números dos testes variam muito com o perfil (memória, Java, argumentos), com o computador, com a primeira abertura depois de preparar a instância (caches frios) e com o registro detalhado do "Testar com perfil de desempenho" ([ADR-0038](0038-monitor-de-desempenho.md)). Comparar qualquer teste com qualquer outro daria avisos falsos.

## Decisão

- **Métricas por teste** em `perf/<pack-id>.jsonl`, nos dados locais do Warden: tempos para carregar e entrar no mundo, picos de memória, perfil (nome e assinatura), impressão do computador, modo, árvore do pack testada, marca de primeira abertura e, quando houver, o tempo por tick do spark.
- **Testes comparáveis:** modo normal, que chegaram a carregar, com o mesmo perfil (assinatura) e no mesmo computador (impressão local), fora a primeira abertura e os testes com perfil de desempenho.
- **Mediana** dos testes comparáveis por versão salva.
- **Critério de "mais pesada"** em relação à versão salva anterior com testes comparáveis: tempo para abrir pelo menos 20% maior e 10 s a mais; ou memória máxima pelo menos 20% maior e 512 MB a mais; ou tempo por tick do spark pelo menos 25% maior e 5 ms a mais. Mínimo de 2 testes comparáveis em cada versão; com menos, só "Poucos testes para comparar".
- Mora no **Histórico** (página de detalhe) e no aviso do resultado do teste ([ADR-0045](0045-funcoes-1-1-na-estrutura.md)). A linha "Desempenho" da saúde do pack passa a usar o mesmo critério.
- **Nada sai do computador.**
- Código em `warden-perf::history`.

## Alternativas consideradas

- Média: um teste ruim (antivírus rodando, outro programa pesado) puxa o valor e gera aviso falso.
- Comparar com qualquer teste: mistura perfis e máquinas.
- Guardar as métricas na pasta do pack: são dados de máquina, que ficam nos dados locais ([ADR-0021](0021-layout-em-disco.md)).
- Só percentual, sem o mínimo absoluto: em packs leves, 3 s a mais já dariam 20%.

## Consequências

- Depende dos ganchos da v1: assinatura do perfil, impressão do computador, marca de primeira abertura e a linha em `perf/<pack-id>.jsonl` que a poda das sessões não apaga ([ADR-0039](0039-warden-1-1-profissional.md), gancho 3). Sem eles, os testes feitos antes da 1.1 não entrariam no gráfico.
- A impressão do computador é um código reduzido, só local, sem identificar a máquina fora do Warden.
- SPEC T33.
