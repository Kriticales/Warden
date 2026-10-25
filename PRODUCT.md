# Product

<!-- impeccable:product-schema 1 -->

> Confirmado pelo dono em 19/10/2026. Cada item cita a fonte no repositório entre parênteses.

## Platform

web

## Users

- **Usuário principal: o próprio dono.** O Warden é de uso pessoal e privado e nunca será distribuído publicamente (ADR-0004; SPEC §1). Ele monta modpacks de Minecraft para outras pessoas jogarem (SPEC §1; SPEC §10 D14).
- **Leigo em programação, exigente com qualidade.** Quer funcionamento real e código muito bem feito (AGENTS.md). A interface usa linguagem simples e explica cada termo técnico (SPEC §1 princípio 5; QUALITY §8.1).
- **Situação de uso:** Windows 11, com o Warden maximizado em 1920×1080 (SPEC §10 D38). Valida cada versão instalando o app e seguindo um roteiro de aceite (ROADMAP §2).
- **Tarefa:** montar, ajustar, testar, consertar e entregar um modpack packwiz. O caminho do dia a dia é abrir o pack → adicionar mods → ajustar configs → testar → ver o que mudou → salvar versão → exportar ou publicar (ESTRUTURA §1 regra 7; ESTRUTURA §8).
- **Público indireto: os jogadores do pack.** Não usam o Warden. Recebem as versões finais pelo link do pack (packwiz-installer-bootstrap no Prism ou no MultiMC) ou por uma instância pronta para o Prism, e podem mandar o log de um travamento (SPEC §2; SPEC T18; SPEC §10 D29 e D34). São amigos do dono, sem experiência técnica (confirmado pelo dono em 19/10/2026).

## Product Purpose

- **O que faz:** app de desktop para criar, editar, versionar, testar e diagnosticar modpacks de Minecraft no formato packwiz, com launcher offline embutido e diagnóstico com IA (README.md; SPEC §1).
- **Cobertura:** Minecraft de 1.7.10 até a versão mais nova, com Forge (todas as versões), NeoForge (1.20.1+) e Fabric (1.14+); versões anteriores à 1.7.10 só como "melhor esforço" (SPEC §4; ADR-0005).
- **Por que existe:** substitui os protótipos `packwiz-gui` e `packwiz-gui-manager`. A ponte com o Prism era a fonte de defeito mais recorrente nesses apps (AGENTS.md; ADR-0010).
- **Sucesso:** a pasta do pack fica sempre limpa e no formato packwiz puro. Nada é dado como pronto sem prova contra o packwiz real e o jogo real. O dono descobre por que o jogo travou e entrega versões que os jogadores recebem sozinhos (SPEC §1 princípios 1 a 4; QUALITY §1).
- **Estado:** versão 1.0.0 lançada, conserto 1.0.1 publicado e a 1.1, com os consertos e as telas novas do relatório de bugs nº 1, preparada (docs/versoes/1.0.0.md; docs/versoes/1.0.1.md; docs/versoes/1.1.0.md; ADR-0071). A próxima é o Warden 1.2 "Profissional", marco M6: segurança dos mods, manutenção e substitutos, travamento de um jogador, notas e grupos, itens repetidos e desempenho entre versões (ROADMAP §13; SPEC §4).

## Positioning

- **O pack não fica preso ao Warden.** Ele é 100% packwiz, e qualquer ferramenta que lê packwiz consegue abri-lo (SPEC §1).
- **Junta o ciclo inteiro de quem cria um pack: criar → testar → diagnosticar → versionar → publicar.** Os launchers concorrentes foram feitos para jogar, e as ferramentas declarativas não testam nem diagnosticam (docs/research/08-concorrencia-e-mercado.md §0 item 1, marcado pela própria pesquisa como inferência).
- **Mecanismos que, segundo a pesquisa, ninguém maduro oferece:** projeto separado do teste, com "O que mudou durante o teste"; busca automática do mod culpado, que respeita as dependências e acha duplas de mods; travamentos agrupados pela causa e ligados às versões; testar "como o jogador recebe"; versões no git sem mostrar git; IA que cita a evidência e só muda o pack quando o dono clica em **Aplicar**, com a diferença exata à vista (docs/research/08-concorrencia-e-mercado.md §7.1, inferência da pesquisa).
- **Sem anúncio, sem telemetria, tudo local** (SPEC §7 regra 8; docs/research/08-concorrencia-e-mercado.md §0 item 9).

