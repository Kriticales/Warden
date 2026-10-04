# Spike S-R5-2 — Memória da JVM sem JDK no Windows

> Tarefa S-R5-2 (onda 0), 04/10/2026. Branch `Kriticales/spike-s-r5-2-hsperfdata`; código descartável em `spikes/s-r5-2/` (fica na branch). Executado direto no Windows 11 Pro desta máquina (ADR-0048), code page ANSI 1252, sem instalar nada: os Javas foram baixados em zip para uma pasta temporária e apagados no fim.
>
> Marcas: **[verificado]** = executei e vi o resultado; **[código]** = li no código do OpenJDK; **[inferência]** = conclusão minha, não executada.

## Resposta curta

**Sim.** Um leitor próprio em Rust lê o `hsperfdata` pelo *file mapping* nomeado do Windows, sem JDK e sem anexar nada à JVM, com o Java 8, 17, 21 e 25 (Temurin) e também com os runtimes da Mojang (8u51 e 25.0.1) **[verificado]**:

- Com o programa parado, a heap usada lida pelo Warden e a do `jstat -gc` de um JDK são **idênticas** (diferença de 0,00%) em todas as 192 medições (64 etapas × 3 rodadas), em 16 combinações de Java e coletor (G1, Parallel, Serial, CMS, ZGC, Shenandoah). As coletas também batem sempre **[verificado]**.
- Com o programa alocando ~200 MB/s, a leitura feita logo depois de o `jstat` terminar bate (±0,5%) em 101 de 128 medições; as outras diferem porque o programa alocou entre uma leitura e outra (as duas leituras não são no mesmo instante) **[verificado]**. O critério CA-T13-10 (5%) tem de ser medido com o jogo parado.
- A leitura custa 0,3–0,9 ms (cópia de 64 KB) e o `hsperfdata` fica legível ~50–70 ms depois de a JVM abrir **[verificado]**.
- **Usuário acentuado:** "João", "João Ç" (com espaço) e pasta temporária acentuada funcionam, pelo mapeamento e pelo arquivo; o `jstat` também **[verificado por simulação, sem criar usuário]**. Nomes com letras fora da code page ANSI (cirílico, chinês num Windows em 1252) fazem a **JVM** desistir em silêncio de publicar os contadores: nem o Warden nem o `jstat` conseguem ler **[verificado por simulação]**.
- `-XX:+PerfDisableSharedMem` e `-XX:-UsePerfData` desligam a publicação em todas as versões: não há arquivo nem mapeamento, e o `jstat` também falha **[verificado]**.
- **Surpresa:** no Java 25 com os coletores Parallel ou Serial, o "usado" do eden só é atualizado nas coletas (o `jstat` mostra o mesmo valor velho). G1, ZGC e Shenandoah, e o Parallel do 8 ao 21, atualizam sozinhos **[verificado + código]**.

## 1. Javas usados

| Java | Fonte | Download | Extraído |
|---|---|---|---|
| Temurin JRE 1.8.0_504-b01 | API do Adoptium, zip, SHA-256 conferido | 40,1 MB | 98 MB |
| Temurin JRE 17.0.20.1+1 | idem | 43,8 MB | 126 MB |
| Temurin JRE 21.0.12.1+1 | idem | 49,0 MB | 146 MB |
| Temurin JRE 25.0.4.1+1 | idem | 58,5 MB | 180 MB |
| Temurin **JDK** 25.0.4.1+1 (só para `jstat` e `javac`) | idem | 141,2 MB | 291 MB |
| Mojang `jre-legacy` 1.8.0_51 (Java(TM) da Oracle, o que o S1 viu no Windows) | manifesto `java-runtime` da Mojang, 192 arquivos, SHA-1 conferido | 150,2 MB | 144 MB |
| Mojang `java-runtime-epsilon` 25.0.1 (build da Microsoft) | idem, 411 arquivos | 105,1 MB | 102 MB |

