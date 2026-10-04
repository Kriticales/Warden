# Spike S-R5-1 — código descartável

Relatório: `docs/spikes/S-R5-1-intermed.md`. Nada daqui entra no app; a D-10 reimplementa.

## O que tem aqui

- `harness/`: crate Rust com três subcomandos.
  - `proto <fabric|forge|neoforge> <pasta-mods> <saida.json> [--threads N] [--cache PASTA] [--config-dir PASTA] [--known ARQ]`:
    o protótipo do raio-x da R5A (§5.3 a §5.5) refeito no Windows (o original ficou no WSL), mais o modo
    "refinado" do spike (opções desligadas, seletor de variável/argumento, plugin que cita o outro mod).
  - `intermed <pasta-mods> <saida.json> [--cache PASTA]`: chama o `intermed-mixin-intel` como biblioteca
    (commit `7d90f2e`, dependência git fixada no `Cargo.toml`).
  - `dump <loader> <jar> [filtro]`: lista as alterações extraídas de um jar (para conferir casos à mão).
- `tools/baixar_pack.py`: baixa um `.mrpack` e os jars de `mods/` pela API do Modrinth (sem chave), confere o sha1 e grava `origem.json`.
- `tools/textos_da_classe.py`: lista os textos de classes de um jar (para ver se um plugin cita outro mod).
- `tools/comparar.py`: resume as saídas para o relatório.
- `tools/extrair_fixtures.py`: grava as fixtures de `fixtures/`.
- `fixtures/`: configs de mixin, descritores e configs de pack pequenos (origem em `fixtures/ORIGEM.json`),
  lista dos jars de cada pack com URL e sha1 (`jars-*.json`), resultado do protótipo por pack
  (`resultado-*.json`) e os veredictos julgados à mão (`veredictos.json`). Nenhum jar é versionado.

## Reproduzir (PowerShell 7)

```powershell
$sp = 'C:\caminho\curto\s-r5-1'   # pasta de trabalho fora do git
python spikes\s-r5-1\tools\baixar_pack.py better-mc-fabric-bmc2 1.20.1 fabric "$sp\packs\fabric-1.20.1"
python spikes\s-r5-1\tools\baixar_pack.py better-mc-forge-bmc4 1.20.1 forge "$sp\packs\forge-1.20.1"
python spikes\s-r5-1\tools\baixar_pack.py better-mc-neoforge-bmc5 1.21.1 neoforge "$sp\packs\neoforge-1.21.1"
# config/ do pack: extrair overrides/config/* do .mrpack para "$sp\packs\<pack>\config"

$env:CARGO_TARGET_DIR = 'C:\wt\s-r5-1h'   # caminho curto (QUALITY §13.3)
cargo build --release --manifest-path spikes\s-r5-1\harness\Cargo.toml
$h = 'C:\wt\s-r5-1h\release\s-r5-1-harness.exe'
& $h proto fabric "$sp\packs\fabric-1.20.1\mods" "$sp\out\proto-fabric-1.20.1.json" --threads 8 --config-dir "$sp\packs\fabric-1.20.1\config"
& $h intermed "$sp\packs\fabric-1.20.1\mods" "$sp\out\intermed-fabric-1.20.1.json"
```

O `intermed doctor --mixin-risk --json` (achados que o intermed mostra ao usuário) foi rodado com o binário
compilado do clone do intermed no mesmo commit; comandos no relatório, §2.
