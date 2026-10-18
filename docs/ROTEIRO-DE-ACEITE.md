# Roteiro de aceite da versão 1

Este é o teste final do Warden antes do instalador da versão 1: o caminho inteiro, do pack novo até o jogador recebendo o pack, num pack de verdade e no seu Windows. O mesmo caminho roda sozinho todas as noites na CI (`apps/desktop/e2e/full-flow.e2e.ts`), com o jogo e os sites simulados; aqui ele roda com o Minecraft, o Modrinth, a CurseForge e o GitHub de verdade.

**Leva uns 60 minutos** (uns 45 se você pular a instalação no Prism Launcher, passo 15).

Antes de começar, tenha à mão:

- a sua **chave da CurseForge** (a mesma do `.env`);
- um **token do GitHub** seu (o passo 2 explica onde criar);
- uns 2 GB livres no disco (o Minecraft e o loader são baixados no primeiro teste).

Faça na ordem. Em cada passo, escreva em **Deu certo?** "sim" ou "não" e, se algo saiu diferente do que está escrito, o que apareceu na tela. Se travar num passo, use o **Relatar problema** (na barra de título) e pare ali.

## O caminho completo

1. **Abrir o Warden.** Abra o Warden pelo atalho. Tem que aparecer **Meus packs**.

   Deu certo? (sim/não e o que aconteceu):

2. **Chaves.** Clique em **Configurações**. Em **Chaves e contas**, cole a **Chave da CurseForge**, clique **Salvar** e depois **Testar**: tem que aparecer **Funcionando**. No **Token do GitHub**, siga **Como criar**, cole o token, clique **Salvar** e **Testar**: tem que aparecer **Funcionando**. As duas linhas mostram **Configurada**, sem mostrar a chave.

   Deu certo? (sim/não e o que aconteceu):

3. **Criar o pack.** Volte a **Meus packs** e clique **Criar pack**. Dê o nome **Teste da Versão 1** e clique **Próximo**. Escolha o Minecraft **1.21.1** e clique **Próximo**. Escolha **Fabric** e clique **Próximo**. Na etapa **Mods iniciais**, deixe o spark e o Crash Assistant marcados e clique **Próximo**. No resumo, clique **Criar pack**. O pack tem que abrir em **Mods** com o spark, o Crash Assistant e a Fabric API.

   Deu certo? (sim/não e o que aconteceu):

4. **Adicionar pela busca.** Em **Mods**, clique **Adicionar**. Na página **Adicionar ao pack**, busque `sodium`. O Sodium tem que aparecer **uma vez só**, com **Modrinth e CurseForge** na linha. Clique no nome **Sodium**: no painel do lado, confira a versão e clique **Adicionar ao pack**. Na janela, confira **O que você escolheu** e clique **Adicionar 1 item** (o número pode ser outro, se ele trouxer dependências). Tem que aparecer **1 item adicionado ao pack**.

   Deu certo? (sim/não e o que aconteceu):

5. **Mais um mod.** Ainda na busca, procure `lithium`, marque a caixa do **Lithium** e clique **Adicionar 1 ao pack** e depois **Adicionar 1 item**. Clique **Voltar para Mods**: o Sodium e o Lithium têm que estar na lista.

   Deu certo? (sim/não e o que aconteceu):

6. **Configurar.** Vá a **Configs**. Abra a pasta `crash_assistant` e o arquivo `config.toml`. Em **Texto**, acrescente no fim uma linha como `# meu ajuste` e clique **Salvar**. Na janela, confira a diferença (a linha nova aparece marcada) e confirme com **Salvar**. Tem que aparecer **config.toml salvo.**

   Deu certo? (sim/não e o que aconteceu):

7. **Testar.** Clique **Testar** no alto do pack. Na primeira vez o Warden baixa o Java, o Minecraft e o Fabric (pode levar alguns minutos; a tela mostra cada etapa). O jogo tem que abrir com os mods. Enquanto ele abre, o botão do alto diz **Jogo aberto: ver teste** e o console mostra as linhas do jogo.

   Deu certo? (sim/não e o que aconteceu):

