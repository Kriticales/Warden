# Testar com o jogo real (CA-T13-01 e CA-T13-05)

Roteiro da L-04 para o botão **Testar** com o Minecraft de verdade. O teste automatizado
`ca_t13_01_jogo_real_entra_no_mundo_pelo_testar` (em `tests.rs`, ignorado por padrão) passa
por todas as etapas do Testar e entra num mundo; o jogo simulado cobre o resto na CI
(`ca_t13_02`…`ca_t13_06`).

## O que o teste faz

1. Cria o pack mínimo da matriz da L-05 (`crates/warden-launcher/tests/matrix/pack.py`):
   Fabric 1.20.1 com FerriteCore (Mixin) ou Forge 1.12.2 com AI Improvements.
2. Clica em Testar (`run_test`): o Java vem da `warden-java`, o jogo e o loader do motor, e o
   mod é baixado do Modrinth pela cópia do pack para a instância de teste.
3. Espera os marcadores da matriz (`tests/matrix/markers.rs`, S-R5-3 §3):
   - **Pronto** (menu principal): atlas de blocos e som iniciado; no Forge 1.12.2, também
     `Forge Mod Loader has successfully loaded`;
   - **Mundo**: `logged in with entity id`;
   - qualquer sinal de falha da matriz (crash, mixin, dependência…) reprova na hora.
4. Fica 5 s no mundo, clica em **Parar jogo** e confere a sessão "Encerrado por você", o mod
   na instância (`pack.py verify`), o `quickplay.json` (1.20+) e o
   `onboardAccessibility:false` no `options.txt`.
5. Imprime os tempos de Pronto e Mundo desde a abertura do jogo (comparáveis aos da L-05) e
   desde o clique em Testar.

O mundo é gerado por um servidor descartável, como na L-05 (`prepare.py`): em 1.20+ ele é
copiado para `saves/wardentest` e o jogo entra por Quick Play; no 1.12.2 o jogo entra no
servidor local do Forge com `--server/--port`. A entrada direta no mundo usa um ajuste da
abertura que só existe nos testes (`LaunchTweak`), porque no app ela é do perfil do teste
(L-08). Com `WARDEN_TEST_REAL_OFFLINE=1` o teste confere primeiro que a internet está
bloqueada e não prepara o servidor de novo.

## Linux (com Xvfb e Mesa)

Precisa de Python 3, do `xvfb-run` (pacote `xvfb`) e, para o Forge 1.12.2 (LWJGL 2), do
`xrandr` (pacote `x11-xserver-utils`; sem ele o jogo trava em `LinuxDisplay.getAvailableDisplayModes`).
A pasta de `WARDEN_TEST_REAL_ROOT` guarda os downloads entre as rodadas.

```bash
# 1. Com internet: baixa tudo e entra no mundo.
env -u JAVA_TOOL_OPTIONS WARDEN_TEST_REAL_GAME=fabric-1.20.1 WARDEN_TEST_REAL_ROOT=/tmp/real \
  xvfb-run -a cargo test -p warden-app --lib jogo_real -- --ignored --nocapture

# 2. Sem internet (CA-T13-05): a mesma pasta, com toda conexão de saída recusada.
env -u JAVA_TOOL_OPTIONS WARDEN_TEST_REAL_GAME=fabric-1.20.1 WARDEN_TEST_REAL_ROOT=/tmp/real \
  WARDEN_TEST_REAL_OFFLINE=1 \
  HTTPS_PROXY=http://127.0.0.1:9 HTTP_PROXY=http://127.0.0.1:9 \
  https_proxy=http://127.0.0.1:9 http_proxy=http://127.0.0.1:9 NO_PROXY= no_proxy= \
  xvfb-run -a cargo test -p warden-app --lib jogo_real -- --ignored --nocapture
```

Repita com `WARDEN_TEST_REAL_GAME=forge-1.12.2`. O proxy morto bloqueia o Warden e o motor
(os dois usam o `reqwest`, que respeita essas variáveis); o jogo em si não precisa de rede.
Use `cargo test`, não o nextest, que encerra testes de mais de 3 minutos.

