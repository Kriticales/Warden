# Arquivos reais do perfil de desempenho (L-12)

Servidores dedicados reais rodados em 18/10/2026 num contêiner Linux, com os instaladores oficiais (`--installServer`), o mundo plano e os mods abaixo, baixados do Modrinth. Cada servidor abriu, ficou parado alguns segundos e recebeu pelo console (stdin) os mesmos comandos que o Warden manda no "Testar como servidor" com o perfil de desempenho: `spark profiler start --timeout 40 --save-to-file`, `spark heapsummary --save-to-file` e `stop`. O código do FML que grava as linhas de tempo por mod é o mesmo no cliente e no servidor (o cliente fica no roteiro manual do relatório da L-12).

| Arquivo | Origem | O que mudou |
|---|---|---|
| `logs/forge-1.12.2-servidor-debug.log` | Forge 1.12.2-14.23.5.2860, Java 8 (Temurin 8u504); JEI 4.22.0.1036, AppleSkin 1.0.14, Clumps 3.1.2; `logs/debug.log` | pasta do servidor → `<DADOS>`, pasta do Java → `<JAVA>` |
| `logs/forge-1.20.1-servidor-debug.log` | Forge 1.20.1-47.4.10, Java 21, `-Dforge.logging.debugFile.level=trace`; spark 1.10.53, JEI 15.71.0.224, FerriteCore 6.0.1, Clumps 12.0.0.4; `logs/debug.log` | pasta do servidor → `<DADOS>`; sem as linhas `[mixin/]: Added class metadata for …` (só tamanho) |
| `logs/neoforge-1.21.1-servidor-debug.log` | NeoForge 21.1.252, Java 21, `-Dforge.logging.debugFile.level=trace`; spark 1.10.124, JEI 19.64.0.455, FerriteCore 7.0.3, Clumps 19.0.0.1; `logs/debug.log` | pasta do servidor → `<DADOS>`; sem as linhas do mixin acima; das linhas `Firing/Fired event for phase …` (uma por prioridade de evento, 6.200 no original) ficaram as 60 primeiras |
| `logs/forge-1.7.10-fml-server-latest.log` | Forge 1.7.10-10.13.4.1614, Java 8; WorldEdit 6.1.1; `logs/fml-server-latest.log` | pasta do servidor → `<DADOS>`, pasta do Java → `<JAVA>` |
| `spark/forge-1.20.1-servidor.sparkprofile`, `spark/forge-1.20.1-servidor.sparkheap` | o servidor Forge 1.20.1 acima: `config/spark/profile-….sparkprofile` e `heapsummary-….sparkheap` | lidos e gravados de novo com o código de `src/spark/generated.rs`, com os argumentos da JVM trocados pelos do teste (`-Xmx2G -Dforge.logging.debugFile.level=trace`; os originais traziam a configuração de rede do contêiner) e o `server.properties` copiado pelo spark apagado |
| `spark/neoforge-1.21.1-servidor.sparkprofile` | o servidor NeoForge 21.1.252 acima | idem |

Os logs e os perfis não têm nome de pessoa: o perfil diz que o comando veio do console (`Console`/`Server`). `.gitattributes` desta pasta marca tudo como `-text`: os bytes fazem parte do caso. Os jars não estão no repositório; os mods só aparecem como nomes nos logs e como classes nos perfis.