Fonte de cada um: o Temurin é a fonte padrão do Warden (ADR-0012); os da Mojang são a alternativa e o que o portablemc usa por padrão (S1). Total em disco durante o spike: ~1,46 GB (332,5 MB de zips + 1,13 GB na pasta temporária, quase tudo Javas extraídos), tudo apagado no fim (§10).

O `jstat` do JDK 25 leu todas as JVMs, inclusive o Java 8 e o 8u51 **[verificado]**: o formato é o mesmo (versão 2.0 do PerfData) do 8 ao 25.

## 2. Como a JVM publica os contadores no Windows

Lido em `src/hotspot/os/windows/perfMemory_windows.cpp` do jdk8u, jdk17u, jdk21u, jdk25u e do `jdk` principal **[código]**; o comportamento é o mesmo em todos:

- **Nome do usuário:** `getenv("USERNAME")`; se vazio, `GetUserName` (ANSI). Não é o dono do processo: é a variável de ambiente.
- **Pasta:** `<GetTempPath()>\hsperfdata_<usuário>` (`os::get_temp_directory()` chama `GetTempPath`, que respeita `TMP` e `TEMP`). A HotSpot não é compilada em Unicode: `GetTempPath`, `CreateDirectory`, `CreateFile` e `CreateFileMapping` são as versões **ANSI** (o efeito aparece na §6).
- **Arquivo:** `<pasta>\<pid>`, criado com `GENERIC_READ|GENERIC_WRITE`, compartilhamento `FILE_SHARE_READ|FILE_SHARE_DELETE` e `FILE_FLAG_DELETE_ON_CLOSE`.
- ***File mapping*:** `CreateFileMapping` sobre esse arquivo com o nome `hsperfdata_<usuário>_<pid>` (pid sem sinal), sem prefixo `Global\` (fica no espaço da sessão).
- **Se algo falhar** (pasta, arquivo ou mapeamento), `PerfMemory::create_memory_region` volta para memória privada (`FLAG_SET_ERGO(PerfDisableSharedMem, true)`), sem mensagem (só com `PrintMiscellaneous` e `Verbose`). O jogo segue normal; só os contadores ficam invisíveis.
- **Mapeamento antigo com o mesmo nome:** se `CreateFileMapping` devolve `ERROR_ALREADY_EXISTS`, a JVM fecha o handle e também cai para memória privada.
- **O `jstat`** (`open_file_mapping` → `get_user_name_slow`) não monta o nome a partir do próprio usuário: procura em `%TEMP%` as pastas `hsperfdata_*` que tenham o arquivo `<pid>`, fica com a mais nova e usa o sufixo da pasta como nome de usuário. O leitor do spike faz o mesmo.

## 3. O leitor de prova

`spikes/s-r5-2/` (Rust 1.98.1, `windows-sys` 0.59; sem outras dependências):

- `src/perfdata.rs` (~150 linhas de código, mais testes): lê o cabeçalho (magic `CA FE C0 C0`, ordem dos bytes, versão 2.x, `accessible`, `used`, `mod_time_stamp`, `entry_offset`, `num_entries`) e as entradas (`J` = long, `B` com vetor = texto terminado em NUL, na code page ANSI), com todos os deslocamentos conferidos contra o tamanho do buffer (buffer ruim vira erro, nunca pânico).
- `src/win.rs` (~140 linhas): `locate` procura `<temp>\hsperfdata_*\<pid>` e monta o nome do mapeamento com o sufixo da pasta; `Mapping::open` faz `OpenFileMappingW(FILE_MAP_READ)` + `MapViewOfFile` + `VirtualQuery` (para o tamanho), e `snapshot` copia a vista; `read_file` é o caminho alternativo, abrindo o arquivo com compartilhamento total. Os blocos `unsafe` são curtos e todos têm comentário `SAFETY`.
- `src/main.rs`: CLI `hsperf-probe` com `read`, `counters`, `dump`, `compare` (lê, roda o `jstat`, lê de novo), `parse` e `anonymize`.
- `java/Alloc.java` aloca sob comando (`hold`, `free`, `garbage`, `churn`, `gc`, `mem`); `run-case.ps1` roda um caso; `summarize.mjs` monta a tabela.
- `cargo test --release`: 3 testes, todos passam (inclusive `fixtures_match_jstat`, §9); `cargo clippy` sem avisos.

Os dois caminhos (mapeamento e arquivo) deram os mesmos números em todos os casos em que testei os dois (§6) **[verificado]**. Ler pelo mapeamento custou 0,33–0,94 ms; pelo arquivo, 2,7–3,8 ms **[verificado]**.

Leitura concorrente: a JVM grava os contadores enquanto o Warden copia. No x64, cada `i64` alinhado é gravado de uma vez, então nenhum contador sai "cortado"; contadores diferentes podem vir de instantes ligeiramente diferentes **[inferência]**. Não apareceu nenhum valor incoerente nas mais de 600 leituras do spike **[verificado]**.

## 4. Comparação com o `jstat`

**Método** **[verificado]**: para cada Java e coletor, a JVM roda `Alloc` com `-Xmx1024m` e passa por 5 etapas: início; `hold 200` (200 MB vivos); `garbage 300` (300 MB de lixo); `gc`; `churn 8` (alocando ~200 MB/s por 8 s). Em cada etapa, `hsperf-probe compare` faz 3 rodadas (8 no churn) de: leitura A → `jstat -gc <pid>` (JDK 25; leva 120–570 ms porque abre uma JVM) → leitura B. A heap usada do `jstat` é S0U + S1U + EU + OU (colunas "-" contam 0); a do leitor é a soma de `sun.gc.generation.*.space.*.used`. Saída completa: `spikes/s-r5-2/results/*.jsonl` e `results/tabela.md`.

### 4.1 Programa parado (etapas início, hold200, lixo300, gc)

Todas as 64 linhas (16 casos × 4 etapas, 3 rodadas cada) deram diferença de **0,00%** entre leitor e `jstat`, antes e depois, e o mesmo número de coletas. Valores das etapas hold200 e gc (MB):

| Caso | Java | Coletor | hold200: leitor = jstat | gc: leitor = jstat | Coletas (gc) |
|---|---|---|---|---|---|
| j8-parallel | Temurin 8u504 | Parallel (padrão do 8) | 203,5 | 205,5 | 4 |
| j8-g1 | Temurin 8u504 | G1 | 212,9 | 200,5 | 9 |
| j8-cms | Temurin 8u504 | CMS | 200,7 | 203,2 | 7 |
| mojang8u51-g1 | Mojang 8u51 | G1 com os argumentos do launcher oficial | 187,8 | 200,4 | 8 |
| j17-g1 | Temurin 17 | G1 | 212,4 | 201,1 | 9 |
| j17-g1-joao | Temurin 17, `USERNAME=João` | G1 | 211,9 | 201,1 | 9 |
| j21-g1 | Temurin 21 | G1 | 212,6 | 201,0 | 9 |
| j21-zgc | Temurin 21 | ZGC (não geracional) | 216,0 | 216,0 | 15 |
| j21-zgc-gen | Temurin 21 | ZGC geracional | 218,0 | 218,0 | 23 |
| j21-shenandoah | Temurin 21 | Shenandoah | 204,4 | 202,5 | 66 |
| j25-g1 | Temurin 25 | G1 | 212,9 | 201,5 | 5 |
| j25-zgc | Temurin 25 | ZGC (só geracional) | 222,0 | 220,0 | 23 |
| j25-shenandoah | Temurin 25 | Shenandoah | 203,7 | 202,3 | 73 |
| j25-parallel | Temurin 25 | Parallel | **121,2** | 201,4 | 4 |
| j25-serial | Temurin 25 | Serial | **126,9** | 201,5 | 4 |
| mojang25-g1 | Mojang 25.0.1 | G1 | 212,5 | 201,1 | 5 |

Os 121,2 e 126,9 MB com 200 MB vivos não são erro do leitor: o `jstat` mostra o mesmo, porque no Java 25 o Parallel e o Serial só atualizam o eden nas coletas (§5.3). O 187,8 do 8u51 também é igual ao do `jstat` (causa não investigada; G1 do Java 8 com regiões de 32 MB).

### 4.2 Programa alocando (etapa churn, 8 rodadas por caso)

| Caso | Leitura depois = jstat (±0,5%) | Maior diferença da leitura depois |
|---|---|---|
| j8-parallel | 3/8 | 53,7% |
| j8-g1 | 3/8 | 8,7% |
| j8-cms | 7/8 | 5,6% |
| mojang8u51-g1 | 8/8 | 0,0% |
| j17-g1 | 5/8 | 42,9% |
| j17-g1-joao | 6/8 | 9,2% |
| j21-g1 | 3/8 | 7,2% |
| j21-zgc / j21-zgc-gen | 8/8 / 8/8 | 0,0% |
| j21-shenandoah | 6/8 | 7,6% |
| j25-g1 | 6/8 | 9,3% |
| j25-zgc | 8/8 | 0,0% |
| j25-shenandoah | 7/8 | 8,2% |
| j25-parallel / j25-serial | 8/8 / 8/8 | 0,0% (eden parado, §5.3) |
| mojang25-g1 | 7/8 | 40,5% |

O programa aloca 20 MB de uma vez a cada 100 ms; quando um desses blocos cai entre o fim do `jstat` e a leitura B, a diferença aparece (até uma coleta inteira, daí os 40–54%). As coletas contadas ficaram sempre entre A e B. Conclusão: os números são os mesmos contadores; a diferença em movimento é só de instante **[verificado]**. Um teste de "até 5% do `jstat`" (CA-T13-10) precisa de um momento estável.

## 5. Quais contadores usar

### 5.1 Nomes por coletor **[verificado]**

| Coletor | Gerações e espaços | Coletores (`sun.gc.collector.N.name`) | `sun.gc.policy.name` |
|---|---|---|---|
| Parallel | 0 `new` (eden, s0, s1); 1 `old` | 0 `PSScavenge` (8) / `Parallel young collection pauses` (25); 1 `PSParallelCompact` | `ParScav:MSC` |
| Serial | idem | 0 `Copy` (8) / `Serial young collection pauses` (25); 1 | `Copy:MSC` |
| G1 | 0 `young` (eden, s0, s1); 1 `old` | 0 `G1 incremental collections` (8) / `G1 young collection pauses` (17+); 1 full; 2 `G1 concurrent cycle pauses` (17+) | `GarbageFirst` |
| ZGC (25) | 0 `young` (1 espaço); 1 `old` (1 espaço) | 0 `ZGC minor collection pauses`; 2 `ZGC major collection pauses` (não há 1) | ausente |
| Shenandoah (25) | 0 `Young` (0 espaços); 1 `Heap` (1 espaço) | 0 `Shenandoah partial`; 1 `Shenandoah full` | ausente |

### 5.2 Regras que bateram em todos os casos

- **Heap usada** = soma de `sun.gc.generation.*.space.*.used` (funciona em todos os coletores, inclusive onde o `jstat` mostra "-") **[verificado]**.
- **Coletas** = soma de `sun.gc.collector.*.invocations`; **tempo** = soma de `sun.gc.collector.*.time` ÷ `sun.os.hrt.frequency` (10.000.000 nesta máquina); os índices podem ter buracos (ZGC não tem o 1) **[verificado]**. No ZGC e no Shenandoah os contadores são de **pausas**, não de ciclos: 300 MB de lixo deram ~55 "coletas" no Shenandoah **[verificado]**.
- **Máximo da heap** **[verificado]**: Parallel e Serial = **soma** dos `sun.gc.generation.N.maxCapacity` (357.564.416 + 716.177.408 = 1 GiB com `-Xmx1024m`); G1, ZGC e Shenandoah = **maior** deles (cada geração informa a heap inteira; no Java 8 o G1 soma 8 a 24 bytes). Regra: com `sun.gc.policy.name` igual a `Copy:MSC` ou `ParScav:MSC`, somar; nos outros, o maior. O caminho principal continua sendo o `-Xmx` que o Warden passou (`java.rt.vmArgs` também traz os argumentos, por exemplo `"-Xmx1024m -XX:+UseG1GC"`). No CMS o máximo não foi conferido.
- **Metaspace** (`sun.gc.metaspace.used`) bate com o MU do `jstat`, mas no G1 do 21 ficou 0 até a primeira coleta **[verificado]**; não serve para gráfico contínuo.

### 5.3 Java 25 com Parallel ou Serial

Com 100 MB vivos e nenhuma coleta **[verificado]** (`results/atualizacao-dos-contadores.txt`):

| Java e coletor | Heap vista de dentro (Runtime) | Leitor logo depois | Leitor 2 s depois |
|---|---|---|---|
| 8, 17 e 21 Parallel | 106–120 MB | 18–84 MB (atrasado) | igual ao Runtime |
| 25 Parallel | 112,6 MB | 0 | **0** |
| 25 Serial | 115,2 MB | 0 | **0** |
| 25 G1 | 110,8 MB | 109,7 MB | 109,7 MB |
| 25 ZGC | 116,7 MB | 114,7 MB | 114,7 MB |
| 25 Shenandoah | 106,0 MB | 0 (atrasado) | 106,0 MB |

Depois de uma coleta, todos voltam a bater com o Runtime (diferenças de até 2,5 MB). Causa **[código]**: no jdk21u, `gc/parallel/spaceCounters.cpp` e `gc/serial/cSpaceCounters.cpp` criam o contador `used` com um `UsedHelper` que a JVM amostra periodicamente; no jdk25u esse ajudante não existe mais, e o `used` só muda em `update_used()`, chamado nas coletas. Consequência: no Java 25 com Parallel ou Serial, "Memória do jogo" entre coletas fica velha; o valor **depois de cada coleta** continua certo, e é ele que o `W_LOW_HEAP` usa. O Serial só é escolhido sozinho em máquina com 1 CPU ou menos de ~1,8 GB **[inferência]**; o Minecraft costuma rodar com G1.

## 6. Nome de usuário com acento

O dono não tem acento no nome e criar usuário no Windows é proibido. Como a JVM tira o nome da variável `USERNAME` e a pasta de `TMP`/`TEMP` (§2), dá para simular o caso trocando essas variáveis **só no processo da JVM** (`run-case.ps1 -Env`). Isso reproduz exatamente as chamadas que a JVM faz com um usuário real acentuado; o que não fica testado é o resto do sistema de um usuário real (perfil, permissões), que não entra no `hsperfdata` **[inferência]**.

Resultados (`results/usuario-acentuado.txt`; Java 8 e 25, e Java 21 nas linhas de TEMP):

| `USERNAME` | TEMP | Pasta criada | Leitor (mapeamento e arquivo) | `jstat` |
|---|---|---|---|---|
| `João` | padrão | `hsperfdata_João` | lê, mesmos números | lê |
| `João Ç` | `...\Usuários\João Ç\AppData\Local\Temp` | `hsperfdata_João Ç` | lê | não acha (procura no TEMP dele, outro) |
| `Łukasz` (Ł fora da 1252) | padrão | `hsperfdata_Lukasz` (o Windows trocou Ł por L) | lê | lê |
| `Иван` (cirílico) | padrão | **nenhuma** | não há o que ler | falha |
| `王伟` (chinês) | padrão | **nenhuma** | não há o que ler | falha |
| (sem `USERNAME`) | padrão | `hsperfdata_<usuário>` (via `GetUserNameA`) | lê | lê |
| (o do dono, sem acento) | `...\Иван1\Temp` (cirílico) | **nenhuma** | não há o que ler | — |
| (o do dono, sem acento) | o mesmo, como caminho curto 8.3 (`C:\wt\s-r5-2\2964E~1\Temp`) | `hsperfdata_<usuário>` | lê | — |
| `Иван` | caminho curto 8.3 | **nenhuma** | não há o que ler | — |
| `Ivan` | caminho curto 8.3 | `hsperfdata_Ivan` | lê | — |

O que isso prova **[verificado]**:

1. Montar o nome do mapeamento a partir da pasta encontrada (`hsperfdata_*\<pid>`) funciona com acento, espaço e cedilha: o Windows converte o nome ANSI que a JVM usou para o mesmo Unicode que o `read_dir` do Rust devolve, e `OpenFileMappingW` acha o objeto.
2. Letras que não existem na code page ANSI: o Windows troca por uma parecida quando há (Ł → L) ou por `?`, que não vale em nome de pasta; aí a JVM cai para memória privada em silêncio (o jogo roda normal, `stderr` vazio). O mesmo vale para um TEMP com essas letras.
3. A variável `USERNAME` também muda o `user.name` do Java (`java_props_md.c` usa `_wgetenv(L"USERNAME")`) **[código + verificado: `user.name = João`]**.

O que é **[inferência]**: um brasileiro com acento no nome de usuário num Windows em português (code page 1252) cai no caso 1 e funciona. Um Windows com "UTF-8 para suporte a idiomas mundiais" ligado (code page 65001) deve funcionar para qualquer nome, porque o ANSI vira UTF-8 (não testado: exigiria mudar configuração do sistema). Um usuário com nome cirílico num Windows em 1252 (caso raro) não tem leitura sem contorno; o contorno testado é passar ao jogo `TMP`/`TEMP` em caminho curto 8.3 e `USERNAME` em ASCII, devolvendo o nome real ao Java com `-Duser.name=<nome real>` (essa última parte não foi testada).

## 7. `-XX:+PerfDisableSharedMem` e `-XX:-UsePerfData`

Nos 4 Javas, com cada uma das flags: nenhum `hsperfdata_*\<pid>`, o leitor responde "nenhum ...\hsperfdata_*\<pid>" e o `jstat` dá `MonitorException: Could not attach to <pid>` **[verificado]** (`results/perf-disable.txt`). Não dá para distinguir, de fora, "flag desligou" de "a JVM caiu para memória privada" (§2, §6) ou "não é HotSpot" (OpenJ9) **[código + inferência]**; o Warden distingue pelo que ele mesmo sabe: os argumentos que passou e o Java que escolheu.

O que o Warden deve mostrar: na faixa, "Memória do jogo: indisponível" com o motivo em linguagem simples, por exemplo **"Os argumentos do teste têm -XX:+PerfDisableSharedMem, que esconde a memória do jogo."** e o botão de Ajustes do teste que remove o argumento (CA-T13-10). Com `-XX:-UsePerfData`, o mesmo texto com a outra flag. Sem flag e sem leitura depois de alguns segundos: "O Java deste teste não publica a memória do jogo." (OpenJ9, nome fora da code page ou outro motivo); a RAM do processo continua aparecendo.

## 8. Ciclo de vida

- O `hsperfdata` fica legível 52–68 ms depois de `Process.Start` (Java 8, 17, 21 e 25), antes de o `main()` do programa responder (124–148 ms) **[verificado]**. O Warden pode começar a tentar logo depois de abrir o processo e repetir a cada amostra (1 s) até achar.
- Ao sair normalmente ou ser morto com `TerminateProcess` (como o Job Object do Warden faz), o arquivo some na hora (`FILE_FLAG_DELETE_ON_CLOSE`) **[verificado]**.
- **Fechar o mapeamento quando o jogo sair.** Se o Warden ficar com o mapeamento aberto e uma JVM nova pegar o mesmo pid, a JVM nova encontra o nome ocupado (`ERROR_ALREADY_EXISTS`) e cai para memória privada **[código; não reproduzido, porque forçar reuso de pid não é prático]**.
- Pastas `hsperfdata_*` com o mesmo `<pid>` em mais de um usuário (arquivo velho de outra sessão): ficar com a mais nova, como o `jstat`, e só aceitar se `OpenFileMappingW` abrir (arquivo sem JVM viva não tem mapeamento) **[código + inferência]**.

## 9. Fixtures para a L-11

Em `spikes/s-r5-2/fixtures/` (8 arquivos, 9,9–21,6 KB cada, ~121 KB no total), com a saída do `jstat -gc` de cada um ao lado e um README com a origem: Temurin 8 (Parallel e G1), Mojang 8u51 (G1 com os argumentos do launcher), Temurin 17, 21 e 25 (G1), Temurin 25 ZGC e Shenandoah. Gravados pelo mapeamento no estado "200 MB vivos, programa parado".

- Dados pessoais: o nome do usuário só aparecia no `java.property.java.library.path` (o PATH do Windows inteiro, 14 ocorrências); esse texto virou `<PATH removido>` com NUL até o tamanho original, para não mexer em deslocamentos. Nenhuma outra ocorrência do usuário nem do nome do computador **[verificado com `grep -a -i`]**.
- O teste `fixtures_match_jstat` confere 18 colunas do `jstat` por fixture (tamanhos até 0,05 KB, contagens exatas, tempos até 0,0005 s) e passa **[verificado]**.
- Atenção para fixtures futuras com o Minecraft: `sun.rt.javaCommand` traz a classe principal **e os argumentos** do jogo (nome do jogador, caminhos), e `java.class.path`, os caminhos da instância **[código + inferência; aqui era só `Alloc`]**. Precisam ser redigidos como o `library.path`.

## 10. Limpeza

Apagados no fim **[verificado]**: `C:\wt\s-r5-2\` (1.130.470.691 bytes: Javas extraídos, classes compiladas, dumps brutos e pastas da simulação de usuário), os zips do scratchpad (332.526.420 bytes) e as pastas `hsperfdata_João` e `hsperfdata_Lukasz` que os testes criaram no `%TEMP%` real. Ficou só a pasta `spikes/s-r5-2/target` (17 MB, ignorada pelo git) no worktree. Nada foi instalado; o antivírus (Kaspersky) não bloqueou nada.

## 11. Achados fora do escopo (para o orquestrador)

- **ARCHITECTURE §7.7** diz "máximo = `-Xmx` passado pelo Warden": certo, mas convém registrar a regra de reserva da §5.2 (somar só em Parallel/Serial) e que "coletas" no ZGC/Shenandoah são pausas.
- **ARCHITECTURE §7.7 / R5A §7.3:** além de `-XX:+PerfDisableSharedMem` e `-XX:-UsePerfData`, a leitura some com nome de usuário ou TEMP fora da code page ANSI (§6) e com mapeamento velho segurado pelo próprio Warden (§8).
- **CA-T13-10:** "no mesmo instante" precisa virar "com o jogo parado" (ou "leitura logo depois do `jstat`") para ser testável; em movimento a diferença passa de 5% por puro atraso (§4.2). A CI Windows precisa de um JDK para o `jstat` (baixar o zip do Temurin como aqui, ou `actions/setup-java`) **[inferência sobre a CI]**.
- **Java 25 com Parallel/Serial** (§5.3): se a L-04 (perfil de teste) não fixar o coletor, a faixa pode mostrar heap velha nesse caso.

## Implicações para a tarefa L-11

1. **Leitor:** portar `spikes/s-r5-2/src/perfdata.rs` para `crates/warden-perf/src/hsperf/` (parser puro, sem `unsafe`, buffer ruim vira erro) e `src/win.rs` para a parte Windows (`OpenFileMappingW(FILE_MAP_READ)` + `MapViewOfFile` + `VirtualQuery`, com `SAFETY`), mantendo a leitura do arquivo `<pid>` (compartilhamento `READ|WRITE|DELETE`) como alternativa e o Linux por arquivo em `/tmp/hsperfdata_*/<pid>`. Cada amostra copia a vista inteira (64 KB, < 1 ms).
2. **Localização:** procurar `<TEMP do jogo>\hsperfdata_*\<pid>` e montar o nome `hsperfdata_<sufixo da pasta>_<pid>`; nunca montar o nome a partir do usuário. "TEMP do jogo" = o `TMP`/`TEMP` do ambiente que o Warden passa ao processo. Com mais de uma pasta candidata, tentar da mais nova para a mais velha e aceitar a primeira cujo mapeamento abra.
3. **Tentativas e vida útil:** começar a tentar logo depois de abrir o processo (fica legível em ~50–70 ms) e repetir a cada amostra; só aceitar o buffer com `accessible = 1`. Reler a lista de contadores quando `num_entries` ou `mod_time_stamp` mudar. **Fechar o mapeamento assim que o processo sair** (senão um jogo novo com o mesmo pid fica sem leitura).
4. **Valores:** heap usada = soma de `sun.gc.generation.*.space.*.used`; máximo = `-Xmx` do perfil, com reserva pela regra da §5.2 (`Copy:MSC`/`ParScav:MSC` somam as gerações, os outros usam a maior); coletas e tempo = soma de `sun.gc.collector.*.invocations` e `.time` ÷ `sun.os.hrt.frequency`, aceitando índices faltando. Na interface, ZGC e Shenandoah contam pausas ("Pausas de coleta"), não ciclos.
5. **"Heap depois da coleta"** para o `W_LOW_HEAP`: guardar a heap usada da primeira amostra depois de cada aumento de `invocations` (esse valor é confiável em todos os coletores, inclusive no Java 25 com Parallel/Serial, §5.3). No Java 25 com Parallel ou Serial, a "Memória do jogo" ao vivo fica velha entre coletas: mostrar o valor da última coleta com a legenda "na última coleta", ou fazer o perfil padrão do teste usar G1 (decisão da L-04).
6. **Sem leitura:** amostra sem heap e motivo visível. Se os argumentos do teste têm `-XX:+PerfDisableSharedMem` ou `-XX:-UsePerfData`, dizer qual e oferecer removê-lo em Ajustes do teste; se não, "O Java deste teste não publica a memória do jogo" (OpenJ9, nome de usuário ou pasta temporária com letras fora da code page do Windows). Não há como distinguir esses casos lendo de fora.
7. **Usuário fora da code page (P2, opcional):** detectar quando `USERNAME` ou o TEMP não cabem na code page ANSI (`WideCharToMultiByte` com `WC_NO_BEST_FIT_CHARS` e `lpUsedDefaultChar`) e, só então, passar ao jogo `TMP`/`TEMP` em caminho curto 8.3 e `USERNAME` em ASCII, com `-Duser.name=<nome real>` (essa última parte não foi testada). Acento comum de português funciona sem nada disso.
8. **Testes:** copiar `spikes/s-r5-2/fixtures/` (8 `hsperfdata` + `jstat`) para `crates/warden-perf/tests/fixtures/hsperf/` e portar o teste `fixtures_match_jstat`; acrescentar testes de buffer truncado/corrompido e de localização com pastas `hsperfdata_João` e `hsperfdata_Lukasz` criadas numa pasta temporária. O teste de integração da CI Windows (CA-T13-10) deve baixar os JREs 8/17/21/25 e um JDK (como aqui), rodar um programa que aloca e **para**, e comparar com o `jstat` nesse momento estável; em movimento, comparar com a leitura logo depois do `jstat`.
9. **Privacidade:** o `hsperfdata` traz caminhos, PATH e os argumentos do jogo (`sun.rt.javaCommand`); o Warden só extrai números dele, nunca grava nem envia o buffer cru (nem para a IA), e fixtures novas passam pelo `anonymize` do spike.
