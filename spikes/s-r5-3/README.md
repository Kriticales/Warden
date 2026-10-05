# Spike S-R5-3 — marcadores e mecanismos da busca do culpado (código descartável)

Relatório: [`docs/spikes/S-R5-3-marcadores-da-busca.md`](../../docs/spikes/S-R5-3-marcadores-da-busca.md).
Nada aqui é código de produção. Nenhum Java, jar, mundo ou instância fica no repositório: tudo vai
para `$WARDEN_SPIKE_DATA` (padrão `C:\wt\s-r5-3\data`).

## Conteúdo

| Caminho | O que é |
|---|---|
| `bisect-proto/` | Protótipo em Rust. `engine.rs` é o adaptador do portablemc 5.0.5 do S1 (com `Quilt`); `supervisor.rs` é o supervisor do S1 com Job Object do Windows e entrada padrão para o servidor; `markers.rs` é o catálogo de marcadores por faixa; `main.rs` tem os comandos `run` (cliente, com Quick Play, `--server/--port`, propriedades da JVM, `options.txt` e servidor local) e `server` (só o servidor dedicado). |
| `scripts/prep_server.py` | Prepara um servidor dedicado descartável (vanilla, Fabric, Forge ou NeoForge) com `online-mode=false`. |
| `scripts/modrinth.py` | Baixa uma versão do Modrinth ou instala um `.mrpack` (lado cliente), sem chave. |
| `scripts/matrix.py` | Matriz de versões: cliente pronto e "entrou no mundo" por combinação. |
| `scripts/build_testmod.py` + `testmod/` | Mod de teste `wardenfalhas` (Fabric, NeoForge e Forge) que provoca falhas controladas: mixin quebrado, exceção ao iniciar, exceção ao entrar no mundo, travamento e conflito de 2 mods. |
| `scripts/scenarios.py` | Cenários de falha reais (dependência faltando, versão errada, outro loader, duplicado, mixin, crash, memória, Java errado, travado). |
| `scripts/busca_culpado.py` | Busca do culpado de ponta a ponta no pack de ~150 mods (grafo, ordem topológica, prefixos, par, confirmação). |
| `scripts/golden.py` | Recorta golden logs curtos de uma captura, com marcadores no lugar de dados pessoais. |
| `scripts/entrada.py` | Detector de entrada humana (teclado/mouse) durante cada rodada. |
| `scripts/make_fixtures.py` | Gera os golden logs de `fixtures/` a partir das capturas. |
| `fixtures/` | Golden logs para os testes da D-12 e da L-05 (ver o README de lá). |
| `resultados/` | Resultados brutos (JSON por rodada) da matriz, dos cenários e da busca, com `<DADOS>` no lugar da pasta de dados. |

## Como reproduzir (PowerShell 7, Windows)

```powershell
$env:CARGO_TARGET_DIR = 'C:\wt\s-r5-3\target'
$env:ALSOFT_DRIVERS = 'null'
cargo build --release --manifest-path spikes/s-r5-3/bisect-proto/Cargo.toml
$b = 'C:\wt\s-r5-3\target\release\bisect-proto.exe'

# Cliente pronto (menu)
& $b run 1.12.2 forge:1.12.2-14.23.5.2860 --hold 3
# Quick Play (1.20+), com o mundo já em saves\wardentest
& $b run 1.21.1 neoforge:21.1.252 --goal world --world wardentest --qp-path quickplay.json --option onboardAccessibility:false
# Antes de 1.20: servidor local + --server/--port
python spikes/s-r5-3/scripts/prep_server.py forge 1.12.2-14.23.5.2860 C:\wt\s-r5-3\data\servers\forge-1.12.2 --java C:\wt\s-r5-3\data\shared\runtimes\jre-legacy\bin\java.exe
& $b run 1.12.2 forge:1.12.2-14.23.5.2860 --goal world --server-dir C:\wt\s-r5-3\data\servers\forge-1.12.2 --legacy-server 127.0.0.1:25599
# Só o servidor, com a resposta automática do FML 1.12.2
& $b server C:\wt\s-r5-3\data\servers\forge-1.12.2 --java C:\wt\s-r5-3\data\shared\runtimes\jre-legacy\bin\java.exe --prop fml.queryResult=confirm

python spikes/s-r5-3/scripts/matrix.py                 # matriz inteira
python spikes/s-r5-3/scripts/build_testmod.py C:\wt\s-r5-3\data\testmod
python spikes/s-r5-3/scripts/scenarios.py              # cenários de falha
python spikes/s-r5-3/scripts/modrinth.py mrpack create-complete-by-shalz 2.0.0 C:\wt\s-r5-3\data\pack-original
python spikes/s-r5-3/scripts/busca_culpado.py busca C:\wt\s-r5-3\data\pack-culpado
```

Os servidores de teste gravam `eula=true` porque são descartáveis e apagados no fim; no Warden o
aceite é do usuário (R5A §3.4).

## Origem de terceiros

- `portablemc` 5.0.5 — https://github.com/theorzr/portablemc, Apache-2.0 (crates.io).
- `windows-sys` 0.61 — Microsoft, MIT/Apache-2.0 (crates.io).
- Mods e modpack baixados do Modrinth em tempo de execução (não versionados); lista no relatório.
- O mod de teste é compilado contra os jars do Mixin (SpongePowered, MIT) e do FML/FancyModLoader
  já baixados pelo motor; nada disso é versionado.
