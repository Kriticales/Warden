# ADR-0038 — Monitor de desempenho do jogo sem JDK

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** técnica (tarefa D4, pesquisa R5A §7)

## Contexto

O dono quer ver memória ao vivo, tempo de carregamento e o que pesa no jogo. Os JREs Temurin não trazem `jcmd`, `jstat` nem a Attach API, e JMX remoto abre uma porta que executa código.

## Decisão

- **RAM do processo** pela API do sistema (Windows `GetProcessMemoryInfo`; picos pelo Job Object) e **tempo total de carregamento** (até o marcador de "carregou" e até "entrou no mundo") em todo teste: P0.
- **Heap da JVM** lida do arquivo `hsperfdata` (contadores que a HotSpot publica por padrão, o mesmo que o `jstat` lê), por um leitor próprio de cerca de 150 linhas, sem anexar nada ao jogo: P1, depois do spike S-R5-2 no Windows com Java 8, 17, 21 e 25. Se os argumentos do teste tiverem `-XX:+PerfDisableSharedMem` ou `-XX:-UsePerfData`, o Warden avisa (e as flags do perfil podem ser removidas com um clique); OpenJ9 fica sem leitura.
- Achados `W_LOW_HEAP` e `I_HEAP_OVERSIZED` a partir da memória depois das coletas.
- **Tempo por mod** onde o loader já registra (Forge 1.12.2 por padrão; Forge e NeoForge modernos com `TRACE` só no "Testar com perfil de desempenho") e leitura dos `.sparkprofile` salvos pelo usuário (protobuf, com os `.proto` públicos do spark), mais o spark automático pelo stdin do servidor local: P1. O visualizador do spark abre o arquivo local no navegador; o Warden nunca faz upload.
- Fora da v1 (P2): perfil automático do cliente (JFR ou agente do spark) e tempo por mod no Fabric.

## Alternativas consideradas

- JMX remoto: abre porta e não tem cliente viável em Rust.
- Exigir um JDK: mais 200 MB por versão de Java e nada a mais para o usuário.
- Embutir o visualizador do spark (GPL-3.0): não é necessário, porque ele abre arquivos locais.

## Consequências

- Nova crate `warden-perf` (ARCHITECTURE §7.7); a faixa de desempenho na tela do teste consome as amostras a cada 1 s.
