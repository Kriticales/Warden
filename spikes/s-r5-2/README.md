# Spike S-R5-2: memória da JVM pelo `hsperfdata` no Windows

Código descartável. O relatório está em `docs/spikes/S-R5-2-memoria-da-jvm.md`.

| Caminho | O que é |
|---|---|
| `src/perfdata.rs` | Leitor do formato `hsperfdata` (cabeçalho, entradas, textos), sem depender de plataforma; testes com as fixtures. |
| `src/win.rs` | Windows: acha `%TEMP%\hsperfdata_*\<pid>`, abre o *file mapping* nomeado (`OpenFileMappingW` + `MapViewOfFile`, só leitura) ou lê o arquivo de apoio. |
| `src/main.rs` | CLI `hsperf-probe` (`read`, `counters`, `dump`, `compare`, `parse`, `anonymize`). |
| `java/Alloc.java` | Programa que aloca memória sob comando pela entrada padrão. |
| `run-case.ps1` | Roda um caso: abre a JVM, passa pelas etapas e compara o leitor com o `jstat` em cada uma. |
| `summarize.mjs` | Junta `results/*.jsonl` em `results/tabela.md`. |
| `results/` | Saídas reais das execuções (nome de usuário trocado por `<usuário>`). |
| `fixtures/` | Arquivos `hsperfdata` gravados por versão de Java, com o `jstat` ao lado (veja o README de lá). |

## Como reproduzir (PowerShell)

Os Javas e o JDK ficaram em `C:\wt\s-r5-2\j\{8,17,21,25,jdk,mojang8,mojang25}` durante o spike e foram apagados no fim. Para refazer, baixe os zips do Temurin (API do Adoptium, `image_type=jre` e um `jdk`) e confira o SHA-256.

```powershell
$env:CARGO_TARGET_DIR = $null   # a pasta target deste spike cabe no limite de 260 caracteres
cargo build --release
cargo test --release
& C:\wt\s-r5-2\j\jdk\bin\javac.exe -Xlint:-options --release 8 -d C:\wt\s-r5-2\cls java\Alloc.java
./run-case.ps1 -Java C:\wt\s-r5-2\j\21\bin\java.exe -Label j21-g1 -JvmArgs '-Xmx1024m','-XX:+UseG1GC' |
    Out-File -Encoding utf8 results\j21-g1.jsonl
./run-case.ps1 -Java C:\wt\s-r5-2\j\21\bin\java.exe -Label j21-joao -Env @{ USERNAME = 'João' }
node summarize.mjs > results\tabela.md
```
