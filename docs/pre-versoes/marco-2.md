# Marco 2: Testar

Agora dá para testar o pack no jogo de verdade, sem sair do Warden: um clique em **Testar** baixa o Minecraft, o loader e o Java certo, abre o jogo e mostra o console, a memória e o tempo de carregamento. Se o jogo travar, o Warden diz o porquê e junta os travamentos com a mesma causa.

## O que chegou

- **Testar:** botão no alto do pack. O Warden baixa o jogo, o loader e o Java na primeira vez, copia o pack para a instância de teste e abre o Minecraft com o seu nome de jogador (offline). As etapas aparecem na tela, com **Cancelar teste**. Só um jogo por vez, e o Warden avisa se você tentar fechá-lo com o jogo aberto.
- **Java automático:** o Warden escolhe sozinho o Java que cada versão do Minecraft aceita. Em **Configurações**, na parte **Teste**, a tabela **Java** mostra os Javas baixados, os packs que usam cada um e o motivo da escolha.
- **Do 1.7.10 até a versão mais nova:** Forge, NeoForge e Fabric abrem pelo Testar.
- **Console do jogo:** a saída do jogo ao vivo, com filtro por nível (**Só avisos e erros**, **Só erros**), **Buscar no console**, **Pausar rolagem**, **Copiar tudo** e **Salvar em arquivo…**. Os **Testes anteriores** guardam o console de cada teste.
- **Desempenho do jogo:** faixa acima do console com a **RAM do processo** e os tempos **Abriu em** e **Entrou no mundo em**. No resultado, o pico de RAM e a comparação do tempo com a última versão testada do pack.
- **Downloads manuais da CurseForge:** quando o autor de um mod não deixa outros apps baixarem, o teste para e mostra a lista, com **Abrir página de download** e **Selecionar arquivo baixado…**. O Warden também procura sozinho na pasta Downloads e o teste continua quando todos forem encontrados.
- **Problemas:** o Warden confere o pack antes de cada teste (dependência faltando, mod duplicado, mod de outro loader ou de outra versão do Minecraft, Java errado, conflitos conhecidos). A seção lista tudo por gravidade, com **Como sabemos** e correções de um clique (por exemplo adicionar a dependência que falta ou mudar o lado de um mod). Também tem **Verificar agora**, **Ignorar neste pack**, um contador no menu do pack e marcas na lista de Mods. Se houver erro ao clicar em Testar, um aviso oferece corrigir ou **Testar mesmo assim**.
- **Por que travou:** quando o jogo trava, o resultado do teste mostra a causa provável com o trecho do log que a prova, inclusive qual mod falhou ao alterar o jogo. **Copiar relatório** copia tudo sem dados pessoais.
- **Travamentos:** em **Problemas**, os travamentos dos seus testes ficam numa lista agrupada pela causa: o mesmo travamento vira uma linha só, com quantas vezes aconteceu, em quais versões e se voltou a acontecer. O Warden guarda os 30 testes mais novos que fecharam normalmente e os que travaram até 500 MB; perto disso ele avisa e oferece **Apagar os mais antigos**.
- **Dependências nos detalhes do mod:** **Depende de**, **Usado por**, **Por que está no pack** e **Se você remover este item** (quais mods param de funcionar).

## O que ainda não tem

- Editar configs, ver **O que mudou durante o teste**, salvar versões, exportar e publicar para os jogadores: Marco 3.
- Diagnóstico com IA, busca do mod culpado, perfis do teste, servidor local, grafo desenhado, console agrupado e nota de saúde do pack: Marco 4.
- Instalador final, revisão completa e uso diário: Marco 5 (Versão 1).

Esta pré-versão sai do código mais recente, então algumas telas de marcos seguintes podem já aparecer, ainda incompletas. Elas são conferidas nas pré-versões dos próprios marcos: agora o foco é **testar o pack no jogo**.

## O que testar

Leva uns 40 minutos (o primeiro teste baixa o Minecraft e o Java e demora mais). Faça na ordem e anote o que der errado.

1. Abra o pack do Marco 1 (Minecraft 1.20.1 com Fabric). Em **Problemas**, clique **Verificar agora**. Tem que aparecer **Nenhum problema encontrado** ou a lista em **Erros**, **Avisos** e **Informações**; se houver algum, abra **Como sabemos** e veja se a explicação faz sentido.
2. Em **Mods**, abra os detalhes de um mod que outros usam (por exemplo a Fabric API). No bloco **Dependências**, confira **Usado por**, **Por que está no pack** e **Se você remover este item**.
3. Clique **Testar** no alto do pack. A tela **Testando o pack** mostra as etapas (**Verificar o pack**, **Preparar o Minecraft**, **Copiar o pack para o teste**, **Abrir o jogo**) e, em **Este teste**, o Java escolhido como "(automático)". Na primeira vez isso leva alguns minutos.
4. Se aparecer **mods da CurseForge precisam ser baixados à mão**: clique **Abrir página de download**, baixe o arquivo e use **Selecionar arquivo baixado…** (ou só espere o Warden achar na pasta Downloads). O teste tem que continuar sozinho. Se não aparecer, pule este passo.
5. Enquanto o jogo abre, olhe o **Console do jogo** ao vivo. Teste **Só avisos e erros**, **Buscar no console** e **Pausar rolagem**. Na faixa **Desempenho do jogo**, a **RAM do processo** tem que mudar a cada segundo e, quando o menu do jogo aparecer, surgir o **Abriu em**.
6. No jogo, crie um mundo e entre nele (a faixa mostra **Entrou no mundo em**). Jogue um pouco, saia e feche o jogo pelo menu do próprio jogo. O resultado tem que dizer **O jogo fechou normalmente** e mostrar o **Desempenho do teste**.
7. Clique **Testar de novo**: desta vez tem que ser bem mais rápido. Com o jogo aberto, tente fechar o Warden: tem que aparecer **Fechar o Warden com o jogo aberto?**. Clique **Continuar no Warden**.
8. Ainda nesse teste, entre no mundo e segure **F3 + C** por uns 10 segundos para forçar um travamento. O resultado tem que dizer **O jogo travou** e, em **Por que travou**, "Travamento pedido no jogo (F3 + C)".
9. Clique **Copiar relatório** e cole num bloco de notas: confira que não aparece o seu nome de usuário do Windows nem caminhos com ele.
10. Teste mais uma vez e trave do mesmo jeito (**F3 + C**). Em **Problemas**, no bloco **Travamentos**, os dois têm que aparecer numa linha só, com "Travou 2 vezes". Clique **Ver as 2 vezes** e depois **Ver o que causou**.
11. No botão ao lado de **Testar** (**Mais opções do teste**), clique **Ver último teste**. Em **Testes anteriores**, abra um teste antigo e confira que o console dele está lá.
12. Em **Configurações**, na parte **Teste**, confira a tabela **Java**: o Java usado tem que aparecer com o nome do pack em **Usado por** e o motivo em **Por que esta versão**.
13. Em **Meus packs**, clique **Criar pack** com Minecraft 1.12.2 e Forge (deixe os **Mods iniciais** como vierem) e clique **Testar**. O Warden tem que baixar sozinho o Java 8 e o jogo abrir até o menu. Feche o jogo e confira que a tabela **Java** agora tem dois Javas.

Deu tudo certo? Avise o orquestrador. Algo estranho? Use o **Relatar problema** (veja abaixo).
