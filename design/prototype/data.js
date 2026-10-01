// Dados fictícios do protótipo. Nomes de mods são reais (como exemplo), mas
// versões, datas, problemas e logs foram inventados para contar uma história
// coerente entre as telas. Nada aqui é afirmação sobre os mods reais.
window.DATA = (function () {
  const packs = [
    {
      id: "vale", name: "Vale das Engrenagens", mc: "1.21.1", loader: "NeoForge", loaderVersion: "21.1.209",
      mods: 24, version: "1.4.0", savedAgo: "salva há 2 dias", test: { kind: "warn", text: "Passou com avisos", when: "ontem, 21:14" },
      hue: 2,
    },
    {
      id: "ceu", name: "Céu de Pedra", mc: "1.20.1", loader: "Fabric", loaderVersion: "0.16.14",
      mods: 68, version: "0.9.2", savedAgo: "salva há 5 dias", test: { kind: "danger", text: "Travou", when: "há 3 dias" },
      hue: 5,
    },
    {
      id: "classica", name: "Engenharia Clássica", mc: "1.12.2", loader: "Forge", loaderVersion: "14.23.5.2860",
      mods: 187, version: "2.1.0", savedAgo: "salva há 3 semanas", test: { kind: "ok", text: "Passou", when: "há 3 semanas" },
      hue: 1,
    },
    {
      id: "raizes", name: "Raízes 1.7.10", mc: "1.7.10", loader: "Forge", loaderVersion: "10.13.4.1614",
      mods: 96, version: "0.3.0", savedAgo: "salva há 1 mês", test: { kind: "neutral", text: "Nunca testado", when: "" },
      hue: 4,
    },
    {
      id: "leve", name: "Puro e Leve", mc: "26.3", loader: "Fabric", loaderVersion: "0.17.3",
      mods: 31, version: null, savedAgo: "nenhuma versão salva", test: { kind: "ok", text: "Passou", when: "hoje, 10:02" },
      hue: 0,
    },
  ];

  // Lado: "cliente" | "servidor" | "ambos" | "?" (desconhecido)
  // issue: { kind: "danger"|"warn", text }
  const mods = [
    { id: "create", name: "Create", author: "Creators of Create", ver: "6.0.8", src: "modrinth", side: "ambos", changed: "Atualizado de 6.0.6 (não salvo)" },
    { id: "createaddition", name: "Create Crafts & Additions", author: "mrh0", ver: "1.3.2", upd: "1.3.4", src: "modrinth", side: "ambos" },
    { id: "sodium", name: "Sodium", author: "CaffeineMC", ver: "0.6.13", src: "modrinth", side: "cliente" },
    { id: "iris", name: "Iris Shaders", author: "coderbot, IMS", ver: "1.8.8", upd: "1.8.11", src: "modrinth", side: "cliente" },
    { id: "embeddium", name: "Embeddium", author: "embeddedt", ver: "1.0.15", src: "curseforge", side: "cliente", issue: { kind: "danger", text: "Conflita com Sodium" } },
    { id: "jei", name: "Just Enough Items (JEI)", author: "mezz", ver: "19.21.0.247", src: "curseforge", side: "ambos" },
    { id: "jade", name: "Jade", author: "Snownee", ver: "15.10.0", upd: "15.10.2", src: "modrinth", side: "ambos" },
    { id: "xaerominimap", name: "Xaero's Minimap", author: "xaero96", ver: "25.2.6", src: "modrinth", side: "cliente" },
    { id: "farmersdelight", name: "Farmer's Delight", author: "vectorwing", ver: "1.2.9", src: "modrinth", side: "ambos" },
    { id: "supplementaries", name: "Supplementaries", author: "MehVahdJukaar", ver: "3.1.36", src: "modrinth", side: "ambos" },
    { id: "moonlight", name: "Moonlight Lib", author: "MehVahdJukaar", ver: "2.17.12", src: "modrinth", side: "ambos", lib: true },
    { id: "waystones", name: "Waystones", author: "BlayTheNinth", ver: "21.1.12", src: "modrinth", side: "ambos", issue: { kind: "danger", text: "Falta o mod Balm" } },
    { id: "mousetweaks", name: "Mouse Tweaks", author: "YaLTeR", ver: "2.26.1", src: "modrinth", side: "cliente" },
    { id: "appleskin", name: "AppleSkin", author: "squeek502", ver: "3.0.6", src: "modrinth", side: "ambos" },
    { id: "clumps", name: "Clumps", author: "Jaredlll08", ver: "19.0.0.1", src: "curseforge", side: "ambos" },
    { id: "ferritecore", name: "FerriteCore", author: "malte0811", ver: "7.0.2", src: "modrinth", side: "ambos" },
    { id: "modernfix", name: "ModernFix", author: "embeddedt", ver: "5.20.2", src: "modrinth", side: "ambos" },
    { id: "entityculling", name: "Entity Culling", author: "tr7zw", ver: "1.7.4", src: "modrinth", side: "cliente" },
    { id: "sophisticatedbackpacks", name: "Sophisticated Backpacks", author: "P3pp3rF1y", ver: "3.23.6", upd: "3.23.9", src: "curseforge", side: "ambos" },
    { id: "sophisticatedcore", name: "Sophisticated Core", author: "P3pp3rF1y", ver: "1.2.31", src: "curseforge", side: "ambos", lib: true },
    { id: "storagedrawers", name: "Storage Drawers", author: "Texelsaur", ver: "13.8.5", src: "curseforge", side: "ambos" },
    { id: "comforts", name: "Comforts", author: "TheIllusiveC4", ver: "9.0.3", src: "modrinth", side: "ambos", changed: "Adicionado (não salvo)" },
    { id: "chatheads", name: "Chat Heads", author: "dzwdz", ver: "0.13.9", src: "modrinth", side: "cliente", changed: "Adicionado (não salvo)" },
    { id: "packutils", name: "Utilitários do Vale", author: "arquivo local", ver: "1.0.0", src: "local", side: "?", file: "vale-utils-1.0.0.jar", issue: { kind: "warn", text: "Lado não identificado" } },
  ];

  const resourcepacks = [
    { id: "freshanimations", name: "Fresh Animations", author: "FreshLX", ver: "1.9.4", src: "modrinth", side: "cliente", issue: { kind: "warn", text: "Precisa de 2 mods que não estão no pack" } },
    { id: "defaultdark", name: "Default Dark Mode", author: "nebulr", ver: "2025.3.0", src: "modrinth", side: "cliente" },
    { id: "texturasvale", name: "Texturas do Vale", author: "arquivo local", ver: "—", src: "local", side: "cliente", file: "Texturas do Vale.zip", size: "2,1 MB" },
  ];
  const shaders = [
    { id: "complementary", name: "Complementary Shaders – Reimagined", author: "EminGT", ver: "r5.5.1", src: "modrinth", side: "cliente" },
    { id: "bsl", name: "BSL Shaders", author: "capttatsu", ver: "10.0", src: "modrinth", side: "cliente" },
  ];

  // Versões do Minecraft (ordem do manifesto, mais nova primeiro)
  const mcVersions = [
    { group: "Mais recentes (numeração por ano)", items: ["26.3", "26.2", "26.1.2", "26.1.1", "26.1"] },
    { group: "1.21", items: ["1.21.11", "1.21.10", "1.21.8", "1.21.5", "1.21.4", "1.21.1", "1.21"] },
    { group: "1.20", items: ["1.20.6", "1.20.4", "1.20.2", "1.20.1"] },
    { group: "1.19 a 1.14", items: ["1.19.4", "1.19.2", "1.18.2", "1.17.1", "1.16.5", "1.15.2", "1.14.4"] },
    { group: "1.13 a 1.7.10", items: ["1.13.2", "1.12.2", "1.11.2", "1.10.2", "1.9.4", "1.8.9", "1.7.10"] },
    { group: "Anteriores à 1.7.10 (suporte limitado)", items: ["1.6.4", "1.5.2"] },
  ];

  const loaderVersions = {
    "Forge|1.21.1": [{ v: "52.1.3", rec: true }, { v: "52.1.0" }, { v: "52.0.40" }],
    "Forge|1.12.2": [{ v: "14.23.5.2860", rec: true }, { v: "14.23.5.2859" }, { v: "14.23.5.2851", bad: "Tem um problema conhecido na instalação" }],
    "Forge|1.7.10": [{ v: "10.13.4.1614", rec: true }, { v: "10.13.4.1558" }],
    "NeoForge|1.21.1": [{ v: "21.1.209", rec: true }, { v: "21.1.200" }, { v: "21.1.186" }],
    "NeoForge|26.3": [{ v: "26.3.0.14-beta", beta: true }],
    "Fabric|*": [{ v: "0.17.3", rec: true }, { v: "0.17.2" }, { v: "0.16.14" }],
  };

  const searchResults = [
    { id: "createaddition", name: "Create Crafts & Additions", author: "mrh0", desc: "Adiciona eletricidade ao Create: motores, alternadores, fios e baterias que conversam com energia de outros mods.", dl: "18,4 mi", updated: "12 set 2026", srcs: ["modrinth", "curseforge"], state: "inpack", stateText: "No pack (1.3.2)" },
    { id: "steamrails", name: "Create: Steam 'n' Rails", author: "Layers of Railways", desc: "Mais trilhos, sinais, engates e estilos de locomotiva para os trens do Create.", dl: "9,1 mi", updated: "3 set 2026", srcs: ["modrinth", "curseforge"] },
    { id: "ccc", name: "Create: Connected", author: "Lysine", desc: "Pequenos blocos utilitários que completam as máquinas do Create.", dl: "2,3 mi", updated: "28 ago 2026", srcs: ["modrinth"] },
    { id: "createenchant", name: "Create Enchantment Industry", author: "MrKitsune", desc: "Automatize encantamentos e experiência com fluidos e máquinas do Create.", dl: "6,7 mi", updated: "19 ago 2026", srcs: ["curseforge"] },
    { id: "embeddium", name: "Embeddium", author: "embeddedt", desc: "Motor de renderização alternativo para Forge e NeoForge.", dl: "31 mi", updated: "2 jun 2026", srcs: ["modrinth", "curseforge"], state: "conflict", stateText: "Incompatível com o pack" },
    { id: "createdeco", name: "Create Deco", author: "MRH0", desc: "Blocos decorativos industriais: grades, portas, placas e luzes no estilo do Create.", dl: "11 mi", updated: "30 jul 2026", srcs: ["modrinth", "curseforge"] },
  ];

  const createAdditionDetail = {
    versions: [
      { v: "1.3.4", type: "Estável", date: "12 set 2026", note: "compatível com Create 6.0.8", rec: true },
      { v: "1.3.3", type: "Estável", date: "20 ago 2026", note: "" },
      { v: "1.3.2", type: "Estável", date: "1 ago 2026", note: "versão que está no pack" },
      { v: "1.4.0-beta.2", type: "Beta", date: "25 set 2026", note: "pode ter erros" },
    ],
    deps: {
      required: [{ name: "Create", note: "Já está no pack (6.0.8)", inPack: true }],
      optional: [
        { name: "CC: Tweaked", note: "Permite controlar as máquinas com computadores do jogo", inPack: false },
        { name: "Just Enough Items (JEI)", note: "Já está no pack", inPack: true },
      ],
    },
  };

  const checks = [
    {
      id: "c1", sev: "danger", title: "Sodium e Embeddium estão juntos no pack",
      body: "Os dois fazem a mesma coisa (trocam o jeito como o jogo desenha os blocos) e não podem rodar ao mesmo tempo. No NeoForge 1.21.1 o Sodium já funciona sozinho, então o Embeddium não é necessário.",
      ev: "Regra do Warden: só um mod de renderização por pack. Encontrado em <code>mods/sodium.pw.toml</code> e <code>mods/embeddium.pw.toml</code>.",
      fixes: [{ label: "Remover Embeddium", primary: true }, { label: "Remover Sodium" }],
    },
    {
      id: "c2", sev: "danger", title: "Waystones precisa do mod Balm, que não está no pack",
      body: "Sem o Balm, o Waystones não abre e o jogo para na tela de carregamento.",
      ev: "Arquivo <code>waystones-neoforge-1.21.1-21.1.12.jar</code> → <code>META-INF/neoforge.mods.toml</code>: dependência obrigatória <code>balm</code> (21.0.0 ou mais nova).",
      fixes: [{ label: "Adicionar Balm 21.0.31", primary: true }],
    },
    {
      id: "c3", sev: "warn", title: "Não sabemos de que lado “Utilitários do Vale” precisa",
      body: "O arquivo é local e não diz se roda só no jogador (cliente), só no servidor ou nos dois. Por enquanto ele vai para os dois.",
      ev: "Arquivo <code>vale-utils-1.0.0.jar</code> sem campo de ambiente nos metadados.",
      fixes: [{ label: "Marcar como “Ambos”", primary: true }, { label: "Escolher lado…" }],
    },
    {
      id: "c4", sev: "warn", title: "Fresh Animations precisa de 2 mods que não estão no pack",
      body: "Sem eles, o resource pack carrega, mas as animações novas não aparecem.",
      ev: "Página do Fresh Animations no Modrinth: requer Entity Model Features e Entity Texture Features.",
      fixes: [{ label: "Adicionar os 2 mods", primary: true }, { label: "Ignorar" }],
    },
    {
      id: "c5", sev: "info", title: "O teste vai usar o Java 21",
      body: "É a versão que o Minecraft 1.21.1 com NeoForge pede. Ela já está instalada, nada para baixar.",
      ev: "Tabela de Java do Warden: 1.20.5 a 1.21.x → Java 21.",
      fixes: [],
    },
  ];
  const passedChecks = [
    "Todos os mods são para NeoForge", "Todos os mods aceitam o Minecraft 1.21.1", "Nenhum mod repetido",
    "Nenhum mod aparece em duas fontes", "Versão do NeoForge aceita por todos os mods", "Memória escolhida (6 GB) cabe no computador",
    "Nenhum mod pede um Java mais novo", "Resource packs no formato da 1.21.1", "Shaders têm o Iris instalado",
    "Arquivos do pack batem com o index (packwiz)", "Nenhum arquivo suspeito na pasta do pack",
  ];

  const prepSteps = [
    { id: "java", title: "Java 21", detail: "Já instalado (Eclipse Temurin 21.0.8)", total: 0 },
    { id: "mc", title: "Minecraft 1.21.1", detail: "Bibliotecas e arquivos do jogo", total: 3412, unit: "arquivos" },
    { id: "loader", title: "NeoForge 21.1.209", detail: "Instalando o loader", total: 10, unit: "etapas" },
    { id: "mods", title: "Mods e arquivos do pack", detail: "24 mods, 3 resource packs, 2 shaders, configs", total: 29, unit: "arquivos" },
    { id: "sync", title: "Preparar a instância de teste", detail: "Copiar configs e guardar o “antes” para comparar depois", total: 1 },
  ];

  const consoleLines = [
    ["21:12:03", "INFO", "[main/INFO] [cpw.mods.modlauncher.Launcher/MODLAUNCHER]: ModLauncher running: args [--username, Construtor42, --version, neoforge-21.1.209, --gameDir, C:\\Users\\Rafael\\AppData\\Roaming\\Warden\\instances\\vale-das-engrenagens\\minecraft, --assetsDir, …]"],
    ["21:12:03", "INFO", "[main/INFO] [cpw.mods.modlauncher.Launcher/MODLAUNCHER]: JVM identified as Eclipse Adoptium OpenJDK 64-Bit Server VM 21.0.8+9-LTS"],
    ["21:12:04", "INFO", "[main/INFO] [net.neoforged.fml.loading.FMLLoader/CORE]: Starting FancyModLoader"],
    ["21:12:05", "INFO", "[main/INFO] [mixin/]: SpongePowered MIXIN Subsystem Version=0.8.7 Source=union:/C:/Users/Rafael/AppData/Roaming/Warden/shared/libraries/… Service=ModLauncher Env=CLIENT"],
    ["21:12:06", "WARN", "[main/WARN] [mixin/]: Reference map 'xaerominimap.refmap.json' for xaerominimap.mixins.json could not be read. If this is a development environment you can ignore this message"],
    ["21:12:08", "INFO", "[main/INFO] [net.neoforged.fml.loading.moddiscovery.ModDiscoverer/SCAN]: Found 25 mod files"],
    ["21:12:09", "INFO", "[Render thread/INFO] [net.minecraft.client.Minecraft/]: Setting user: Construtor42"],
    ["21:12:11", "INFO", "[Render thread/INFO] [com.mojang.blaze3d.systems.RenderSystem/]: Backend library: LWJGL version 3.3.3"],
    ["21:12:14", "INFO", "[Render thread/INFO] [net.minecraft.server.packs.resources.ReloadableResourceManager/]: Reloading ResourceManager: vanilla, mod_resources, Moonlight Mods Dynamic Assets"],
    ["21:12:17", "INFO", "[modloading-worker-0/INFO] [com.simibubi.create.Create/]: Create 6.0.8 initializing!"],
    ["21:12:19", "WARN", "[modloading-worker-2/WARN] [net.neoforged.fml.ModLoader/LOADING]: Mod 'vale_utils' declares no display test and no environment"],
    ["21:12:22", "INFO", "[Render thread/INFO] [net.minecraft.client.sounds.SoundEngine/]: Sound engine started"],
    ["21:12:26", "INFO", "[Render thread/INFO] [net.minecraft.client.renderer.texture.TextureAtlas/]: Created: 4096x2048x4 minecraft:textures/atlas/blocks.png-atlas"],
    ["21:12:31", "INFO", "[Render thread/INFO] [net.minecraft.client.Minecraft/]: Loaded 2 shader packs"],
    ["21:12:33", "INFO", "[Server thread/INFO] [net.minecraft.server.MinecraftServer/]: Preparing level \"Teste\""],
    ["21:12:41", "INFO", "[Server thread/INFO] [net.minecraft.server.MinecraftServer/]: Done (7.912s)! For help, type \"help\""],
    ["21:12:42", "INFO", "[Render thread/INFO] [net.minecraft.client.gui.components.ChatComponent/]: [CHAT] Construtor42 entrou no jogo"],
  ];

  const changes = [
    {
      id: "ch1", path: "options.txt", kind: "Alterado", pick: true,
      why: "Você mudou 3 opções do jogo.",
      dest: "Vira padrão de primeira execução: o jogador recebe na primeira vez e depois pode mudar.",
      keys: [
        { k: "renderDistance", label: "Distância de renderização", old: "12", nw: "10", pick: true },
        { k: "guiScale", label: "Escala da interface", old: "0 (automática)", nw: "3", pick: true },
        { k: "resourcePacks", label: "Resource packs ativos", old: "[\"vanilla\", \"mod_resources\"]", nw: "[\"vanilla\", \"mod_resources\", \"file/Texturas do Vale.zip\"]", pick: true },
        { k: "soundCategory_music", label: "Volume da música", old: "1.0", nw: "0.35", pick: false, note: "Parece preferência pessoal" },
      ],
    },
    {
      id: "ch2", path: "config/jade/plugins.json", kind: "Alterado", pick: true,
      why: "Opções do Jade mudadas dentro do jogo (menu de configurações do mod).",
      dest: "Vai para config/jade/plugins.json no pack.",
      diff: [
        [" ", "{"],
        [" ", "  \"minecraft\": {"],
        ["-", "    \"item_storage\": true,"],
        ["+", "    \"item_storage\": false,"],
        [" ", "    \"harvest_tool\": true,"],
        ["-", "    \"mob_spawner\": true"],
        ["+", "    \"mob_spawner\": false"],
        [" ", "  },"],
        [" ", "  \"create\": { \"goggles\": true }"],
        [" ", "}"],
      ],
    },
    {
      id: "ch3", path: "saves/Teste/serverconfig/create-server.toml", kind: "Alterado", pick: true,
      why: "Config de servidor do Create, mudada no mundo de teste.",
      dest: "Vai para defaultconfigs/create-server.toml: vale para todo mundo novo criado com o pack.",
      diff: [
        [" ", "[trains]"],
        [" ", "\t#Whether moving Trains can hurt colliding mobs and players."],
        ["-", "\ttrainsCauseDamage = true"],
        ["+", "\ttrainsCauseDamage = false"],
        [" ", "\t#Maximum length of track that can be placed as one batch or turn."],
        [" ", "\tmaxTrackPlacementLength = 32"],
      ],
    },
    {
      id: "ch4", path: "config/sodium-options.json", kind: "Alterado", pick: false,
      why: "Qualidade gráfica do Sodium. Geralmente é gosto de cada jogador.",
      dest: "Vai para config/sodium-options.json no pack.",
      diff: [
        [" ", "\"quality\": {"],
        ["-", "  \"weather_quality\": \"DEFAULT\","],
        ["+", "  \"weather_quality\": \"FAST\","],
        ["-", "  \"leaves_quality\": \"DEFAULT\""],
        ["+", "  \"leaves_quality\": \"FAST\""],
        [" ", "}"],
      ],
    },
    {
      id: "ch5", path: "config/xaerominimap.txt", kind: "Novo", pick: false,
      why: "Criado pelo próprio mod na primeira vez que o jogo abriu, com os valores padrão.",
      dest: "Não precisa ir para o pack: o mod recria sozinho.",
      diff: [["+", "ignoreUpdate:0"], ["+", "minimapSize:0"], ["+", "zoom:1"], ["+", "chunkGrid:-1"], ["+", "…mais 212 linhas"]],
    },
  ];
  const ignored = [
    "logs/latest.log", "logs/debug.log", "saves/Teste/ (mundo de teste)", "screenshots/2026-10-01_21.15.02.png",
    "usercache.json", ".mixin.out/", "XaeroWaypoints/", "XaeroWorldMap/", "config/sodium-fingerprint.json",
    "command_history.txt", "hotbar.nbt", "crash-reports/ (vazio)", "options.txt → lastServer", "options.txt → tutorialStep",
  ];

  const crashLog = [
    [1827, "[21:43:40] [modloading-worker-1/INFO] [com.simibubi.create.Create/]: Create 6.0.8 initializing!"],
    [1828, "[21:43:41] [modloading-worker-3/ERROR] [net.neoforged.fml.ModLoader/LOADING]: Failed to create mod instance. ModID: createaddition, class com.mrh0.createaddition.CreateAddition", true],
    [1829, "java.lang.NoSuchMethodError: 'void com.simibubi.create.foundation.data.CreateRegistrate.setTooltipModifierFactory(java.util.function.Function)'", true],
    [1830, "\tat com.mrh0.createaddition.CreateAddition.<init>(CreateAddition.java:84) ~[createaddition-1.21.1-1.3.2.jar%23142!/:1.3.2]", true],
    [1831, "\tat java.base/jdk.internal.reflect.DirectConstructorHandleAccessor.newInstance(Unknown Source) ~[?:?]"],
    [1832, "\tat net.neoforged.fml.javafmlmod.FMLModContainer.constructMod(FMLModContainer.java:115) ~[loader-4.0.42.jar%23122!/:4.0]"],
    [1833, "[21:43:41] [Render thread/FATAL] [net.neoforged.neoforge.common.NeoForge/]: Mod loading failures have occurred; displaying loading errors"],
  ];

  const aiPreview = [
    "Pack: Vale das Engrenagens · Minecraft 1.21.1 · NeoForge 21.1.209 · Java 21.0.8",
    "Mods (24): create 6.0.8, createaddition 1.3.2, sodium 0.6.13, iris 1.8.8, … (lista completa anexada)",
    "Checagem automática: NoSuchMethodError em createaddition ao chamar código do create.",
    "Trecho do log (linhas 1810–1840):",
    "[21:43:38] Setting user: {{JOGADOR}}",
    "[21:43:39] Loading mods from {{PASTA}}\\AppData\\Roaming\\Warden\\instances\\vale-das-engrenagens\\minecraft\\mods",
    "[21:43:41] Failed to create mod instance. ModID: createaddition …",
    "java.lang.NoSuchMethodError: 'void com.simibubi.create…setTooltipModifierFactory(…)'",
    "[21:43:41] Connecting to {{IP}}:25565 (servidor salvo na lista)",
  ];

  const versions = [
    { v: "1.4.0", title: "Trens e cozinha", date: "29 set 2026", added: 3, updated: 6, removed: 0, configs: 2, test: "warn", testText: "Passou com avisos", pushed: true, current: true },
    { v: "1.3.1", title: "Correções de configs", date: "21 set 2026", added: 0, updated: 4, removed: 0, configs: 3, test: "ok", testText: "Passou", pushed: true },
    { v: "1.3.0", title: "Armazenamento", date: "14 set 2026", added: 5, updated: 2, removed: 0, configs: 1, test: "ok", testText: "Passou", pushed: true },
    { v: "1.2.0", title: "Mapas e viagem rápida", date: "2 set 2026", added: 4, updated: 7, removed: 1, configs: 0, test: "ok", testText: "Passou", pushed: true },
    { v: "1.0.0", title: "Primeira versão estável", date: "9 ago 2026", added: 0, updated: 3, removed: 0, configs: 4, test: "ok", testText: "Passou", pushed: true },
    { v: "0.1.0", title: "Começo do pack", date: "28 jul 2026", added: 12, updated: 0, removed: 0, configs: 0, test: "neutral", testText: "Não testado", pushed: false, channel: "Alfa" },
  ];

  const pending = {
    added: ["Comforts 9.0.3", "Chat Heads 0.13.9"],
    updated: ["Create 6.0.6 → 6.0.8"],
    removed: [],
    configs: ["defaultconfigs/create-server.toml (maxBeltLength: 20 → 32)", "options.txt (3 opções de primeira execução)"],
  };

  const configTree = [
    { name: "config", dir: true, open: true, children: [
      { name: "create-client.toml" },
      { name: "jade", dir: true, open: false, children: [{ name: "plugins.json" }, { name: "jade.json" }] },
      { name: "sodium-options.json" },
      { name: "supplementaries-common.toml" },
      { name: "waystones-common.toml" },
      { name: "xaerominimap.txt" },
    ] },
    { name: "defaultconfigs", dir: true, open: true, children: [
      { name: "create-server.toml", changed: true },
      { name: "waystones-server.toml" },
    ] },
    { name: "options.txt", changed: true },
  ];

  const createServerToml = [
    { section: "kinetics", title: "[kinetics]", fields: [
      { key: "maxBeltLength", type: "int", value: 32, orig: 20, def: 20, min: 5, max: 256, desc: "Maximum length in blocks of mechanical belts." },
      { key: "maxBlocksMoved", type: "int", value: 2048, orig: 2048, def: 2048, min: 1, max: 100000, desc: "Maximum amount of blocks in a structure movable by Pistons, Bearings or other means." },
      { key: "maxChassisRange", type: "int", value: 16, orig: 16, def: 16, min: 1, max: 64, desc: "Maximum value of a chassis attachment range." },
      { key: "crankHungerMultiplier", type: "float", value: 0.01, orig: 0.01, def: 0.01, min: 0, max: 1, desc: "multiplier used for calculating exhaustion from speed when a crank is turned." },
    ] },
    { section: "trains", title: "[trains]", fields: [
      { key: "trainsCauseDamage", type: "bool", value: true, orig: true, def: true, desc: "Whether moving Trains can hurt colliding mobs and players." },
      { key: "maxTrackPlacementLength", type: "int", value: 32, orig: 32, def: 32, min: 16, max: 128, desc: "Maximum length of track that can be placed as one batch or turn." },
    ] },
  ];

  const exportTree = [
    { name: "pack.toml", size: 0.4, kind: "file", note: "Nome, versão 1.5.0, Minecraft e NeoForge" },
    { name: "index.toml", size: 7.9, kind: "file", note: "Lista de todos os arquivos com hash" },
    { name: ".packwizignore", size: 0.6, kind: "file", note: "O que o packwiz deve ignorar" },
    { name: "mods/", size: 95.8, kind: "dir", note: "24 referências (.pw.toml) + 1 arquivo local de 84 KB", count: 25 },
    { name: "resourcepacks/", size: 2151.1, kind: "dir", note: "2 referências + Texturas do Vale.zip (2,1 MB)", count: 3 },
    { name: "shaderpacks/", size: 0.9, kind: "dir", note: "2 referências (.pw.toml)", count: 2 },
    { name: "config/", size: 286.0, kind: "dir", note: "31 arquivos de configuração", count: 31 },
    { name: "defaultconfigs/", size: 14.2, kind: "dir", note: "3 configs de servidor padrão", count: 3 },
  ];
  const exportExcluded = [
    "logs/, crash-reports/, saves/, screenshots/ (gerados pelo jogo)",
    ".mixin.out/, .fabric/, .cache/ (caches)",
    "usercache.json, command_history.txt, hotbar.nbt",
    "XaeroWaypoints/, XaeroWorldMap/ (dados do minimapa)",
    ".warden/ (dados internos do Warden)",
  ];

  return {
    packs, mods, resourcepacks, shaders, mcVersions, loaderVersions, searchResults, createAdditionDetail,
    checks, passedChecks, prepSteps, consoleLines, changes, ignored, crashLog, aiPreview, versions, pending,
    configTree, createServerToml, exportTree, exportExcluded,
  };
})();
