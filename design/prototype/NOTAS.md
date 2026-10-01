# Warden — notas do protótipo de design

Protótipo clicável da interface do Warden. **Não é o código do app.** É a referência visual e de comportamento que os agentes de frontend vão seguir. Tudo roda em HTML, CSS e JavaScript estáticos, sem servidor e sem bibliotecas externas. Os dados são fictícios e nada é salvo.

## Como usar

Abra `index.html` no navegador ou a página publicada. A barra amarela do topo é do protótipo, não do app:

- **Direção visual**: troca entre as 3 direções (a escolha fica lembrada no navegador).
- **Estado da tela**: mostra os estados *carregando*, *vazio* e *erro* nas telas onde eles existem. Os botões riscados não se aplicam à tela atual.
- **Ir para**: pula direto para qualquer uma das 17 telas e subtelas.

No console do jogo há um quadro tracejado “Protótipo: simular o fim do teste”. Ele também não faz parte do app: serve para ver as duas saídas do teste (o jogo fechou normalmente ou o jogo travou).

## Arquivos

| Arquivo | O que tem |
|---|---|
| `tokens.css` | Sistema de tokens: escalas comuns e a paleta das 3 direções. **É a fonte da verdade das cores.** |
| `styles.css` | Componentes base (botões, campos, chips, tabelas, diálogos, árvore, console…). Só usa tokens. |
| `screens.css` | Estilos específicos das telas (Testar, sessão de jogo, diagnóstico…). |
| `identidade.css` | Tudo que é “jeito Minecraft/Deep Dark”, isolado: cena de fundo, trilho lateral, cantos em degrau, brilho, pulso. |
| `app.js` | Núcleo: estado, rotas, diálogos acessíveis, toasts, cena de fundo gerada. |
| `screens-pack.js` | Telas 1 a 5: Início, Criar, Editor (Mods, Resource packs, Shaders, Arquivos), Adicionar conteúdo, Configs. |
| `screens-test.js` | Telas 6 e 7: fluxo de Testar inteiro e Diagnóstico de crash. |
| `screens-more.js` | Telas 8 a 10: Versões, Exportar, Configurações. |
| `data.js` | Dados fictícios coerentes entre as telas. |
| `icons.js` | Ícones Lucide inline e a marca do Warden em pixel art. |
| `tools/contraste.mjs` | Verificador de contraste WCAG dos tokens (`node tools/contraste.mjs`). |

## As 3 direções visuais

O dono pediu as **cores do Warden** (o mob do Deep Dark, com o brilho do sculk) e **ideias do Minecraft** (pixel, blocos). As três direções usam os mesmos nomes de token; muda só o valor. Grafite e Ardósia, da primeira rodada, foram descartadas: o que a Ardósia tinha de pixel foi para o Deep Dark.

### 1. Deep Dark (padrão, a mais caprichada)

Paleta do Warden com identidade Minecraft forte:

- **Títulos de tela em fonte pixel** (Pixelify Sans), com uma palavra brilhando em ciano (“Meus **modpacks**”, “Testar o **pack**”). Frases longas e todo o texto corrido ficam na sans legível (Manrope). Exemplo: o título do diagnóstico, que é uma frase inteira, volta para a sans.
- **Cena de fundo em pixel art original**: uma caverna gerada pelo próprio protótipo, com pilares, estalactites em blocos, chão com camada de sculk, veias ciano e “almas” paradas no ar. Fica escurecida, levemente desfocada e com uma vinheta, atrás de cards translúcidos. Ela nunca disputa a leitura.
- **Partículas de alma** subindo devagar. Somem quando o sistema pede menos movimento.
- **Trilho lateral só de ícones**, com rótulo curto embaixo, item ativo com barra e brilho ciano (inspirado na referência GIGACAT, desenho próprio). Leitores de tela continuam ouvindo o nome completo (“Meus modpacks”, “Configurações”).
- **Botões com cantos em degrau de pixel.** O recorte fica nos pseudo-elementos, não no botão, para o anel de foco nunca ser cortado.
- **Botão Testar** como peça central: bloco ciano grande com “antenas” de pixel em cima (lembra um sensor de sculk, mas é forma própria) e pulso de brilho lento.
- **Barras de progresso em blocos** que brilham, anel de progresso quadrado, ponto “ao vivo” quadrado.
- Chips e etiquetas com cantos em degrau e caixa alta; separadores em blocos; cards com borda que acende em ciano no foco ou ao passar o mouse.
- Ícones provisórios dos mods em pixel art gerada a partir do nome. No app real eles são o ícone oficial vindo da API.

### 2. Sculk sóbrio

A mesma paleta, para usar o dia inteiro: títulos em Manrope, cantos levemente arredondados, sem cena de fundo, sem partículas e sem cantos em degrau. A veia de sculk aparece só, bem fraca, na barra lateral. A barra lateral é completa, com rótulos.

