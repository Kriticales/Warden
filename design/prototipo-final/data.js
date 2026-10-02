/* Dados fictícios do protótipo final. Uma história só, coerente entre as telas:
   o pack "Vale Sereno" (Minecraft 1.20.1, Forge 47.3.0) está na versão 1.4.2, tem 5 alterações
   não salvas, 4 problemas (2 erros, 2 avisos), saúde 33 (Crítico), um travamento hoje por falta
   do Balm e um travamento repetido sem causa (Ticking entity) que a busca do culpado resolve:
   Epic Fight junto com o Supplementaries.
   Nomes de mods reais aparecem só como exemplo; versões, logs, issues, modpacks e datas são inventados. */
window.DATA = (function () {
  const PACK = { name: "Vale Sereno", mc: "1.20.1", loader: "Forge 47.3.0", loaderShort: "Forge", version: "1.4.2", author: "Kriticales", folder: "Documentos\\Warden\\vale-sereno", unsaved: 5 };
  const PACK_LINK = "https://raw.githubusercontent.com/kriticales/vale-sereno/main/pack.toml";

  const PACKS = [
    { name: "Vale Sereno", mc: "1.20.1", loader: "Forge 47.3.0", version: "1.4.2", test: ["danger", "travou · hoje, 14:40"], health: 33, unsaved: 5, when: "hoje, 14:40", go: "mods" },
    { name: "Técnico Clássico", mc: "1.7.10", loader: "Forge 10.13.4.1614", version: "2.0.1", test: ["danger", "travou · ontem, 22:10"], health: 61, unsaved: 0, when: "ontem" },
    { name: "Leve e Bonito", mc: "1.21.1", loader: "Fabric 0.16.5", version: "0.3.0", test: ["muted", "nunca testado"], health: 88, unsaved: 2, when: "12/09/2026" },
    { name: "Sky Factory do Zero", mc: "1.12.2", loader: "Forge 14.23.5.2860", version: "1.0.0", test: ["ok", "abriu normalmente · 03/09"], health: 97, unsaved: 0, when: "03/09/2026" },
  ];

  // name, desc, ver, src, side, update(ok|available|na), updateTo, kind, flags(keys)
  const MODS = [
    ["Create", "Engrenagens, eixos e máquinas que se mexem.", "0.5.1.j", "modrinth", "both", "available", "0.5.1.k"],
    ["Just Enough Items (JEI)", "Mostra todos os itens e receitas do jogo.", "15.20.0.106", "curseforge", "both", "available", "15.20.0.112"],
    ["Embeddium", "Deixa o jogo mais leve e com mais quadros por segundo.", "0.3.31", "modrinth", "client", "ok"],
    ["Rubidium", "Otimização de desenho dos blocos (antigo).", "0.7.1", "local", "client", "na", null, "error", ["Erro"]],
    ["Waystones", "Pedras de teleporte entre lugares que você já visitou.", "14.1.6", "modrinth", "both", "ok", null, "error", ["Erro"]],
    ["Xaero's Minimap", "Minimapa no canto da tela.", "24.6.1", "curseforge", "both", "available", "24.6.2", "warn", ["Aviso"]],
    ["Farmer's Delight", "Culinária, facas e plantações novas.", "1.2.6", "modrinth", "both", "available", "1.2.7"],
    ["Sophisticated Backpacks", "Mochilas com melhorias, filtros e muito espaço.", "3.20.17", "modrinth", "both", "ok", null, "new", ["Não salvo"]],
    ["Sophisticated Core", "Biblioteca usada pelas mochilas Sophisticated.", "0.6.26", "modrinth", "both", "ok", null, "new", ["Não salvo"]],
    ["Supplementaries", "Jarros, placas, cata-ventos e outros detalhes.", "2.8.17", "modrinth", "both", "available", "2.8.21", "warn", ["Aviso"]],
    ["Jade", "Mostra o que você está olhando, no topo da tela.", "11.12.3", "modrinth", "client", "ok"],
    ["Epic Fight", "Combate com animações e golpes novos.", "20.9.4", "curseforge", "both", "ok", null, "warn", ["Aviso", "Download manual"]],
    ["Xaero's World Map", "Mapa do mundo em tela cheia.", "1.39.0", "curseforge", "both", "ok", null, null, ["Download manual"]],
    ["Clumps", "Junta orbes de experiência para o jogo não pesar.", "12.0.0.4", "curseforge", "both", "ok"],
  ];
  const MODS_TOTAL = 124;
  const RESOURCEPACKS = [
    ["Fresh Animations", "Animações novas para os mobs.", "1.9.2", "modrinth", "client", "ok"],
    ["Stay True", "Texturas de blocos com mais variedade.", "1.3.1", "curseforge", "client", "ok"],
    ["Vale Sereno UI", "Ajustes visuais do próprio pack.", "1.0.0", "local", "client", "na"],
  ];
  const SHADERS = [["Complementary Reimagined", "Iluminação e água mais bonitas.", "r5.4", "modrinth", "client", "ok"]];

  const SEARCH = [
    // nome, autor, resumo, downloads, atualizado, fontes, jáNoPack, manual
    ["Sophisticated Backpacks", "P3pp3rF1y", "Mochilas com melhorias, filtros e muito espaço.", "48 mi", "há 6 dias", "both", false, false],
    ["Traveler's Backpack", "Tiviacz1337", "Mochilas temáticas com tanques de líquido.", "30 mi", "há 1 mês", "both", true, false],
    ["Sophisticated Storage", "P3pp3rF1y", "Baús e barris com melhorias.", "21 mi", "há 6 dias", "both", false, false],
    ["Backpacked", "MrCrayfish", "Uma mochila simples que aparece nas costas.", "9 mi", "há 2 meses", "curseforge", false, true],
    ["Simple Backpack", "Zerkil", "Mochila de um slot, sem interface.", "310 mil", "há 3 meses", "modrinth", false, false],
    ["Packed Up", "Ender", "Mochilas que combinam com o estilo do jogo.", "1,2 mi", "há 4 meses", "modrinth", false, false],
  ];

  const CONFIG_FILE = [
    "#.", "#Configurações gerais do Create", "[worldgen]", "\t#Desliga a geração de minérios do Create", "\tdisableWorldGen = false", "",
    "[kinetics]", "\t#Velocidade máxima de rotação", "\t#Range: > 64", "\tmaxRotationSpeed = 256", "\t#Multiplicador de stress", "\t#Range: 0.0 ~ 10.0",
    "\tstressMultiplier = 1.0", "", "[fluids]", "\t#Quantos blocos o cano leva o líquido", "\t#Range: 1 ~ 256", "\tmechanicalPumpRange = 16",
  ];

  const LOG = [
    ["warden", "14:20:01", "Warden", "Abrindo Minecraft 1.20.1 com Forge 47.3.0 · Java 17.0.12 · 6 GB · jogador Jogador"],
    ["info", "14:20:04", "modlauncher/Launcher", "ModLauncher running: args [--launchTarget, forgeclient, --fml.mcVersion, 1.20.1]"],
    ["info", "14:20:06", "fml.loading/FMLLoader", "Java 17.0.12 by Eclipse Adoptium; OS Windows 11 arch amd64"],
    ["info", "14:20:11", "fml.loading/moddiscovery", "Found 128 mods"],
    ["warn", "14:20:19", "mixin", "Reference map 'xaerominimap.refmap.json' could not be read. If this is a development environment you can ignore this message"],
    ["info", "14:20:27", "minecraft/Minecraft", "Setting user: Jogador"],
    ["info", "14:20:38", "minecraft/Minecraft", "Reloading ResourceManager: vanilla, mod_resources, Fresh Animations, Stay True"],
    ["warn", "14:20:52", "minecraft/ModelBakery", "Unable to load model: 'supplementaries:jar_boat' referenced from: supplementaries:jar_boat#inventory"],
    ["info", "14:21:02", "minecraft/MinecraftServer", "Preparing level \"Mundo de teste\""],
    ["info", "14:21:09", "minecraft/MinecraftServer", "Done (6.912s)! For help, type \"help\""],
    ["info", "14:21:10", "minecraft/IntegratedServer", "Saving chunks for level 'ServerLevel[Mundo de teste]'/minecraft:overworld"],
    ["info", "14:24:41", "minecraft/ChatComponent", "[System] [CHAT] Jogo salvo"],
  ];
  const CRASH_LOG = [
    ["warden", "14:40:02", "Warden", "Abrindo Minecraft 1.20.1 com Forge 47.3.0 · Java 17.0.12 · 6 GB"],
    ["info", "14:40:09", "fml.loading/moddiscovery", "Found 126 mods"],
    ["error", "14:40:31", "fml/ModLoader", "Missing or unsupported mandatory dependencies:"],
    ["error", "14:40:31", "fml/ModLoader", "\tMod ID: 'balm', Requested by: 'waystones', Expected range: '[7.3.0,)', Actual version: '[MISSING]'"],
    ["warden", "14:40:50", "Warden", "O jogo fechou com o código 1 depois de 48 segundos."],
  ];

  const VERSIONS = [
    { v: "1.4.2", date: "28/09/2026", summary: "2 mods atualizados, 1 config alterada", kind: "final" },
    { v: "1.4.1", date: "24/09/2026", summary: "Ajuste de configs do Create", kind: "saved" },
    { v: "1.4.0", date: "20/09/2026", summary: "3 mods adicionados, 1 shader", kind: "published" },
    { v: "1.3.0", date: "02/09/2026", summary: "Create atualizado, 5 configs alteradas", kind: "published" },
    { v: "1.2.0", date: "18/08/2026", summary: "Primeira versão com o Create", kind: "published" },
  ];

  const JAVAS = [
    ["25", "nenhum pack ainda", "O mais novo. Usado pelo Minecraft 26.1 e seguintes."],
    ["21", "Leve e Bonito (1.21.1)", "O Minecraft 1.20.5 a 1.21 foi feito para o Java 21. Com o 25 não há garantia de que abre."],
    ["17", "Vale Sereno (1.20.1)", "O Minecraft 1.18 a 1.20.4 foi feito para o Java 17. Forge e mods dessa época não são garantidos em Java mais novo."],
    ["8", "Técnico Clássico (1.7.10), Sky Factory do Zero (1.12.2)", "O Forge 1.7.10 e 1.12.2 só abre no Java 8. A partir do Java 9 ele trava ao iniciar."],
  ];

  const MC_VERSIONS = ["1.21.9", "1.21.8", "1.21.5", "1.21.1", "1.20.6", "1.20.4", "1.20.1", "1.19.2", "1.18.2", "1.16.5", "1.12.2", "1.7.10"];

  // ---------- D4: funções avançadas ----------
  // Saúde do pack: [pontos, categoria, texto]
  const HEALTH = 33;
  const HEALTH_LOSSES = [
    ["−30", "Problemas encontrados:", "2 erros (Balm faltando; Embeddium e Rubidium juntos)."],
    ["−3", "Problemas encontrados:", "1 aviso (Xaero's Minimap com o lado errado)."],
    ["−20", "Último teste:", "travou hoje às 14:40."],
    ["−6", "Travamentos recentes:", "2 causas diferentes nas últimas 10 vezes que você testou."],
    ["−4", "Mods que alteram o mesmo ponto do jogo:", "Epic Fight e Supplementaries (risco alto)."],
    ["−4", "Download pelos jogadores:", "2 mods da CurseForge precisam ser baixados à mão."],
  ];
  // Travamentos agrupados pela causa
  const CRASHES = [
    { title: "Falta o mod Balm, exigido pelo Waystones", times: 1, last: "hoje, 14:40", versions: "1.4.2 com alterações", state: ["ok", "Causa encontrada"] },
    { title: "NullPointerException ao atualizar uma entidade (Ticking entity)", times: 3, last: "30/09, 21:14", versions: "1.4.1 a 1.4.2", state: ["danger", "Causa não encontrada"] },
    { title: "O jogo ficou sem memória (OutOfMemoryError)", times: 1, last: "22/09", versions: "1.4.0", state: ["muted", "Não voltou a acontecer desde a 1.4.1"] },
  ];
  // Busca do culpado: rodada, mods ligados, resultado, tempo
  const ROUNDS = [
    { n: 0, mods: 118, result: "same", what: "Pack inteiro", time: "1 min 52 s", note: "Travou 3 de 3 vezes, com a mesma mensagem" },
    { n: 1, mods: 0, result: "pass", what: "Só o Forge, sem mods", time: "38 s", note: "Abriu e entrou no mundo" },
    { n: 2, mods: 59, result: "pass", what: "Primeiros 59 mods", time: "1 min 4 s", note: "Ficou 20 s no mundo sem travar" },
    { n: 3, mods: 88, result: "same", what: "Primeiros 88 mods", time: "1 min 31 s", note: "Travou com a mesma mensagem" },
    { n: 4, mods: 73, result: "same", what: "Primeiros 73 mods", time: "1 min 18 s", note: "Travou com a mesma mensagem" },
    { n: 5, mods: 66, result: "pass", what: "Primeiros 66 mods", time: "1 min 9 s", note: "Ficou 20 s no mundo sem travar" },
    { n: 6, mods: 69, result: "now", what: "Primeiros 69 mods", time: "48 s até agora", note: "Abrindo o jogo" },
  ];
  // Console agrupado por mod
  const GROUPS = [
    { mod: "Supplementaries", errors: 0, warns: 1290, total: 1290, open: true, lines: [[1284, "warn", "Unable to load model: 'supplementaries:{id}' referenced from: supplementaries:{id}#inventory", true], [6, "warn", "Missing texture for {id} at {n}", true]] },
    { mod: "Epic Fight", errors: 1, warns: 2, total: 51, open: true, lines: [[2, "warn", "Animation 'epicfight:biped/living/{id}' has no keyframes"]], stack: { title: "java.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.getX()\"", frames: ["at java.base/java.util.Objects.requireNonNull(Objects.java:259)", "at net.minecraft.world.entity.LivingEntity.travel(LivingEntity.java:2108)", "at yesman.epicfight.world.capabilities.entitypatch.LivingEntityPatch.onTravel(LivingEntityPatch.java:412) ~[epicfight-forge-20.9.4-1.20.1.jar%23154!/:20.9.4]", "at net.minecraft.world.entity.LivingEntity.aiStep(LivingEntity.java:2541)"], hidden: 44, modFrame: 2 } },
    { mod: "Xaero's Minimap", errors: 0, warns: 1, total: 12, lines: [[1, "warn", "Reference map 'xaerominimap.refmap.json' could not be read. If this is a development environment you can ignore this message", true]] },
    { mod: "Jogo e loader", errors: 0, warns: 3, total: 412, lines: [[1, "info", "Found 128 mods"], [1, "info", "Done (6.912s)! For help, type \"help\""]] },
    { mod: "Desconhecido", errors: 0, warns: 0, total: 7, lines: [[7, "info", "Loading {n} chunk(s) for dimension {id}"]] },
  ];
  // Descoberta
  const POPULAR = [
    ["Biomes O' Plenty", "Forstride", "Mais de 50 biomas novos.", "70 mi", "há 3 dias", "both", false],
    ["Storage Drawers", "Texelsaur", "Gavetas que guardam muitos itens de um tipo.", "64 mi", "há 2 semanas", "both", false],
    ["Alex's Mobs", "sbom_xela", "Mais de 80 animais e criaturas novas.", "59 mi", "há 1 mês", "curseforge", false],
    ["Macaw's Bridges", "sketch_macaw", "Pontes de madeira e pedra em vários estilos.", "41 mi", "há 3 semanas", "both", false],
    ["Comforts", "TheIllusiveC4", "Sacos de dormir e redes para passar a noite.", "21 mi", "há 2 meses", "modrinth", false],
  ];
  const UPDATED = [
    ["Sophisticated Backpacks", "P3pp3rF1y", "Mochilas com melhorias, filtros e muito espaço.", "48 mi", "hoje", "both", false],
    ["Supplementaries", "MehVahdJukaar", "Jarros, placas, cata-ventos e outros detalhes.", "39 mi", "ontem", "both", true],
    ["Create Slice & Dice", "possible_triangle", "Máquinas do Create para a cozinha do Farmer's Delight.", "6 mi", "há 2 dias", "modrinth", true],
  ];
  const CATEGORIES = [["Aventura", 812], ["Tecnologia", 640], ["Magia", 521], ["Armazenamento", 233], ["Comida", 198], ["Decoração", 466], ["Geração de mundo", 374], ["Otimização", 151], ["Utilidades", 902], ["Bibliotecas", 1104]];
  const MODPACKS = [
    ["Cozy Create", "harborlight", "Create com fazendas, cozinha e casas aconchegantes.", "1,2 mi", "há 5 dias", "modrinth", "168 mods"],
    ["Vales do Norte", "ana_mods", "Exploração com biomas novos e vilas maiores.", "640 mil", "há 3 semanas", "curseforge", "212 mods"],
    ["Engenho Simples", "pedro.mc", "Tecnologia leve para servidor entre amigos.", "88 mil", "há 2 meses", "both", "94 mods"],
  ];
  const MODPACK_MODS = [
    ["Create Steam 'n' Rails", "Trens, trilhos e estações para o Create.", "selected", "modrinth"],
    ["Create: Connected", "Peças a mais para as máquinas do Create.", "selected", "modrinth"],
    ["Storage Drawers", "Gavetas que guardam muitos itens de um tipo.", "selected", "both"],
    ["Handcrafted", "Móveis: mesas, cadeiras, estantes.", "selected", "both"],
    ["Comforts", "Sacos de dormir e redes.", "selected", "modrinth"],
    ["Create", "Engrenagens, eixos e máquinas que se mexem.", "inpack", "both"],
    ["Farmer's Delight", "Culinária, facas e plantações novas.", "inpack", "both"],
    ["Cozy Create Tweaks", "Ajustes feitos pelo autor do modpack.", "external", null, "Arquivo fora das lojas: o Warden não copia jars de pacotes de terceiros"],
    ["Better Clouds", "Nuvens volumétricas.", "noversion", "modrinth", "Sem versão para Forge 1.20.1"],
    ["Dynamic Trees", "Árvores que crescem galho por galho.", "normal", "curseforge"],
  ];
  // Configs: busca em todas as configs por "spawn"
  const CONFIG_HITS = [
    { file: "config/waystones-common.toml", hits: [{ label: "Gerar pedras de teleporte nas vilas", key: "spawnInVillages", value: "true", ctx: "#Se as vilas geram uma pedra de teleporte. Padrão: true" }, { label: "Chance por chunk", key: "worldGenFrequency", value: "25", changed: true, ctx: "#Quanto maior, mais raras. Valores pequenos geram muitas pedras perto do <mark>spawn</mark>." }] },
    { file: "config/create-common.toml", hits: [{ key: "disableWorldGen", value: "false", ctx: "#Desliga a geração de minérios do Create, inclusive perto do <mark>spawn</mark>" }] },
    { file: "config/supplementaries-common.toml", hits: [{ label: "Pássaros aparecem nos campos", key: "bird_spawn", value: "true", ctx: "" }, { key: "firefly_spawn_chance", value: "0.4", changed: true, ctx: "" }, { key: "jar_spawn_rate", value: "0.05", ctx: "" }] },
    { file: "defaultconfigs/epicfight-server.toml", hits: [{ key: "spawnEpicFightPlayerSkin", value: "false", ctx: "" }] },
    { file: "options.txt", hits: [{ label: "Distância de simulação", key: "simulationDistance", value: "8", ctx: "Opção do jogo · controla até onde os mobs aparecem (<mark>spawn</mark>)" }] },
  ];
  // Script KubeJS de exemplo (linha 16 com o ID errado)
  const SCRIPT = [
    "// Receitas do Vale Sereno",
    "ServerEvents.recipes(event => {",
    "  // Mochila com couro e baú",
    "  event.shaped('sophisticatedbackpacks:backpack', [",
    "    'SLS',",
    "    'LCL',",
    "    'LLL'",
    "  ], {",
    "    S: 'minecraft:string',",
    "    L: 'minecraft:leather',",
    "    C: 'minecraft:chest'",
    "  })",
    "",
    "  // Latão no misturador do Create",
    "  event.recipes.create.mixing('create:brass_ingot', [",
    "    'minecraft:copper_ingott',",
    "    'create:zinc_ingot'",
    "  ]).heated()",
    "})",
  ];
  // Log do servidor local
  const SERVER_LOG = [
    ["warden", "15:10:02", "Warden", "Abrindo o servidor do Vale Sereno · Forge 47.3.0 · Java 17 · 4 GB · só em 127.0.0.1, porta 25566"],
    ["info", "15:10:05", "minecraft/DedicatedServer", "Starting minecraft server version 1.20.1"],
    ["info", "15:10:05", "minecraft/DedicatedServer", "Loading properties"],
    ["info", "15:10:09", "fml.loading/moddiscovery", "Found 120 mods"],
    ["warn", "15:10:21", "minecraft/DedicatedServer", "**** SERVER IS RUNNING IN OFFLINE/INSECURE MODE!"],
    ["info", "15:10:24", "minecraft/DedicatedServer", "Preparing level \"world\""],
    ["info", "15:10:31", "minecraft/DedicatedServer", "Done (6.912s)! For help, type \"help\""],
    ["info", "15:11:02", "minecraft/MinecraftServer", "Jogador joined the game"],
  ];
  const LOAD_TIMES = [["Create", "9,8 s"], ["Epic Fight", "6,1 s"], ["Supplementaries", "4,4 s"], ["Just Enough Items (JEI)", "3,9 s"], ["Waystones", "1,2 s"]];
  const MEM_SERIES = [2.4, 2.6, 2.5, 2.8, 2.7, 2.9, 3.0, 2.6, 2.8, 3.1, 2.9, 3.0, 3.2, 2.8, 3.0, 3.1, 3.0, 2.9, 3.1, 3.1];
  const MEM_SERIES_HIGH = [4.8, 5.0, 5.2, 5.3, 5.4, 5.5, 5.5, 5.6, 5.6, 5.7, 5.6, 5.7, 5.8, 5.7, 5.8, 5.8, 5.9, 5.8, 5.9, 5.9];
  const PROFILES = [["padrao", "Padrão", "6 GB · Java automático"], ["fraco", "PC fraco", "4 GB"], ["shaders", "Shaders", "8 GB"]];

  return { PACK, PACK_LINK, PACKS, MODS, MODS_TOTAL, RESOURCEPACKS, SHADERS, SEARCH, CONFIG_FILE, LOG, CRASH_LOG, VERSIONS, JAVAS, MC_VERSIONS,
    HEALTH, HEALTH_LOSSES, CRASHES, ROUNDS, GROUPS, POPULAR, UPDATED, CATEGORIES, MODPACKS, MODPACK_MODS, CONFIG_HITS, SCRIPT, SERVER_LOG, LOAD_TIMES, MEM_SERIES, MEM_SERIES_HIGH, PROFILES };
})();
