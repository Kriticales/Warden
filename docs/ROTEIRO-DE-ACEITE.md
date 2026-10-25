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

11. **Publicar versão.** Na mesma janela, clique **Publicar versão 1.0.0**. Em **Primeira publicação deste pack**, deixe o nome do repositório sugerido, escolha **Público** e clique **Criar repositório e continuar**. Leia os **Avisos**. Se aparecer **A versão 1.0.0 não foi testada.**, clique **Testar agora**. Na tela do teste, clique **Testar de novo**, feche o jogo pelo menu dele quando abrir e volte a **Publicar versão 1.0.0** pelo **Histórico**: o aviso tem que ter sumido. Clique **Publicar**.

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

Os cinco caminhos das funções avançadas, do jeito que você vai usar no dia a dia. Leva uns 50 minutos. O caminho 3 precisa da chave do Gemini e o caminho 1, da chave da CurseForge (salve as duas em **Configurações** → **Chaves e contas** antes de começar). Faça na ordem e anote o que der errado; no fim de cada caminho há o que conferir com o teclado.

### 1. Descobrir e adicionar vários mods de uma vez

1. Abra um pack com Minecraft 1.21.1 e Fabric e clique **Adicionar**, em **Mods**. Tem que abrir **Adicionar ao pack**, com **Populares** e **Atualizados recentemente**, os filtros à direita e, embaixo da busca, a linha **Só versões para Minecraft 1.21.1 com Fabric…**. O que já está no pack aparece com **No pack**; as bibliotecas ficam numa linha **… bibliotecas escondidas**, com **Mostrar**.
2. Digite `sodium` no campo de busca. Os resultados das duas fontes aparecem numa lista só, cada um com a marca da fonte (quadrado verde do Modrinth, losango laranja da CurseForge).
3. Clique no nome de um resultado. No lugar dos filtros aparece o detalhe, com a descrição, a galeria, as versões e as dependências; **← Filtros** volta.
4. Marque a caixa de dois resultados que ainda não estão no pack. Embaixo aparece **2 marcados**, com os nomes, e o botão **Adicionar 2 ao pack →**. Troque para **Cartões** ao lado da busca: os dois continuam marcados. Feche e abra a página de novo: ela volta em **Cartões**.
5. Clique **Adicionar 2 ao pack**. Abre **uma** janela só, **Adicionar … itens ao pack**, com um bloco por mod escolhido e as dependências penduradas nele (as que os dois pedem aparecem uma vez, em **Em comum**). Clique numa dependência: à direita aparece o porquê dela. O título, o botão e a conta do rodapé têm de mostrar o mesmo número.
6. Clique **Adicionar … itens**. Aparece o aviso **… itens adicionados ao pack**, os dois ficam com **No pack** na busca e o botão do alto vira **Mods · … adicionados**.
7. Clique **Mods**, no alto. Os itens novos estão na lista.
8. Teclado: volte a **Adicionar**, clique no campo de busca e use só **Tab**, **Espaço**, **Enter** e as setas. O contorno do foco tem que aparecer em cada parada; **Espaço** marca a caixa de um resultado, **Enter** no nome abre o detalhe e as setas trocam **Lista** e **Cartões**.

### 2. Trazer mods de outro modpack

1. Em **Adicionar ao pack**, troque **Tipo** para **Modpacks** e busque um modpack conhecido.
2. Clique **Ver os mods** num resultado. À direita aparece **Mods deste modpack (…)**, com **No pack** nos que você já tem, **Sem versão para …** nos que não servem e **Arquivo fora das lojas** nos jars do autor (esses não podem ser marcados).
3. Marque dois mods que ainda não estão no seu pack e clique **Adicionar 2 selecionados**. Abre uma janela só, com o que vai entrar.
4. Confirme. Os dois passam a mostrar **No pack** na lista do modpack, e aparecem em **Mods** quando você voltar.

### 3. Travou sem causa: encontrar o culpado e conversar com a IA