### 3. Calcita (claro)

Mantida porque saiu barata: são só valores de token. É a mesma estrutura em versão clara, com o verde-azulado escuro do Warden como destaque, sem texturas. O console continua escuro, porque log se lê melhor assim. Se o dono não quiser tema claro, basta apagar o bloco `calcita` de `tokens.css` e o botão da barra do protótipo.

## Paleta final (Warden)

Cores cruas de referência (definidas uma vez em `tokens.css`):

| Nome | Hex | Papel |
|---|---|---|
| abismo | `#050D12` | fundo do Deep Dark |
| sculk-900 | `#0A1A21` | painel (cards usam versão translúcida a 86%) |
| sculk-800 | `#0E2630` | superfície 2 (“corpo do Warden”) |
| sculk-700 | `#12313C` | superfície elevada, trilho das barras |
| sculk-600 | `#1E4552` | bordas de cards |
| alma | `#19D3E0` | destaque principal: botão primário, progresso, seleção |
| alma-escura | `#0FB5C2` | botão primário ao passar o mouse |
| alma-clara | `#7FF3F0` | partículas e brilhos |
| alma (texto) | `#5FE3EC` | ciano usado como texto/link sobre superfícies |
| osso | `#CFE3DA` | anel de foco e detalhes (distinto do ciano de seleção) |
| texto | `#E4F3EF` / `#B4CFCA` / `#8AACAA` | principal / secundário / apagado |
| sucesso | `#3DDC84` (texto `#6BE7A2`) | verde |
| aviso | `#F2B33D` (texto `#FFCB66`) | âmbar |
| erro | `#FF5C5C` (texto `#FF8E8E`, botão `#C9353A`) | vermelho |
| informação | `#B3A6FF` (texto `#C6BCFF`) | lavanda: nunca se confunde com o ciano do destaque |

Decisões de cor:

- **Foco em “osso”, não em ciano.** Ciano já significa “selecionado/ativo”; se o foco do teclado também fosse ciano, não daria para distinguir foco de seleção.
- **Informação em lavanda.** O azul comum ficaria parecido demais com o ciano.
- **O texto sobre o botão ciano é quase preto** (`#021418`): contraste de 10,25:1, acima até do nível AAA. O botão Testar é a peça central e merece leitura perfeita.

Contraste medido (WCAG 2.1) com `node tools/contraste.mjs`: 62 pares por direção, **todos aprovados** nas 3 direções. Alguns números do Deep Dark: texto principal 17,1:1; secundário 10,8:1; apagado sobre a superfície mais clara 5,6:1; contorno de campo 4,6:1 (mínimo 3); anel de foco 14,6:1.

## Sistema de tokens

- **Escalas comuns:** texto de 12 a 32 px (base 14 px, porque é um app desktop denso); espaçamento em múltiplos de 4 px; alturas de controle de 28, 36 e 48 px; durações de 120, 200 e 320 ms.
- **Por direção:** famílias tipográficas e tamanho do título (`--title-size`), raios (`--radius-*` e o degrau `--notch`), cores por papel (`--c-*`), sombras e brilho (`--shadow-*`, `--glow`), efeitos de identidade (`--fx-*`: textura de veia, chanfro de bloco, segmentos de progresso) e tons dos ícones provisórios (`--tile-*`).
- **Regra:** componente nunca usa cor ou medida literal. Exceções documentadas: a barra do protótipo (de propósito fora do tema) e as amostras de cor em Configurações → Aparência, que mostram a paleta de *cada* direção.
- **Para o app real:** os tokens viram variáveis CSS do frontend React do mesmo jeito. As fontes (Manrope, Pixelify Sans e IBM Plex Mono, todas SIL OFL) devem ir **embutidas no app**: aqui vêm do Google Fonts só porque é uma página. Os ícones devem vir do pacote `lucide-react`.

## Decisões de UX por tela

