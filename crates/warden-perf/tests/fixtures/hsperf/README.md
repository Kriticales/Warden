# Fixtures `hsperfdata` (spike S-R5-2)

Copiadas sem mudança de `spikes/s-r5-2/fixtures/` da branch `Kriticales/spike-s-r5-2-hsperfdata` (commit `4a1c078`) para a L-11 (ROADMAP L-11, item 8). Fixtures novas passam pelo `hsperf-probe anonymize` do spike antes de entrar aqui (S-R5-2 §9).

Arquivos `hsperfdata` reais, gravados em 04/10/2026 no Windows 11 (x64, code page ANSI 1252) pelo `hsperf-probe dump`, a partir do *file mapping* nomeado de uma JVM rodando o `java/Alloc.java` com `-Xmx1024m`, logo depois do comando `hold 200` (200 MB vivos em blocos de 64 KB) e com o programa parado. Cada um tem ao lado a saída do `jstat -gc <pid>` (JDK Temurin 25.0.4.1) tirada em seguida, no mesmo estado: `<nome>.jstat.txt` (decimais com vírgula, CRLF, como o `jstat` imprimiu nesta máquina).

Só a parte usada do buffer foi gravada (o campo `used` do cabeçalho; a vista inteira tem 64 KB e o resto é zero).

## Dados pessoais

- O texto do contador `java.property.java.library.path` (o PATH inteiro do Windows) foi trocado por `<PATH removido>`, completado com bytes NUL até o tamanho original, para não mudar nenhum deslocamento. Era o único lugar com o nome do usuário do Windows (14 ocorrências).
- `hsperf-probe anonymize` confere que o buffer continua válido depois da troca. Nenhum outro contador tem nome de usuário, nome do computador ou caminho pessoal: os caminhos que restam são da pasta temporária do spike (`C:\wt\s-r5-2\...`).
- O nome do usuário não aparece no conteúdo do `hsperfdata`; ele só aparece no nome da pasta (`hsperfdata_<usuário>`) e do *file mapping*.

## Arquivos

| Arquivo | Java | Coletor | Contadores | Bytes | Heap usada (KB) | Coletas |
|---|---|---|---|---|---|---|
| `java8-parallel.hsperfdata` | Temurin 1.8.0_504 | Parallel (padrão do Java 8) | 289 | 21.584 | 208.399,4 | 1 |
| `java8-g1.hsperfdata` | Temurin 1.8.0_504 | G1 | 255 | 19.600 | 217.984,5 | 6 |
| `mojang8u51-g1.hsperfdata` | Mojang `jre-legacy` 1.8.0_51 (Oracle) | G1 com os argumentos do launcher oficial (`G1NewSizePercent=20`, `G1ReservePercent=20`, `MaxGCPauseMillis=50`, `G1HeapRegionSize=32M`) | 255 | 19.824 | 192.327,0 | 2 |
| `java17-g1.hsperfdata` | Temurin 17.0.20.1 | G1 | 187 | 13.200 | 217.536,5 | 6 |
| `java21-g1.hsperfdata` | Temurin 21.0.12.1 | G1 | 187 | 13.224 | 217.664,1 | 6 |
| `java25-g1.hsperfdata` | Temurin 25.0.4.1 | G1 | 185 | 13.144 | 217.988,4 | 1 |
| `java25-zgc.hsperfdata` | Temurin 25.0.4.1 | ZGC (geracional) | 142 | 10.248 | 227.328,0 | 10 |
| `java25-shenandoah.hsperfdata` | Temurin 25.0.4.1 | Shenandoah | 137 | 9.896 | 208.561,7 | 0 |

"Heap usada" é a soma de `sun.gc.generation.*.space.*.used`; "Coletas" é a soma de `sun.gc.collector.*.invocations`.

## Como usar nos testes

- O teste `fixtures_match_jstat` (`tests/hsperf_fixtures.rs`; no spike, `src/perfdata.rs`) confere, para cada fixture, todas as colunas do `jstat -gc` (S0C, S1C, S0U, S1U, EC, EU, OC, OU, MC, MU, CCSC, CCSU, YGC, YGCT, FGC, FGCT, CGC, CGCT) contra os contadores lidos: diferença máxima de 0,05 KB nos tamanhos (arredondamento do `jstat`), 0 nas contagens e 0,0005 s nos tempos. `cargo test` passa.
- `leitura_do_warden_em_cada_java_e_coletor` confere a leitura que o Warden usa (heap usada, máximo, coletas, tempo, coletor e versão do Java) contra esta tabela e o `jstat`; `fixtures_cortadas_ou_corrompidas_nao_dao_panico` corta e corrompe cada arquivo; `fixtures_sem_dados_pessoais` confere a anonimização.
- O programa `../java/WardenHeapProbe.java` (compilado para Java 8 em `WardenHeapProbe.class`, `javac --release 8 -encoding UTF-8 -d . WardenHeapProbe.java`) é o dos testes com a JVM de verdade (`tests/hsperf_jvm_real.rs`).