1. Use um pack que trava ao entrar no mundo sem o Warden achar a causa no log (se não tiver um, peça ao orquestrador um mod que faz isso).
2. Clique no ▾ ao lado de **Testar** e escolha **Encontrar o mod culpado…**. Na primeira vez, a janela explica o mundo de teste e pede **Aceito a EULA**.
3. Escolha **Trava ao entrar no mundo** e clique **Começar a busca**. O botão do alto vira **Buscando o culpado: ver progresso** e a tela mostra as **Rodadas**.
4. No fim, tem que aparecer **Culpado encontrado:** com o mod certo e a **Confiança**.
5. Clique **Ver o que o … altera no jogo**. Abrem os detalhes do mod em **Mods**, com o bloco **O que este mod altera no jogo**. Feche com **Esc**.
6. Abra **✦ Diagnóstico com IA**. Em **Nova conversa**, escolha **Trava ao entrar no mundo**, escreva em **Contar algo à IA** o que a busca encontrou e clique **Começar conversa**.
7. Na janela **Começar uma conversa com a IA?**, confira que o seu texto está no **Texto inicial, exatamente como vai**, e clique **Começar conversa**.
8. A resposta aparece com **O que a IA afirma**, e cada consulta da IA fica num bloco **Enviado à IA**. Abra um bloco e confira o que foi enviado.

### 4. Testar como servidor e gerar o pacote para servidor

1. Teclado: clique em qualquer lugar vazio do cabeçalho do pack e aperte **Tab** até o ▾ ao lado de **Testar** ficar com o contorno do foco. Aperte **Enter**: o menu abre. Desça com a **seta para baixo** até **Testar como servidor…** e aperte **Enter**.
2. Na janela **Aceitar a EULA do Minecraft?**, aperte **Esc**. A janela fecha e o foco volta para o ▾.
3. Abra de novo pelo ▾, clique **Aceito a EULA**, deixe **Abrir o servidor e entrar nele com o jogo** e clique **Abrir servidor e testar**.
4. A tela **Testando como servidor** mostra o **Console do servidor**; o jogo entra no servidor sozinho.
5. Clique **Parar jogo e servidor** e confirme. Tem que aparecer **Encerrado por você** e **Entrou no servidor.**
6. Vá a **Exportar**. Escolha **Instância pronta para o Prism (.mrpack)** e depois **.mrpack**, só para ver as explicações de cada formato.
7. Escolha **Pacote para servidor (.zip)**. Como a EULA já foi aceita, **Testar no servidor deste computador depois de gerar** já vem marcado.
8. Clique **Gerar pacote para servidor…** e escolha onde salvar. Tem que aparecer **Pacote para servidor gerado** e **Pacote para servidor testado: o servidor abriu em …**.

### 5. Importar um modpack de outro app

1. Baixe um `.mrpack` pequeno do site do Modrinth.
2. Em **Meus packs**, clique **Abrir ou importar…** e **Escolher arquivo…**. Abre **Importar modpack**, com o nome, a versão do Minecraft e o loader.
3. Confira **O que foi reconhecido** e **O que acontece com cada arquivo**: os mods das lojas viram referência, os jars soltos ficam como arquivo local e o lixo de outros launchers não entra.
4. Clique **Importar como pack novo**. O pack novo abre em **Mods**, com os mods do modpack.
5. Se algum item ficou para revisar, **Problemas** mostra o aviso com a lista; **Revisar** abre o item em **Mods** e **Já revisei** tira o aviso.
6. Volte a **Meus packs**: o pack novo está na lista. O arquivo `.mrpack` que você baixou não muda.
7. Em **Configurações**, desça até **Armazenamento** e confira o espaço usado (cache, Minecraft e loaders, Javas, instâncias de teste).

Deu tudo certo? Avise o orquestrador. Algo estranho? Use o **Relatar problema**, na barra de título, com a tela e o passo.
