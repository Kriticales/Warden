# Matriz L-05

Os recortes em `matriz/`, `falhas/`, `mecanismos/` e `busca/` vieram da branch
`Kriticales/spike-s-r5-3-marcadores`, diretório `spikes/s-r5-3/fixtures/`.
Foram capturados no Windows em 04/10/2026 e redigidos no spike; a origem e os
limites dos recortes estão no `SOURCE.md` original, preservado nesta pasta.
`smoke_logs.rs` passa cada recorte pelo `warden_launcher::log::LogParser` e
confere as 20 combinações, o atlas definitivo de 1.7.10, falhas reais e a
política de log4j por faixa. A política de log4j é um teste da configuração
selecionada e da linha de comando; não injeta uma expressão JNDI no jogo.

`smoke_game.rs` baixa o Java e o jogo pelos componentes existentes da L-02,
cria o pack mínimo com `pack.py` e materializa o mod pela L-03 antes de abrir
cada combinação. `mods.json` fixa URL, versão e SHA-1 de cada JAR no CDN do
Modrinth; `pack.py` confere o hash e o arquivo de Mixin dentro do JAR
materializado. Forge 1.7.10 e 1.12.2 usam AI Improvements, sem Mixin; as
versões modernas usam FerriteCore ou Mouse Tweaks, com Mixin. Vanilla 26.3
tem o pack sem mod. O teste prepara um servidor descartável com `prepare.py`.
Em 1.20+ o servidor vanilla gera um mundo sem mods, copiado para `saves/`, e o
cliente entra via Quick Play. Antes de 1.20 o servidor local do loader recebe
o cliente via `--server/--port`. O teste exige os marcadores de pronto e de
entrada, confere `--quickPlayPath` nas versões modernas, grava os logs e para
os processos. `WARDEN_LAUNCHER_DADOS` permite reaproveitar downloads dentro de
uma pasta de testes isolada. Os diretórios do usuário e o cofre real não são
usados. Os tempos de pronto e mundo começam na abertura do cliente, mesmo nas
faixas com servidor local.

O workflow `.github/workflows/smoke-game.yml` é **somente manual**. Quatro
runners Linux executam cinco combinações cada, em série, com Xvfb e Mesa.
O artefato de cada runner inclui o log do teste, os logs de cliente/servidor,
`options.txt` e `quickplay.json` quando existir. O workflow não é chamado pela
CI de push nem pela CI de pull request.

Para as três execuções locais pedidas no Windows, defina `CARGO_BUILD_JOBS=2`,
`WARDEN_SMOKE_GAME=1`, `WARDEN_LAUNCHER_DADOS` numa pasta temporária isolada e
execute, **uma por vez**:

```powershell
$env:WARDEN_SMOKE_ONLY = 'forge-1.7.10'
cargo test -p warden-launcher --test smoke_game -- --ignored --nocapture
$env:WARDEN_SMOKE_ONLY = 'forge-1.20.1'
cargo test -p warden-launcher --test smoke_game -- --ignored --nocapture
$env:WARDEN_SMOKE_ONLY = 'neoforge-26.2'
cargo test -p warden-launcher --test smoke_game -- --ignored --nocapture
```

As três execuções locais com mod estão em `relatorio-windows.md`. A matriz
completa no Linux depende do disparo manual do workflow após a integração.
