# ADR-0043 — Analisar o travamento de um jogador

- **Status:** aceita · **Data:** 2026-10-02 · **Origem:** decisão do dono (tarefa D5, decisão D29)

## Contexto

Quem distribui um pack recebe travamentos dos jogadores, quase sempre como link de um site de colagem. O Crash Assistant sempre envia o log ao mclo.gs, e o Prism Launcher usa o mclo.gs por padrão. O mclo.gs anonimiza IPs e caminhos, mas não o nome do jogador, e mascara versões de quatro partes como `**.**.**.**` (o Crash Assistant troca os pontos por `∙`). O diagnóstico da v1 só lê os logs dos testes do próprio usuário, e o jogador pode estar numa versão antiga do pack.

## Decisão

- **Serviços aceitos**, com o texto cru lido pelo Warden: mclo.gs por `api.mclo.gs/1/raw/<id>`, inclusive nos formatos gnomebot.dev, p.kdan.dev e paste.kostromdan.dev, que são visualizadores do mclo.gs; pastebin (`/raw/`), paste.ee (`/r/`), gist cru, 0x0.st, hst.sh e paste.gg. O hastebin.com exige token e fica de fora.
- **Download pelo Rust**, só HTTPS, só texto, até 10 MB.
- **Redação própria sempre**, mesmo quando o serviço já anonimizou, mais o nome do jogador e o da instância. O Warden guarda **só a cópia redigida**, nos dados do Warden, por pack, nunca na pasta do pack. Versões mascaradas contam como "versão escondida", não como diferença.
- **Versão do pack**, em ordem de confiança: `packwiz.json` do packwiz-installer (o campo `packFileHash` é o sha256 do `pack.toml` recebido, verificado no código do packwiz-installer); nome dos jars (Forge, NeoForge e Prism); modid e versão (o Fabric não informa o jar).
- **Limiares:** versão exata; 90% ou mais dos mods conferindo = versão próxima; menos de 50% = pack desconhecido, e o Warden não força uma versão.
- **IA:** a mesma de sempre, com o mesmo consentimento por conversa ([ADR-0030](0030-ia-com-ferramentas-e-conversas.md)).
- **Lugar:** Problemas → Travamentos, com a origem "Jogador" ([ADR-0045](0045-funcoes-1-1-na-estrutura.md)).
- **Código:** `warden-diagnostics::{logsource, modlist}` (funções puras) e a orquestração em `warden-app::player_reports`.

## Alternativas consideradas

- Aceitar qualquer link: baixaria páginas HTML e conteúdo de qualquer site; a lista fechada sabe onde está o texto cru de cada serviço.
- Confiar na anonimização do mclo.gs: deixa o nome do jogador passar.
- Abrir o link no navegador e pedir para colar o texto: mais passos para o usuário, sem ganho de segurança.
- Identificar a versão só pelo nome dos jars: falha no Fabric e quando o jogador renomeia arquivos.

## Consequências

- Usa os ganchos da v1: análise pós-crash que aceita qualquer texto e redação com regras extras, e o campo `origin` no registro de travamento ([ADR-0039](0039-warden-1-1-profissional.md), gancho 5).
- **Convivência com a [ADR-0033](0033-mods-iniciais-e-kits.md):** o Warden não lê os arquivos próprios do Crash Assistant (como o `modlist.txt`, que traria hashes por jar). O link que o Crash Assistant mostra ao jogador é o log do próprio jogo guardado no mclo.gs, e esse o Warden lê normalmente.
- SPEC T30.