1. **Início.** Cards com o que importa para decidir o que abrir: versão do Minecraft, loader, número de mods, última versão salva e o status do último teste, sempre com ícone e texto, nunca só cor. Um pack que travou mostra “Ver diagnóstico” no lugar de “Testar”. Tem estados carregando, vazio e erro.
2. **Criar modpack.** As regras de loader aparecem *antes* do erro: NeoForge e Fabric ficam desativados com o motivo escrito (“O NeoForge existe a partir do Minecraft 1.20.1”). Versões do Minecraft vêm agrupadas e ordenadas pelo manifesto, nunca por texto. Versões de loader com problema conhecido aparecem bloqueadas. Um resumo lateral diz o que vai ser criado, inclusive o Java que será baixado.
3. **Editor do pack.** Mods em **cartões** (ícone, nome, descrição de 2 linhas e interruptor ativo/desativado, inspirado na referência GIGACAT) ou em **tabela** (densa, com seleção em lote, lado editável na linha e ações em lote). Filtros rápidos: com problemas, com atualização, só no cliente, não salvos. As atualizações são revisadas antes de aplicar. A gaveta do mod mostra versão, lado, “opcional para o jogador”, “fixar versão” e o `.pw.toml` real. No estado de erro, um arquivo ilegível vira um item com erro e não derruba a lista. A aba **Arquivos** mostra o que entra no pack (lista de permissões) e os arquivos do packwiz.
4. **Adicionar conteúdo.** Busca unificada com os filtros de Minecraft e loader **travados** (cadeado) e as fontes como filtro. Um mod que está nas duas fontes aparece uma vez só, e você escolhe de onde baixar. Os detalhes mostram versões (recomendada, beta), dependências obrigatórias pré-marcadas (que não dá para desmarcar), opcionais, “já no pack” e o aviso de incompatível com o motivo. Abas para colar link (com validação e erro claro) e para arquivo local ou URL (com exemplos de arquivo aceito, já existente e de loader errado). Adicionar o Cooking for Blockheads traz o Balm junto e resolve também o aviso do Waystones: as telas conversam entre si.
5. **Configs.** Árvore de arquivos navegável por teclado, editor em formulário com a descrição original do mod, valor padrão, faixa permitida e “desfazer” por campo, e modo texto. Cada arquivo diz de que tipo é: config do pack (sobrescreve sempre), config de servidor padrão (`defaultconfigs/`) ou primeira execução (`options.txt`). “Revisar e salvar” mostra o diff antes. No estado de erro, o arquivo abre em modo texto com a linha problemática marcada.
6. **Testar** (atenção especial, a pedido do dono). O Warden abre o Minecraft sozinho; **nenhum outro launcher aparece**. São quatro etapas com indicador de progresso:
   - **Início:** botão grande, com instância de trabalho ou “como o jogador vai receber”, entrar direto no mundo de teste e o resumo de Java, memória e jogador offline.
   - **Checagem:** erros, avisos e informações em linguagem simples, com “como sabemos” (a evidência) e correção em um clique. O botão Testar fica bloqueado enquanto houver erro, mas existe “testar mesmo assim”.
   - **Preparação:** anel de porcentagem, baixado de quanto, velocidade e tempo restante, mais etapas com contagem de arquivos e o arquivo atual. O leitor de tela é avisado quando cada etapa termina. O erro de download diz que o que já foi baixado fica guardado.
   - **Jogo aberto:** tempo de jogo, memória em uso, jogador, abrir a pasta e parar o jogo (com confirmação). O console tem filtros (tudo, avisos, erros), busca, rolar sozinho, “linhas completas” (por padrão a origem da linha é encurtada; o texto em si nunca é traduzido) e um contador ao vivo de mudanças detectadas.
   - **O que mudou:** resumo em números, arquivos agrupados em “sugerimos trazer” e “parecem pessoais”, mais os ignorados automaticamente. Cada arquivo mostra de onde vem e para onde vai (`saves/…/serverconfig` → `defaultconfigs/`). O `options.txt` é tratado chave por chave. Nada é copiado sem confirmar.
7. **Diagnóstico.** Primeiro o que aconteceu, em português simples, com o grau de certeza e por que a checagem não pegou antes. Depois as correções em um clique (a recomendada destacada), a prova no log com as linhas marcadas e o mod suspeito. A IA é uma “segunda opinião” opcional: o diálogo mostra o que vai (com caixas de seleção), quais dados pessoais são trocados e uma prévia com os trechos substituídos destacados. Também há o estado sem chave do Gemini.
8. **Versões.** Três cartões explicam o tipo de mudança em linguagem de jogador (Correção, Novidade, Grande mudança), com o número já calculado. Há sugestão com justificativa, canal Estável/Beta/Alfa, changelog gerado das mudanças reais e editável, e envio ao GitHub. Na linha do tempo, “voltar para esta versão” explica que nada é apagado e que as alterações atuais viram rascunho.
9. **Exportar.** Tabela com exatamente o que vai no pack, tamanhos e barras de tamanho (uma cor só, porque é magnitude), pastas expansíveis e total. Alertas antes de exportar: arquivo que identifica o computador (deixar de fora), arquivo grande sem fonte e `options.txt`. Mostra o que fica de fora automaticamente e a conferência com o packwiz. Você escolhe entre a versão salva ou o estado atual.
10. **Configurações.** Chaves mascaradas com mostrar/ocultar e testar, guardadas no cofre do Windows. Nome do jogador offline com explicação. Java automático por faixa de versão. Memória com faixa recomendada e alertas acima de 10 GB ou abaixo de 4 GB. Pastas e espaço, GitHub sem token próprio e aparência (direção visual e reduzir animações).

