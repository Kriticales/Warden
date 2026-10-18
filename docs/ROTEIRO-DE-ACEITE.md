# Roteiro de aceite

## Funções avançadas

Os cinco caminhos das funções avançadas, do jeito que você vai usar no dia a dia. Leva uns 50 minutos. O caminho 3 precisa da chave do Gemini e o caminho 1, da chave da CurseForge (salve as duas em **Configurações** → **Chaves e contas** antes de começar). Faça na ordem e anote o que der errado; no fim de cada caminho há o que conferir com o teclado.

### 1. Descobrir e adicionar vários mods de uma vez

1. Abra um pack com Minecraft 1.21.1 e Fabric e clique **Adicionar**, em **Mods**. Tem que abrir **Adicionar ao pack**, com **Populares para Minecraft 1.21.1 com Fabric** e **Atualizados recentemente**.
2. Digite `sodium` no campo de busca. Os resultados das duas fontes aparecem numa lista só, cada um com a marca **Modrinth**, **CurseForge** ou **Modrinth e CurseForge**.
3. Clique no nome de um resultado. À direita aparece a pré-visualização, com a descrição, a galeria, as versões e as dependências.
4. Marque a caixa de dois resultados que ainda não estão no pack. Embaixo aparece **2 selecionados** e o botão **Adicionar 2 ao pack**.
5. Clique **Adicionar 2 ao pack**. Abre **uma** janela só, **Adicionar 2 itens**, com **O que você escolheu** e as dependências obrigatórias de cada um.
6. Clique **Adicionar … itens**. Aparece o aviso **… itens adicionados ao pack**, os dois ficam com **Já no pack** na busca e o botão do alto vira **Voltar para Mods · … adicionados**.
7. Clique **Voltar para Mods**. Os itens novos estão na lista.
8. Teclado: volte a **Adicionar**, clique no campo de busca e use só **Tab**, **Espaço** e **Enter**. O contorno do foco tem que aparecer em cada parada; **Espaço** marca a caixa de um resultado e **Enter** no nome abre a pré-visualização.

### 2. Trazer mods de outro modpack

1. Em **Adicionar ao pack**, troque **Tipo** para **Modpacks** e busque um modpack conhecido.
2. Clique **Ver os mods** num resultado. À direita aparece **Mods deste modpack (…)**, com **Já no pack** nos que você já tem, **Sem versão para …** nos que não servem e **Arquivo fora das lojas** nos jars do autor (esses não podem ser marcados).
3. Marque dois mods que ainda não estão no seu pack e clique **Adicionar 2 selecionados**. Abre uma janela só, com o que vai entrar.
4. Confirme. Os dois passam a mostrar **Já no pack** na lista do modpack, e aparecem em **Mods** quando você voltar.

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
