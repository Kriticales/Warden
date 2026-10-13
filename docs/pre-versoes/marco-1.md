# Marco 1: Montar packs

Primeira versão para montar modpacks de verdade: criar ou abrir um pack, procurar mods no Modrinth e na CurseForge, adicionar vários de uma vez (inclusive por link ou arquivo do computador) e atualizar o que estiver velho.

## O que chegou

- **Primeira execução e Configurações:** o Warden pede as chaves (CurseForge e as outras) e guarda no cofre do Windows. A página **Configurações** tem tudo numa tela só, e **Sobre o Warden** mostra a versão e o commit.
- **Meus packs:** a lista dos seus packs, com **Criar pack** (nome, versão do Minecraft e loader: Forge, NeoForge ou Fabric) e **Abrir ou importar…** (uma pasta com `pack.toml`, de outro programa ou feita à mão).
- **Mods do pack:** a lista do que está no pack, com busca, e os detalhes de cada mod (de onde veio, versão, lado cliente ou servidor).
- **Adicionar ao pack:** uma página só, sem abas, que procura no Modrinth e na CurseForge ao mesmo tempo. Você pode marcar vários mods e adicionar todos juntos; o Warden mostra as dependências antes e acrescenta as que faltarem.
- **Descobrir mods:** página inicial com sugestões e categorias, e a pré-visualização completa do mod (descrição, imagens e versões) antes de adicionar.
- **Por link ou arquivo:** cole o link de um mod do Modrinth ou da CurseForge no campo de busca, ou use **Escolher arquivo do computador…** (ou arraste o `.jar` para a lista).
- **Atualizações:** mostra quais mods têm versão nova e atualiza um, alguns ou todos.
- Por baixo, o Warden guarda o histórico das mudanças de cada pack, para dar para voltar atrás.

## O que ainda não tem

Esta pré-versão sai do código mais recente, então algumas telas de marcos seguintes (testar no jogo, configs, histórico, exportar e outras) podem já aparecer, ainda incompletas. Elas são conferidas nas pré-versões dos próprios marcos: agora o foco é **montar packs**.

## O que testar

Leva uns 20 minutos. Faça na ordem e anote o que der errado.

1. Instale (veja **Como instalar**, abaixo) e abra o Warden. Na primeira execução, leia o aviso e clique em **Entendi**, escolha o nome do jogador e a pasta dos packs e, na etapa **Chaves (opcional)**, digite a chave da CurseForge.
2. Em **Configurações**, abra **Sobre o Warden** e confira que a versão e o commit são os desta página.
3. Em **Meus packs**, clique em **Criar pack**: escolha um nome, a versão 1.20.1 e o loader Fabric. Na etapa **Mods iniciais**, deixe marcados os que vierem (são mods que ajudam a achar problemas). O pack novo tem que aparecer na lista e abrir, já com esses mods.
4. Abra o pack e, em **Mods**, clique em **Adicionar** (abre a página **Adicionar ao pack**). Procure `sodium`. Os resultados devem vir das duas fontes (Modrinth e CurseForge). Adicione o Sodium.
5. Ainda em **Adicionar ao pack**, marque três mods de uma vez (por exemplo Lithium, Iris e Mod Menu) e adicione juntos. Se aparecer a tela de dependências, confira e confirme.
6. Abra a pré-visualização de algum mod (clique no nome): veja descrição, imagens e versões.
7. Cole no campo de busca o link de um mod da CurseForge (copie da página do mod no navegador) e adicione.
8. Clique em **Escolher arquivo do computador…** e adicione um `.jar` que você tenha baixado.
9. Na lista de **Mods**, confira que tudo o que você adicionou está lá (mais os mods iniciais), e abra os detalhes de um deles.
10. Feche o Warden, abra de novo e confira que o pack continua com os mesmos mods.
11. Em **Meus packs**, clique em **Abrir ou importar…** e escolha a pasta de um pack packwiz que você já tenha (se tiver). Ele tem que abrir com os mods certos.
12. Em **Mods**, clique em **Verificar atualizações**. Se algum mod aparecer com **Atualização disponível**, clique em **Atualizar** nele.

Deu tudo certo? Avise o orquestrador. Algo estranho? Use o **Relatar problema** (veja abaixo).
