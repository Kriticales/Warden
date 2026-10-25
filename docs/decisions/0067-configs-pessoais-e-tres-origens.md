# ADR-0067 — Configs pessoais só neste computador e a seção Configs com três origens

- **Status:** aceita · **Data:** 2026-10-25 · **Origem:** decisão do dono (esboço `design/bug-01/configs.html`, opção 1 "Três colunas" com o seletor "Comparar" da opção 3; tarefa T3-CONFIGS) · **Complementa:** [ADR-0013](0013-editor-de-configs.md) e [ADR-0009](0009-exportacao-nativa-e-separacao.md) · **Muda trechos de:** SPEC T12 e T15

## Contexto

O dono abriu a seção Configs e só viu o `crash_assistant`. Os motivos:

- A seção lia só a pasta do pack (`list_files` em `crates/warden-project/src/configs.rs`).
- As configs dos mods nascem na instância de teste quando o jogo abre. O "O que mudou durante o teste" (T15) as traz desmarcadas, no grupo de arquivos novos.
- Só a config do Crash Assistant é escrita no pack ao criar o pack (`initial_mods.rs`).

Além disso, o `options.txt` (teclas, distância de renderização, idioma, volume) é preferência da pessoa, não do pack. A captura do T15 o oferecia por chave, pré-marcado, e ele ia parar no pack e nos jogadores. O dono decidiu (P7) que essas preferências ficam só no computador dele.

O esboço aprovado também contraria trechos antigos da SPEC: a origem escolhida por `<select>` "Mostrando: arquivos do pack / instância", os filtros como caixas de seleção e o `options.txt` por chave na captura. Por isso a SPEC foi atualizada junto com esta ADR.

Dois defeitos visuais do print antigo ficaram explicados: a borda branca grossa era o foco do botão da árvore (`.tree__item:focus-visible` em `design/system/components.css`) e a seta cortada vinha do `nowrap` numa coluna de 260 px (`configs.css`).

## Decisão

### 1. A seção junta as duas origens

- O backend junta a pasta do pack e a instância de teste (`crates/warden-project/src/config_places.rs`, comando `config_overview`). A diferença teste × pack vem por chave (`config_compare`, que usa `warden_configs::compare`; formatos sem leitura por chave contam linhas).
- A árvore tem três grupos: **No pack** (vai para os jogadores), **Só no teste** (o jogo criou na instância de teste e nunca foi para o pack) e **Só neste computador** (preferências pessoais, com cadeado).
- O seletor **Do pack | Do teste local | Comparar** escolhe o que ver. "Do pack" mostra só o que está no pack, com o aviso "N configs estão só no teste" e **Ver no teste local**. Antes do primeiro teste não há instância e só "Do pack" tem arquivos.
- Layout em três colunas: árvore (uma busca só, por nome e conteúdo; filtros "Mudou do padrão" e "Diferente do pack" como botões, sem caixa de seleção), editor (Formulário | Texto, com faixa colorida dizendo para onde o arquivo vai) e painel **Detalhes** (recolhível).
- Quando o teste difere do pack, o editor mostra "no pack: X" e **Trazer para o pack** por chave; no Texto, as linhas diferentes do pack ficam marcadas. **Comparar** põe teste e pack lado a lado, com → (trazer o valor do teste para o pack) e ← (voltar ao do pack) por chave.
- Comandos novos: `config_overview`, `config_compare`, `config_bring` (arquivo inteiro ou chaves, pela `PackTransaction` com `packwiz refresh`), `config_revert` (grava na instância), `config_mark_personal`, `config_unmark_personal`, `config_keep_personal_out`.
- A árvore nova tem foco fino por dentro do item e colunas que cortam o nome com reticências.

### 2. Configs pessoais

- **Lista padrão** (`DEFAULT_PERSONAL` em `crates/warden-instance/src/personal.rs`): `options.txt`, `optionsof.txt`, `optionsshaders.txt`, `config/xaerominimap.txt` e `config/xaeroworldmap.txt`.
- **Onde ficam:** nos dados do Warden, por pack, em `<dados>/personal/<pack-id>/` (`AppPaths::personal_dir`): `personal.json` com as escolhas e `files/<caminho>` com as cópias.
- **Quando voltam:** em todo teste. O gancho `personal` do Testar, em `before_launch` (antes da linha de base da captura), põe as cópias na instância; quando o jogo fecha, `after_exit` guarda de novo. Editar um deles no Warden pela origem "teste" também atualiza a cópia.
- **Nunca vão para o pack:** a materialização do pack na instância não escreve, não apaga e não pede revisão desses caminhos (`MaterializeOptions::personal`) e o manifesto da instância os esquece. O "O que mudou durante o teste" não os mostra como candidatos. O teste como servidor ignora as configs pessoais.
- **Marcar como pessoal** acrescenta um arquivo à lista. Se estava no pack, ele sai do pack depois de a pessoa confirmar; a cópia guardada é a da instância, ou a do pack se a instância não tem. **Deixar de ser pessoal…** tira da lista (inclusive um padrão), apaga a cópia guardada, e o arquivo continua na instância como "Só no teste"; para ir aos jogadores, **Trazer para o pack**.
- **Migração:** quando o pack aberto ainda leva uma config pessoal (por exemplo o `options.txt` que a captura antiga gravava no pack), aparece ao abrir o pack o aviso "O options.txt agora fica só neste computador" com três escolhas:
  - **Tirar do pack e guardar aqui:** guarda a cópia (a da instância de teste, se houver; senão a do pack) e tira o arquivo do pack numa transação com `packwiz refresh`;
  - **Manter no pack:** o arquivo deixa de ser pessoal;
  - **Agora não:** pergunta de novo na próxima abertura.
  Nada some sem o dono ver.

## Alternativas consideradas

- **Manter o `options.txt` por chave na captura (como na SPEC antiga):** as preferências do dono (teclas, volume) iam para todos os jogadores; bastava um clique errado para vazar.
- **Só avisar sem guardar as cópias:** o `options.txt` da instância seria sobrescrito ou apagado pela sincronização com o pack, e a pessoa perderia as preferências a cada teste.
- **Tirar o arquivo do pack sem perguntar na migração:** contraria "nada muda sem o dono ver".
- **Origem por `<select>` e filtros em caixas de seleção (esboço antigo):** esconde a diferença entre teste e pack; o dono preferiu ver as três origens e comparar lado a lado.
- **Opção 2 ou só a 3 do esboço:** a opção 1 mostra tudo ao mesmo tempo; o "Comparar" da 3 foi acrescentado a ela.

## Consequências

- O `options.txt` deixa de ser candidato da captura do T15; o CA-T15-01 passa a usar uma config do pack como exemplo.
- A opção "Não substituir se o jogador já tiver" (`preserve = true`) do `options.txt` deixa de ser o caminho normal. A exportação e o pacote do servidor já não levavam `options.txt`.
- A importação de uma instância do Prism não mudou nesta tarefa: se ela levar o `options.txt` para o pack, o aviso de migração pergunta na próxima abertura do pack.
- Packs criados antes desta decisão que já levam um `options.txt` no pack disparam o aviso de migração na abertura.