8. **Mudar algo no jogo.** No jogo, crie um mundo ou entre num, abra **Opções…** e mude a **Distância de renderização**. Feche o jogo pelo menu do próprio jogo (**Salvar e sair** e depois **Sair do jogo**). O resultado do teste tem que dizer que o jogo fechou normalmente.

   Deu certo? (sim/não e o que aconteceu):

9. **Trazer o que mudou.** No resultado, em **O que mudou durante o teste**, tem que aparecer **Opções do jogo (options.txt)** com a distância de renderização. Deixe marcada e clique **Trazer 1 selecionado para o pack** (o número é o de itens marcados). Na janela, clique **Continuar editando**. Em **Configs**, a **Opções do jogo (options.txt)** tem que estar na lista com o valor novo.

   Deu certo? (sim/não e o que aconteceu):

10. **Salvar versão.** Clique **Salvar versão** no alto do pack. Escreva `1.0.0` no número, escreva uma nota curta, marque **Marcar como versão final** e clique **Salvar versão 1.0.0**. Tem que aparecer **Versão 1.0.0 salva como versão final**.

    Deu certo? (sim/não e o que aconteceu):

11. **Publicar versão.** Na mesma janela, clique **Publicar versão 1.0.0**. Em **Primeira publicação deste pack**, deixe o nome do repositório sugerido, escolha **Público** e clique **Criar repositório e continuar**. Leia os **Avisos**. Se aparecer **A versão 1.0.0 não foi testada.**, clique **Testar agora**, feche o jogo pelo menu dele quando abrir e volte a **Publicar versão 1.0.0** pelo **Histórico**: o aviso tem que ter sumido. Clique **Publicar**.

    Deu certo? (sim/não e o que aconteceu):

12. **Versão publicada.** Tem que aparecer **Versão 1.0.0 publicada**, com o **Link do pack** e **Como os jogadores instalam**. Clique **Abrir a Release no GitHub** e confira no navegador que a Release 1.0.0 existe. Volte ao Warden, clique **Copiar link** e guarde o link (num bloco de notas) para o passo 15. Em **Histórico**, a versão tem que estar como **Publicada**.

    Deu certo? (sim/não e o que aconteceu):

13. **Instalar como um jogador.** Clique no ▾ ao lado de **Testar** e escolha **Testar como o jogador recebe**. O Warden instala o pack do zero pelo link do pack, com o packwiz-installer-bootstrap, numa instância separada, e abre o jogo. Feche o jogo pelo menu dele. No resultado tem que aparecer **Instalado do zero pelo link do pack (versão 1.0.0 publicada).** e **O jogador recebeu o mesmo que o pack.**

    Deu certo? (sim/não e o que aconteceu):

14. **Exportar.** Vá a **Exportar**. Leia **1. Antes de exportar**, escolha **Arquivo .zip do pack packwiz**, confira **3. O que vai no pack** e clique **Exportar…**. Escolha uma pasta. Tem que aparecer **Pack exportado** e **Conferido com o packwiz.**; clique **Mostrar o arquivo** e confira que o `.zip` está lá.

    Deu certo? (sim/não e o que aconteceu):

15. **Opcional: o pack num launcher de jogador.** Se você tem o Prism Launcher, crie nele uma instância com o Minecraft 1.21.1 e o Fabric e siga o **Como os jogadores instalam** do passo 12, com o link que você guardou. Abra a instância: o jogo tem que abrir com os mesmos mods do Warden e com a distância de renderização do passo 8.

    Deu certo? (sim/não e o que aconteceu):

16. **De volta a Meus packs.** Clique **← Meus packs**. A linha do **Teste da Versão 1** tem que mostrar a versão **1.0.0** e o último teste.

    Deu certo? (sim/não e o que aconteceu):

Deu tudo certo? Avise o orquestrador. Algo estranho? Use o **Relatar problema** e diga o número do passo.

## Funções avançadas

Preenchido pela A-07.
