# Marco 3: Núcleo completo

Com este marco, o caminho todo funciona: montar o pack, ajustar as configs, testar, trazer para o pack o que você mudou dentro do jogo, salvar versões, exportar e publicar no GitHub para os jogadores. Todas as funções principais do Warden já existem.

## O que chegou

- **Configs:** a seção mostra os arquivos de ajuste do pack numa lista, com um editor de texto colorido. **Salvar** grava só o que você mudou e mantém os comentários do arquivo. O seletor no alto troca entre **Mostrando: arquivos do pack** e **Mostrando: instância de teste**. O Warden avisa sobre alterações não salvas, sobre arquivos que o jogo reescreve e quando outro programa mudou o arquivo enquanto ele estava aberto.
- **Buscar em todas as configs:** campo acima da lista que procura no nome da chave, no comentário e no valor, sem diferenciar maiúsculas nem acentos. Os resultados vêm agrupados por arquivo e abrem o arquivo já na linha certa. Dá para filtrar com **Só deste mod**.
- **O que mudou durante o teste:** no resultado do teste, o Warden mostra o que o jogo mudou (opções do jogo, configs, arquivos novos criados pelos mods, mods colocados à mão na pasta), sem a reformatação que o jogo faz sozinho. Você escolhe o que vai para o pack com **Trazer selecionados para o pack**, ou usa **Descartar tudo** ou **Decidir depois**. Se o mesmo arquivo mudou no pack e no jogo, ele pergunta qual manter. Antes do próximo teste, avisa se ficou algo sem revisar.
- **Salvar versão:** botão no alto do pack, com o número de alterações. O diálogo sugere o número da versão e explica o motivo, monta o resumo automático do que mudou, aceita notas e tem **Marcar como versão final**.
- **Histórico:** as alterações não salvas (com **Descartar** por arquivo), as versões salvas com o estado de cada uma (**Só salva**, **Versão final · não publicada**, **Publicada**), **Ver mudanças**, **Voltar para esta versão** e os **Pontos de segurança…**, cópias automáticas feitas antes de qualquer ação que apaga ou substitui.
- **Exportar:** confere o pack antes (arquivos que não deveriam ir para quem joga, erros em Problemas, alterações não salvas), mostra o que vai no pack, permite **Excluir do pack** um arquivo e gera a pasta packwiz ou o arquivo .zip, conferido com o packwiz no fim.
- **Publicar no GitHub:** a área **Publicação para os jogadores**, no Histórico, cria (ou usa) um repositório no seu GitHub e publica a versão final. Antes, avisa de mods da CurseForge que os jogadores teriam que baixar à mão ou que só existem lá (com **Trocar pelo Modrinth**), de versão não testada e de chaves esquecidas em arquivos. Depois, mostra o **Link do pack** e o passo a passo para os jogadores instalarem no Prism Launcher ou no MultiMC.
- **Mods iniciais e kits de desempenho:** o **Criar pack** sugere o spark (mede o que deixa o jogo lento) e o Crash Assistant (janela clara para o jogador quando o jogo trava, sem envio de dados ao autor e fora dos seus testes normais), mais um **Kit de desempenho** opcional. Os **Kits de desempenho** também aparecem na página **Adicionar ao pack**.

## O que ainda não tem

- Diagnóstico com IA, busca do mod culpado, formulário de configs, scripts, perfis do teste, servidor local, importar modpacks, exportar `.mrpack` e zip da CurseForge, pacote para servidor e instância pronta para o Prism: Marco 4.
- Instalador final, revisão completa e uso diário: Marco 5 (Versão 1).

Esta pré-versão sai do código mais recente, então algumas telas de marcos seguintes podem já aparecer, ainda incompletas. Elas são conferidas nas pré-versões dos próprios marcos: agora o foco é **o caminho completo do pack até os jogadores**.

## O que testar

Leva uns 45 minutos (uns 30 se você pular a publicação no GitHub, passos 12 a 14). Faça na ordem e anote o que der errado.