A rodada sem internet também esconde o jar do jogo (`versions/<id>/<id>.jar`), testa de novo e
confere que o Testar para antes de abrir o jogo com `app.TEST_GAME_FILES_MISSING` ("Faltam 1
arquivos do Minecraft para abrir o teste e não foi possível baixá-los: <arquivo>…"); o jar volta
no fim.

Atrás de um proxy que troca o certificado (como o contêiner da nuvem), o `env -u
JAVA_TOOL_OPTIONS` deixa o instalador do servidor Forge (Java 8, chamado pelo `prepare.py`) sem
o certificado do proxy. Prepare o servidor uma vez com as opções do proxy e rode o teste depois:

```bash
python3 crates/warden-launcher/tests/matrix/prepare.py legacy 1.12.2 forge 14.23.5.2860 \
  /tmp/real/servers/forge-1.12.2 /tmp/real/data/shared/runtimes/<java 8>/bin/java 25565
```

## Windows (roteiro manual)

No Windows o teste abre a janela do jogo e ninguém precisa clicar nela (S-R5-3 §9).

1. PowerShell na pasta do projeto, com internet:

   ```powershell
   $env:WARDEN_TEST_REAL_GAME = 'fabric-1.20.1'
   $env:WARDEN_TEST_REAL_ROOT = "$env:TEMP\warden-real"
   cargo test -p warden-app --lib jogo_real -- --ignored --nocapture
   ```

2. Desligue a internet (modo avião ou cabo/Wi-Fi desligado) e rode de novo com
   `$env:WARDEN_TEST_REAL_OFFLINE = '1'`. O teste recusa rodar se a internet ainda responder.
3. Repita os dois passos com `forge-1.12.2`. Apague a pasta de `WARDEN_TEST_REAL_ROOT` no fim.
4. Pelo app (o que o dono vê): num pack Fabric 1.20.1, clique em **Testar** e confira o botão
   "Testando… ver progresso", depois "● Jogo aberto: ver teste"; no jogo, crie um mundo em
   **Um jogador → Criar novo mundo**; feche o jogo e confira o resultado. Desligue a internet,
   clique em **Testar** de novo: o jogo abre. Apague um jar da pasta `mods` da instância
   (menu ▾ → Instância de teste → Abrir pasta) e teste ainda sem internet: a mensagem diz
   "Não foi possível copiar 1 itens do pack para o teste: <nome do mod>".

Anote no relatório da tarefa, para cada combinação, os tempos impressos (Pronto e Mundo) e se
a rodada sem internet passou.

## Resultados

Tempos desde a abertura do jogo (como na L-05) e, entre parênteses, desde o clique em Testar
(inclui baixar Java, jogo, loader e mod na primeira rodada).

| Data | Sistema | Combinação | Internet | Pronto | Mundo | Arquivo faltando |
|---|---|---|---|---|---|---|
| 10/10/2026 | Linux (contêiner, Xvfb + Mesa llvmpipe, 4 CPUs) | Fabric 1.20.1 + FerriteCore | com | 29,3 s (72 s) | 49,4 s (92 s) | — |
| 10/10/2026 | Linux (contêiner, Xvfb + Mesa llvmpipe, 4 CPUs) | Fabric 1.20.1 + FerriteCore | sem | 12,5 s (13 s) | 29,4 s (30 s) | `fabric-1.20.1-0.19.5.jar` nomeado |
| 10/10/2026 | Linux (contêiner, Xvfb + Mesa llvmpipe, 4 CPUs) | Forge 1.12.2 + AI Improvements | com | 18,0 s (19 s) | 20,6 s (22 s) | — |
| 10/10/2026 | Linux (contêiner, Xvfb + Mesa llvmpipe, 4 CPUs) | Forge 1.12.2 + AI Improvements | sem | 18,7 s (19 s) | 21,4 s (22 s) | `forge-1.12.2-14.23.5.2860.jar` nomeado |

O roteiro manual no Windows (passos acima) ainda não foi executado: fica para o orquestrador
ou o dono, com a versão de teste.