## Operating Context

- **Janela do app:** uma só janela do Warden por vez. Abrir o app de novo traz para a frente a janela já aberta (SPEC §7 regra 7).
- **Dois níveis de navegação:** o nível do app (Meus packs, Criar pack, Abrir ou importar…, Configurações) e o nível do pack aberto. O pack tem cabeçalho fixo com **▶ Testar ▾** e um menu de 6 seções: Mods, Configs, Problemas, ✦ Diagnóstico com IA, Histórico e Exportar. Não há abas em lugar nenhum (SPEC §5; ADR-0026; ESTRUTURA §1).
- **O jogo roda fora do Warden:** o Minecraft abre na sua própria janela, numa instância de teste separada, enquanto o Warden mostra o console, a memória e os tempos ao vivo (SPEC §3.2; SPEC T13).
- **Tarefas longas e só quando o dono pede:** servidor local, busca do culpado (20 a 40 minutos ou mais num pack grande) e perfil de desempenho, sempre com pausar e cancelar. O andamento fica no indicador de Tarefas do rodapé (SPEC §7 regra 10; SPEC §10 D23; SPEC §5).
- **Funciona sem internet:** o que não depende de rede continua funcionando. O que depende mostra "Sem conexão com a internet" com **Tentar de novo** e nunca carrega para sempre (SPEC §7 regra 4).
- **Serviços externos:** Modrinth e CurseForge (busca combinada), Mojang, Adoptium, Maven dos loaders, GitHub (publicação) e, com consentimento, Gemini, OpenAI ou Claude (SPEC §7 regra 8; SPEC §10 D11 e D40).
- **Arquivos no disco:** o pack fica em `Documentos\Warden\<pack>\` e é um repositório git. A instância de teste fica nos dados do app (SPEC §3.1 e §3.2).
- **Entrega aos jogadores:** um repositório no GitHub por pack, público recomendado; Release com changelog; link do `pack.toml`; instância pronta para o Prism (SPEC §10 D2, D14 e D34).

## Capabilities and Constraints

- **App DESKTOP, Windows primeiro, em janela nativa.** É Tauri 2 com interface React/TypeScript no WebView2 e instalador `.exe` (NSIS) sem assinatura (AGENTS.md; README.md; apps/desktop/src-tauri/tauri.conf.json; SPEC §10 D4). Também compila em Linux para a CI; macOS e pacote oficial para Linux ficam fora (SPEC §9; ADR-0003).
- **Tem que parecer um app, não uma página web.** A barra do app (ou o cabeçalho do pack) e o rodapé ficam fixos e só o conteúdo rola. A página nunca rola na horizontal e as tabelas rolam dentro do próprio contêiner (HANDOFF §5; apps/desktop/src/styles/warden.css). A organização é de ferramenta: nada de hero, de botão "Jogar" gigante ou de arte de mundo atrás do conteúdo (DESIGN-SYSTEM §8.2). Registrado também por pedido do orquestrador nesta tarefa.
- **Tamanho da janela:** a referência de projeto é 1280 e 1024 px de largura, e o app também é pensado para tela cheia em 1920×1080, com a coluna de conteúdo centralizada (HANDOFF §5; SPEC §10 D38). **Largura, decisão do dono (19/10/2026):** depende da parte da tela. Listas, editores, o chat da IA e a busca de mods usam a janela toda; textos longos e formulários ficam em coluna com largura de leitura. A janela mínima ainda difere entre os documentos: a janela mínima difere entre os documentos. O HANDOFF §5 diz 1024×640, e o `tauri.conf.json` tem `minWidth` 960 e `minHeight` 600.
- **Camadas, não navegação nova:** os detalhes abrem em painel lateral e as confirmações em diálogo. A página de descoberta abre em tela cheia dentro do pack, com o menu recolhido. Criar pack e Abrir ou importar… são diálogos por cima de Meus packs (SPEC §5; SPEC §10 D38).
- **Regras para o app inteiro:**
  - a pasta do pack só recebe conteúdo do pack;
  - toda ação que apaga ou substitui algo pede confirmação, cria um ponto de segurança e pode ser desfeita;
  - todo erro diz o que aconteceu e o que fazer, com **Detalhes técnicos** e **Copiar**;
  - não há botão morto: o que não funciona fica desabilitado, com uma dica dizendo por quê (SPEC §7 regras 1, 2, 3 e 5).
- **IA:** Gemini (padrão, com plano gratuito), OpenAI ou Claude, com a chave do próprio usuário. O consentimento é pedido uma vez por conversa, cada envio fica visível ("Enviado à IA") e nada muda sem **Aplicar** (SPEC §10 D17 e D40; ADR-0030; ADR-0060).
- **Launcher:** só perfil offline, sem login e sem multiplayer online, com o aviso "não oficial" visível. Motor portablemc atrás de `LauncherEngine` (ADR-0010; AGENTS.md; SPEC T01).
- **Segredos:** as chaves ficam no cofre do Windows ou, se o usuário preferir, num `.env` na pasta de dados do app, com aviso. Nunca ficam no estado da interface nem no `localStorage` (SPEC §10 D12; QUALITY §9 itens 1 e 3).
- **Terminologia obrigatória:** o glossário da QUALITY §8.2 vale para toda a interface. Exemplos: Testar (nunca Jogar), Configs × Configurações, Salvar versão × Publicar versão, versão final, link do pack, instância de teste, travou/travamento, saúde do pack, Problemas (QUALITY §8.2; SPEC §2).
- **Desempenho percebido:** o app abre até a lista em menos de 3 s, um pack de 300 mods abre em menos de 1 s e a interface nunca congela (SPEC §8).
- **Fora do escopo:** login e contas, temas além do Deep Dark, outros idiomas, publicar direto no Modrinth ou na CurseForge, editor de quests, atualização automática do Warden e outros itens (SPEC §9).
- **Decisões pendentes do dono:** antivírus pela AMSI, VirusTotal, lista de mods para os jogadores, migração de versão do Minecraft e outros pontos listados em SPEC §10, "Decisões pendentes do dono".

## Brand Commitments

- **Nome:** Warden. O identificador do app é `dev.kriticales.warden` e o publisher é "Kriticales" (apps/desktop/src-tauri/tauri.conf.json).
- **Visual "Deep Dark", obrigatório:** cores do mob Warden, fontes pixel e menus com jeito de Minecraft (AGENTS.md; DESIGN-SYSTEM §1). Há um tema só, escuro, e temas além do Deep Dark estão fora do escopo (SPEC §9; HANDOFF §1). A linguagem visual do protótipo D1 foi aprovada pelo dono; a organização do D1 foi reprovada (ESTRUTURA §9; DESIGN-SYSTEM §1).
- **Nada com "cara de IA":** sem gradiente roxo, glassmorphism, emojis ou texto de marketing, e sem grade de cartões iguais (AGENTS.md; ESTRUTURA §10.3; DESIGN-SYSTEM §8.2).
- **✦ (sparkles) marca o que é IA e só isso,** nunca como decoração (QUALITY §8.1 regra 10; ADR-0026).
- **Sem abas** em nenhuma tela; mudar isso exige nova decisão do dono (ADR-0026; ESTRUTURA §1 regra 3).
- **Nada da Mojang ou da Microsoft:** nenhuma textura, logo, personagem, fonte ou asset delas. Warden, sculk e Deep Dark entram só como inspiração (DESIGN-SYSTEM §9; QUALITY §9 item 9).
- **Aviso legal obrigatório:** "NÃO É UM PRODUTO OFICIAL DO MINECRAFT…", na primeira execução e em Sobre (SPEC T01; ADR-0010).
- **Fontes embutidas no app,** nada baixado da internet (decisão do dono registrada no HANDOFF §1).
- **Voz:**
  - português do Brasil, tratando o usuário por "você", em frases curtas e sentence case;
  - sem exclamação, sem slogans, sem excesso de travessões e sem títulos óbvios;
  - botões com verbo no infinitivo que diz o que acontece, nunca "OK" nem "Sim/Não";
  - português europeu proibido;
  - tom de ferramenta de trabalho, sem ser condescendente (QUALITY §8.1; ESTRUTURA §1 regra 8; DESIGN-SYSTEM §6; docs/research/08-concorrencia-e-mercado.md §4, V12).
- **Honestidade nos textos:** a interface diz o que verifica e o que não verifica, por exemplo "não é um antivírus" (SPEC §1 princípio 8).

## Evidence on Hand

- **Protótipo final aprovado:** `design/prototipo-final/index.html`, com 94 telas e diálogos, também publicado como Artifact (DESIGN-SYSTEM, cabeçalho).
- **Galeria de componentes:** `design/system/index.html` (DESIGN-SYSTEM, cabeçalho).
- **Rascunho de estrutura em cinza:** `design/estrutura/index.html` (ESTRUTURA, cabeçalho).
- **Marca e ícones em pixel art própria:** `apps/desktop/src-tauri/icons/warden.svg`, `icon.ico`, PNGs e `instalador-lateral.bmp`; a marca é um bloco com duas antenas e núcleo de alma (DESIGN-SYSTEM §3.7).
- **Textos revisados da interface:** `apps/desktop/src/i18n/pt-BR/`; os textos do protótipo são a base (HANDOFF §8; QUALITY §8.1).
- **Notas de versão reais:** docs/versoes/1.0.0.md e 1.0.1.md. Há também um roteiro de aceite em docs/ROTEIRO-DE-ACEITE.md.
- **Pesquisa de mercado e concorrência:** com dados verificados (número de modpacks, dores dos criadores) em docs/research/08-concorrencia-e-mercado.md.
- **O que não existe e não se deve inventar:**
  - depoimentos, clientes, preços, licença pública ou métricas de uso. O app é privado e sem telemetria (ADR-0004; SPEC §7 regra 8);
  - os dados, versões, logs e datas do protótipo são fictícios, e os nomes de mods reais aparecem só como exemplo (HANDOFF §9; DESIGN-SYSTEM §9).

## Product Principles

1. **O pack é sagrado.** A pasta do pack só contém o que é do pack, e o projeto fica separado do teste: o que muda durante o jogo só volta para o pack depois de revisado (SPEC §1 princípios 1 e 2).
2. **Funciona de verdade e é honesto sobre o que verifica.** Só está pronto o que foi provado contra o packwiz e o jogo reais, e a interface não promete mais do que confere (SPEC §1 princípios 3 e 8; QUALITY §1).
3. **Sem surpresas.** Tudo o que apaga ou substitui pede confirmação e pode ser desfeito; a IA só propõe, e o dono aplica (SPEC §1 princípio 4; SPEC §10 D17).
4. **Simples para um leigo sem esconder o poder.** O dia a dia é o caminho mais curto, e o que é raro fica em menus ▾ e ⋯ ou em "Mais opções", com no máximo um nível de navegação à vista (ESTRUTURA §1 regras 3, 5 e 7; SPEC §1 princípio 5).
5. **Privado, e o pesado só quando o dono pede.** Nada sai do computador sem pedido do dono, e as tarefas pesadas nunca rodam sozinhas (SPEC §1 princípios 6 e 7).

## Accessibility & Inclusion

- **Padrão exigido:** WCAG 2.1 AA. Toda ação funciona pelo teclado, o foco fica visível e há texto alternativo nos ícones que têm função (SPEC §7 regra 6; DESIGN-SYSTEM §7).
- **Estado nunca só na cor:** todo estado tem ícone e palavra, e as fontes dos mods têm marcadores de formas diferentes (DESIGN-SYSTEM §1 princípio 4; HANDOFF §7).
- **Movimento reduzido:** o app respeita `prefers-reduced-motion` e tem a opção "Menos movimento" nas Configurações (DESIGN-SYSTEM §3.8; HANDOFF §6).
- **Estrutura para leitor de tela:** landmarks, foco no título a cada troca de tela, anúncios `aria-live` nas etapas do teste e "Pular para o conteúdo" (HANDOFF §7).
- **Ainda não verificado:** leitor de tela real (NVDA, Narrador), o próprio WebView2 e zoom de 150% e 200% na escala do Windows (DESIGN-SYSTEM §7).
- **Idioma único:** pt-BR, com linguagem simples para quem não é técnico (ADR-0016; QUALITY §8.1).
