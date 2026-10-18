# Marco 4: Ferramenta completa

Com este marco, o Warden deixa de só montar e testar o pack: ele ajuda a descobrir por que o jogo trava, mede o desempenho, testa como servidor e como o jogador recebe, importa modpacks prontos e exporta para todos os formatos combinados. Todas as funções da versão 1 já existem; falta a revisão final.

## O que chegou

- **Encontrar o mod culpado…:** no menu ▾ do **Testar**, no resultado de um travamento e em **Problemas**. O Warden abre o jogo várias vezes com partes do pack, numa cópia separada, até achar o mod. Mostra as **Rodadas**, o que está fazendo agora e os suspeitos; dá para **Pausar depois desta rodada**, **Continuar a busca** e **Cancelar busca**. Na opção **Outra coisa: eu digo se aconteceu**, ele pergunta a cada rodada (**Aconteceu** / **Não aconteceu**).
- **Busca do culpado com o servidor local:** quando o travamento é no servidor, ou na versão do Minecraft em que o jogo não entra direto no mundo, cada rodada abre um servidor só neste computador e o jogo entra nele (**Entendi, pode abrir**).
- **✦ Diagnóstico com IA:** em **Começar nova conversa**, você conta o que está acontecendo e escolhe por onde começar (o último travamento, um teste, um arquivo ou **Só o pack, sem log**). Antes de enviar, o Warden mostra exatamente o que vai para o Gemini e o custo estimado. Durante a conversa, a IA consulta o pack, os logs, as configs e os problemas já encontrados.
- **Propostas da IA com Aplicar:** a IA sugere mudanças (adicionar, remover ou trocar a versão de um mod, mudar uma config, ajustar o teste, **Encontrar o mod culpado**). Nada muda sem **Aplicar**; antes, ele mostra a diferença, e depois cria um ponto de segurança e oferece **Desfazer**.
- **O que este mod altera no jogo:** nos detalhes de um mod, o raio-x mostra que partes do jogo ele muda, com **Risco alto**, **Risco médio** ou **Risco baixo** quando dois mods alteram o mesmo ponto do jogo, e **Ver todas as alterações**.
- **Dependências:** nos detalhes do item, **Depende de**, **Usado por**, **Por que está no pack** e **Se você remover este item**. Em **Mods**, **Ver como** troca entre **Lista** e **Grafo**.
- **Saúde do pack:** nota de 0 a 100 (**Ótimo**, **Bom**, **Atenção**, **Crítico**) no alto de **Problemas**, com **O que tirou pontos**. Em **Meus packs**, a coluna **Saúde** mostra a nota de cada pack.
- **Console agrupado:** no teste, **Mostrar** troca entre **Linha a linha**, **Agrupado por mod** e **Só problemas**. Linhas que aparecem em muitos packs ficam marcadas como **comum, geralmente inofensivo**.
- **Memória do jogo ao vivo:** durante o teste, a **Memória do jogo** e as **Coletas de memória** aparecem em tempo real, com aviso quando o jogo fica quase sem memória. Em **Problemas**, **Memória no último teste**.
- **Testar com perfil de desempenho:** no menu ▾ do **Testar**. No resultado, **Desempenho** mostra **O que mais demorou para carregar** (por mod) e o resumo do spark.
- **Testar como servidor…:** no menu ▾ do **Testar**. Abre um servidor só neste computador (127.0.0.1) e entra nele com o jogo, ou só o servidor. A tela mostra **Console do servidor** e **Console do jogo** e aceita **Enviar comando ao servidor**.
- **Perfis do teste:** em **Ajustes do teste neste computador…**, **Novo perfil** guarda memória, Java, argumentos, mods opcionais, janela e **Ao abrir o jogo** (com **Entrar direto no mundo de teste**, do Minecraft 1.20 em diante). O perfil se troca no menu ▾ do **Testar**.
- **Testar como o jogador recebe:** no menu ▾ do **Testar**. Instala o pack do zero pelo link, como um jogador, e mostra as diferenças entre o que o jogador recebeu e o pack.
- **Configs em formulário:** em **Configs**, o arquivo abre em **Formulário** (chaves com **Ligado**/**Desligado**, números com faixa, cores, listas) ou em **Texto**.
- **Restaurar o padrão:** as chaves diferentes do padrão ficam marcadas como **alterado**. Dá para restaurar uma chave ou usar **Restaurar o arquivo todo para o padrão…**, sempre vendo as diferenças antes.
- **Editor de scripts:** em **Configs**, os scripts do pack (KubeJS, CraftTweaker) têm **Trechos prontos**, **Salvar e recarregar** com o jogo aberto, os erros do último teste na linha certa e **Abrir no VS Code**.
- **Mods opcionais e versão fixa:** em **Mais opções** de um item, marque como opcional (com **Descrição para o jogador** e **Ligado por padrão**) ou fixe a versão para o Warden não atualizar.
- **Trocar de versão…:** troca a versão de um mod (inclusive para uma mais antiga) e **Trocar versão do loader…** avisa quais mods param ou voltam a funcionar. Ao remover um mod, o Warden mostra as dependências que ficam sem uso e oferece removê-las juntas.
- **Modpacks na página Adicionar ao pack:** busque modpacks, use **Ver os mods** e adicione ao seu pack só os que quiser (**Só os que não estão no seu pack**).
- **Importar modpack:** em **Meus packs**, **Abrir ou importar…** aceita `.mrpack`, zip da CurseForge e instância do Prism, do MultiMC ou do app da CurseForge. O Warden mostra **O que foi reconhecido** e **O que acontece com cada arquivo** antes de criar o pack novo; o arquivo original não muda.
- **Exportar `.mrpack` e `.zip da CurseForge`:** novos formatos em **Exportar**, com **O que este formato perde** e a troca de mods pelo Modrinth quando possível.
- **Pacote para servidor (.zip):** em **Exportar**, **Gerar pacote para servidor…** monta o zip com `start.bat`, a memória recomendada e só os mods do lado servidor, e pode testá-lo no servidor deste computador.
- **Instância pronta para o Prism (.mrpack):** em **Exportar**, **Gerar instância…** cria o arquivo que o jogador arrasta para o Prism Launcher.
- **Mudanças feitas fora do Warden:** se outro programa mexer na pasta do pack, aparece **O pack foi alterado fora do Warden** e a tela passa a mostrar o que está no disco.
- **Armazenamento:** em **Configurações**, o espaço usado pelo Warden (cache, Minecraft e loaders, Javas, instâncias de teste) e **Limpar cache**.
- **Packs grandes:** packs com centenas de mods abrem, rolam e filtram sem travar a tela.

## O que ainda não tem

- O teste do caminho completo com o seu aceite, a revisão final de tudo e o instalador da versão 1: Marco 5 (Versão 1).
- A versão 1.1 ("Profissional") vem depois e é opcional.

Esta pré-versão sai do código mais recente, então algumas telas ainda podem mudar na revisão final. Agora o foco é **descobrir e resolver problemas do pack** e **levar o pack para outros lugares**.

## O que testar

Leva uns 45 minutos. Os passos 3 e 4 precisam da sua chave do Gemini (pule-os se não quiser usar a IA). Faça na ordem e anote o que der errado.

1. Em **Meus packs**, confira a coluna **Saúde** na lista. Abra um pack com Minecraft 1.20.1 e Fabric (o mesmo dos passos seguintes), vá a **Problemas** e confira a **Saúde do pack** com **O que tirou pontos**.
2. Use um pack que trava ao abrir o jogo (se não tiver um, peça ao orquestrador um mod que faz o jogo travar e adicione pela página **Adicionar ao pack**). Clique no ▾ ao lado de **Testar** e escolha **Encontrar o mod culpado…**. Escolha **Trava ao abrir o jogo** e clique **Começar a busca**. Acompanhe as **Rodadas**: no fim, tem que aparecer **Culpado encontrado:** com o mod certo e **Remover … do pack**.
3. Em **Configurações**, salve a **Chave do Gemini**. No pack, abra **✦ Diagnóstico com IA**, clique **Começar nova conversa**, escreva o que está acontecendo, escolha o último travamento e clique **Começar conversa**. Na janela **Começar uma conversa com a IA?**, confira o **Texto inicial, exatamente como vai** e o **Custo**, e clique **Começar conversa**.
4. Se a IA fizer uma **Proposta**, clique **Aplicar**, confira a diferença e confirme com **Aplicar**. Depois clique **Desfazer** e confira que o pack voltou como estava.
5. Clique no ▾ ao lado de **Testar** e escolha **Testar como servidor…**. Aceite a EULA, deixe **Abrir o servidor e entrar nele com o jogo** e clique **Abrir servidor e testar**. O jogo tem que entrar no servidor sozinho. Em **Mostrando**, troque entre **Servidor** e **Jogo** e mande o comando `list` em **Enviar comando ao servidor**. Termine com **Parar jogo e servidor**.
6. No teste, com o jogo aberto, confira a **Memória do jogo** mudando e troque o **Mostrar** do console para **Agrupado por mod**.
7. Baixe um `.mrpack` pequeno do site do Modrinth. Em **Meus packs**, clique **Abrir ou importar…** e **Escolher arquivo…**. Confira **O que foi reconhecido** e **O que acontece com cada arquivo**, crie o pack e veja se os mods aparecem em **Mods**.
8. No pack importado, vá a **Exportar**, escolha **Pacote para servidor (.zip)**, deixe marcado o teste no servidor deste computador e clique **Gerar pacote para servidor…**. Tem que aparecer **Pacote para servidor gerado** e **Pacote para servidor testado: o servidor abriu em …**.
9. Ainda em **Exportar**, escolha **Instância pronta para o Prism (.mrpack)** e clique **Gerar instância…**. Arraste o arquivo para a janela do Prism Launcher e confira que o jogo abre lá.

Deu tudo certo? Avise o orquestrador. Algo estranho? Use o **Relatar problema** (veja abaixo).