1. Em **Meus packs**, clique **Criar pack** com Minecraft 1.20.1 e Fabric. Na etapa **Mods iniciais**, deixe o spark e o Crash Assistant marcados e, em **Kit de desempenho**, marque **Adicionar o kit** e abra **Ver o que vem no kit**. O pack tem que abrir já com esses mods em **Mods**. Em **Mods**, clique **Adicionar** e confira que a página **Adicionar ao pack** mostra os **Kits de desempenho**.
2. Clique **Testar**, entre num mundo e, nas opções do jogo, mude a distância de renderização e uma tecla. Feche o jogo pelo menu do próprio jogo.
3. No resultado do teste, em **O que mudou durante o teste**, confira **Opções do jogo (options.txt)** com as duas mudanças e os **Arquivos criados pelos mods** desmarcados. Clique **Ver diferença** em um deles.
4. Marque as mudanças das opções e um ou dois arquivos criados pelos mods e clique **Trazer selecionados para o pack** (o botão mostra quantos estão marcados). Na janela que aparece, clique **Continuar editando**.
5. Em **Configs**, com **Mostrando: arquivos do pack**, os arquivos que você trouxe têm que estar na lista. Abra um, mude um valor, clique **Salvar** e confirme com **Salvar**. Abra de novo e confira que a mudança ficou e os comentários continuam lá.
6. Troque para **Mostrando: instância de teste**: tem que aparecer o aviso **Você está editando a instância de teste, não o pack.** e todas as configs que o jogo criou.
7. Em **Buscar em todas as configs**, digite uma palavra (por exemplo `render`). Os resultados têm que vir agrupados por arquivo; clique em um e confira que o arquivo abre na linha certa. Teste também o filtro **Só deste mod**.
8. Teste de novo, mude outra opção no jogo e feche. No resultado, clique **Decidir depois**. Clique **Testar** outra vez: tem que aparecer **Mudanças do último teste para revisar**. Clique **Revisar agora**, depois **Descartar tudo** e confirme com **Descartar mudanças**.
9. Clique **Salvar versão** no alto do pack. Confira o número sugerido e o motivo, escreva uma nota, marque **Marcar como versão final** e clique **Salvar versão** (o botão mostra o número).
10. Em **Histórico**, a versão tem que aparecer como **Versão final · não publicada**; clique **Ver mudanças**. Depois, em **Mods**, remova um mod qualquer, volte ao **Histórico**, clique **Voltar para esta versão** na versão salva e confirme. O mod tem que voltar, e o estado de antes tem que aparecer em **Pontos de segurança…**.
11. Em **Exportar**, leia **1. Antes de exportar**, escolha **Arquivo .zip do pack packwiz**, confira **3. O que vai no pack** e clique **Exportar…**. No fim tem que aparecer **Pack exportado** e **Conferido com o packwiz.**; clique **Mostrar o arquivo**.
12. Publicar precisa de um token do GitHub seu e cria um repositório na sua conta. Se não quiser, pule para o fim. Em **Configurações**, na parte **Chaves e contas**, siga **Como criar** no **Token do GitHub**, cole o token, clique **Salvar** e depois **Testar**: tem que aparecer **Funcionando**.
13. Em **Histórico**, clique **Publicar versão** (com o número da versão) na versão final. Em **Primeira publicação deste pack**, escolha o nome do repositório e **Público** (qualquer pessoa com o link vê os arquivos do pack) ou **Privado** (só backup, o link não funciona para os jogadores), e clique **Criar repositório e continuar**. Leia os **Avisos** e as **Notas da versão** e clique **Publicar** (ou **Publicar mesmo assim**, se houver avisos).
14. Tem que aparecer **Versão … publicada** (com o número), com **Abrir a Release no GitHub**: abra e confira no navegador. De volta ao **Histórico**, a área **Publicação para os jogadores** tem que mostrar o **Link do pack** (use **Copiar link**) e **Como os jogadores instalam**, e a versão tem que estar como **Publicada**.

Deu tudo certo? Avise o orquestrador. Algo estranho? Use o **Relatar problema** (veja abaixo).