## Acessibilidade (o que foi verificado executando)

- **axe-core 4** rodado em **todas as 17 telas, em cada estado que a tela tem (normal, carregando, vazio, erro), nas 3 direções**, mais 6 diálogos ou gavetas abertos em cada direção: **nenhuma violação** (regras WCAG 2.0/2.1 A e AA, mais boas práticas). O axe não enxerga pseudo-elementos. Por isso os botões em degrau foram conferidos olhando, e usam os mesmos pares de cor já medidos.
- **Teclado:** foco visível em tudo (anel “osso” de 2 px); abas com setas; árvore de configs com setas, Home, End, Enter e Espaço; nos diálogos o foco entra, o Tab fica preso, o Esc fecha e o foco volta para quem abriu (testado nos 18 casos). Há atalho “Pular para o conteúdo”, e ao trocar de tela o foco vai para o título.
- **Leitores de tela:** rótulos em todos os campos, landmarks, `aria-current` na navegação, progresso com `role="progressbar"` e valores, avisos em região `aria-live`, console como `role="log"`, nome completo dos itens mesmo no trilho só de ícones.
- **Movimento:** com `prefers-reduced-motion` (ou a opção do app), as partículas somem e o pulso do botão para. Testado emulando a preferência.
- **Cor nunca sozinha:** todo estado tem ícone e texto; fontes de mod têm forma diferente no marcador (círculo, losango, anel) além da cor.
- **Larguras:** 1280 e 1024 px conferidas em captura de tela; abaixo de 1180 px a lateral vira trilho compacto.
- **Fluxo de ponta a ponta** automatizado: abrir pack → Testar → corrigir 2 erros → preparar → jogo → fechar → trazer mudanças; buscar e adicionar; link inválido; salvar versão; travamento → diagnóstico → IA; validação do Criar; editar config e revisar diff. Nenhum erro de JavaScript.

Não verificado: leitor de tela real (NVDA ou Narrador), zoom de 200% em todas as telas e o app Tauri real (WebView2).

## Autocrítica (o que melhorei antes de publicar e o que fica em aberto)

Corrigido depois de revisar as capturas:

- O botão contornado “Parar o jogo” ficava vermelho sólido no Deep Dark, porque a borda recortada vazava por baixo do fundo translúcido. O texto ficava ilegível; foi corrigido.
- O console quebrava palavras no meio. Agora cada linha tem colunas fixas (hora, nível, mensagem), quebra só onde pode e mostra a origem encurtada, com opção de linha completa.
- O horário das linhas novas do console não batia com o do log de exemplo.
- Títulos longos em pixel cansavam a leitura: pixel ficou só para títulos curtos.
- Barras de ferramentas que quebravam linha sem querer foram reorganizadas em duas linhas intencionais.
- A cena de fundo estava apagada demais. Foi reforçada, mas continua atrás de uma vinheta e de cards translúcidos.
- Classe com nome repetido (`.alert`) deformava o contador do Diagnóstico.
- Medidor de memória duplicado com o controle deslizante.

Em aberto, para o dono decidir:

1. **Direção final:** Deep Dark, Sculk sóbrio ou as duas no app (com Calcita como opção clara)?
2. **Mods: cartões ou tabela como padrão?** O protótipo abre em cartões (mais bonito, como a referência). A tabela é melhor para packs grandes e ações em lote.
3. **Trilho de ícones no Deep Dark:** mais bonito, mas um pouco menos explícito que a lateral com nomes.
4. **Ícones provisórios em pixel:** no app real entram os ícones oficiais dos mods. O pixel gerado fica só como reserva para arquivo local sem ícone?

## Origem de cada coisa (licenças)

- Ícones: traçados do **Lucide** (licença ISC), copiados inline em `icons.js`.
- Fontes: **Manrope**, **Pixelify Sans** e **IBM Plex Mono**, todas SIL Open Font License, servidas pelo Google Fonts no protótipo.
- Pixel art (marca do Warden, cena de caverna, veias de sculk, ilustração dos estados vazios, ícones provisórios, “antenas” do botão Testar): **criação original deste protótipo**, gerada por código. Nenhuma textura, logo, personagem ou asset da Mojang ou da Microsoft. A palavra “Warden” e as cores são só inspiração.
- Referências do dono (Behance: GIGACAT, WaterLand, Mythera): usadas como inspiração de linguagem (trilho de ícones, cards com interruptor, títulos pixel com palavra destacada, brilho no card em foco). Nenhuma tela, logo ou imagem foi copiada.
- Nomes de mods reais (Create, Sodium, JEI…) aparecem só como exemplo. Versões, problemas, logs e datas são inventados para contar uma história coerente.
