# ADR-0033 — Mods iniciais (spark e Crash Assistant), ferramentas do jogador e kits de desempenho

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (tarefa D4, pesquisas R5A §2 e R5B §5.3)

## Contexto

O dono quer que todo pack novo venha com um profiler (spark) e um mod de crash report para o jogador. O R5A §2 mostrou que o **Crash Assistant** é o único mod de crash que cobre de 1.7.10 à versão mais nova em todos os loaders, mas tem licença própria (KostromDan's Modded Minecraft License 1.1.3) com **cláusula de não concorrência** (§3) e envia metadados ao autor por padrão desde a 1.11.12. A R1 §2.3.5 e a R5B §5.3 listaram kits de desempenho por loader e versão.

## Decisão

- **Mods iniciais:** o assistente Criar pack ganha a etapa "Mods iniciais", com **spark** e **Crash Assistant** já marcados (desmarcáveis) e um **kit de desempenho** opcional, desmarcado. Para 1.7.10 e 1.12.2, o spark vem da CurseForge numa versão antiga, com o aviso "sem atualizações".
- **Crash Assistant, regras de convivência:** o Warden **só inclui o mod no pack por referência** (o packwiz baixa da página oficial, como a licença pede) e escreve uma config inicial documentada pelo autor que **desliga o envio de dados ao autor e o encurtador de links** e mantém a verificação de pirataria desligada. O Warden **nunca usa, lê ou porta o código** do Crash Assistant, nem lê os arquivos próprios que ele gera; o diagnóstico do Warden continua nos arquivos padrão do jogo e no codex-minecraft (MIT). Isto não é parecer jurídico.
- **Ferramentas do jogador:** spark e Crash Assistant ficam marcados no pack como ferramentas do jogador (`.warden/project.toml`). O Crash Assistant fica **fora dos testes normais** (a janela dele atrapalharia o resultado do Warden) e **dentro do "Testar como o jogador recebe"**. Na busca do culpado, os dois ficam desligados, a menos que sejam suspeitos.
- **Kits de desempenho** e a lista de mods iniciais são **dados versionados** no repositório do Warden (IDs do Modrinth, não slugs; itens só da CurseForge pelo ID), validados no CI contra a API do Modrinth para cada versão da faixa. Aplicar um kit passa sempre pelo diálogo de dependências, com caixas marcadas: nada entra em silêncio. Prioridade P1 (antes P2).
- **Higiene:** arquivos gerados pelo spark (`config/spark/*.sparkprofile`, `*.sparkheap`, `config/spark/tmp*`) entram no `.packwizignore` padrão e nos "sempre ignorados" da captura (decisão D21).

## Alternativas consideradas

- Nenhum mod de crash, só MixinTrace: leve e MIT, mas o jogador fica sem ajuda (R5A, plano B).
- Not Enough Crashes: livre, mas não cobre 1.7.10 nem 1.12.2 e o "voltar ao menu" deixa o jogo em estado quebrado.
- Instalar o kit sem perguntar: contraria "sem surpresas" (SPEC §1).

## Consequências

- R2 §6.4 dizia só "All Rights Reserved" para o Crash Assistant; a nota de correção remete a esta ADR.
- Tarefa P1-18 no ROADMAP (dados, CI, etapa do assistente, config inicial, ferramentas do jogador na materialização).
