/* Dados fictícios do protótipo final. Uma história só, coerente entre as telas:
   o pack "Vale Sereno" (Minecraft 1.20.1, Forge 47.3.0) está na versão 1.4.2, tem 5 alterações
   não salvas, 3 problemas (2 erros, 1 aviso) e um travamento recente por falta do Balm.
   Nomes de mods reais aparecem só como exemplo; versões, logs e datas são inventados. */
window.DATA = (function () {
  const PACK = { name: "Vale Sereno", mc: "1.20.1", loader: "Forge 47.3.0", loaderShort: "Forge", version: "1.4.2", author: "Kriticales", folder: "Documentos\\Warden\\vale-sereno", unsaved: 5 };
  const PACK_LINK = "https://raw.githubusercontent.com/kriticales/vale-sereno/main/pack.toml";

  const PACKS = [
    { name: "Vale Sereno", mc: "1.20.1", loader: "Forge 47.3.0", version: "1.4.2", test: ["ok", "abriu normalmente · hoje, 14:32"], unsaved: 5, when: "hoje, 14:40", go: "mods" },
    { name: "Técnico Clássico", mc: "1.7.10", loader: "Forge 10.13.4.1614", version: "2.0.1", test: ["danger", "travou · ontem, 22:10"], unsaved: 0, when: "ontem" },
    { name: "Leve e Bonito", mc: "1.21.1", loader: "Fabric 0.16.5", version: "0.3.0", test: ["muted", "nunca testado"], unsaved: 2, when: "12/09/2026" },
    { name: "Sky Factory do Zero", mc: "1.12.2", loader: "Forge 14.23.5.2860", version: "1.0.0", test: ["ok", "abriu normalmente · 03/09"], unsaved: 0, when: "03/09/2026" },
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
    ["Supplementaries", "Jarros, placas, cata-ventos e outros detalhes.", "2.8.17", "modrinth", "both", "ok"],
    ["Jade", "Mostra o que você está olhando, no topo da tela.", "11.12.3", "modrinth", "client", "ok"],
    ["Epic Fight", "Combate com animações e golpes novos.", "20.9.4", "curseforge", "both", "ok", null, null, ["Download manual"]],
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

  return { PACK, PACK_LINK, PACKS, MODS, MODS_TOTAL, RESOURCEPACKS, SHADERS, SEARCH, CONFIG_FILE, LOG, CRASH_LOG, VERSIONS, JAVAS, MC_VERSIONS };
})();
