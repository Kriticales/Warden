/**
 * Textos das propostas da IA e das consultas externas (SPEC T14 ✦ passos 2 e 3; ROADMAP D-14;
 * protótipo `design/prototipo-final/screens-diag.js`, `ia-conversa`). Glossário (QUALITY §8.2):
 * "proposta" com o botão **Aplicar**, "ponto de segurança", "Enviado ao GitHub", "encontrar o mod
 * culpado", "config", lado "Cliente e servidor"/"Só cliente"/"Só servidor", "Ajustes do teste".
 * Texto fixo da SPEC: "Nada muda no pack sem você clicar em Aplicar".
 */
export const iaPropostas = {
  lista: {
    pendentes_one: '{{count}} proposta esperando você',
    pendentes_other: '{{count}} propostas esperando você',
  },
  cartoes: {
    titulo: 'Propostas',
    rotulo: 'Proposta',
    comBaseEm: 'Com base em',
    nadaMuda: 'Nada muda no pack sem você clicar em Aplicar.',
  },
  titulo: {
    addMod: 'Adicionar {{nome}}',
    addModVersao: 'Adicionar {{nome}} {{versao}}',
    removeMod: 'Remover {{nome}}',
    updateMod: 'Atualizar {{nome}} {{de}} → {{para}}',
    changeVersion: 'Trocar {{nome}} para a versão {{para}}',
    setSide: 'Mudar o lado de {{nome}} para {{lado}}',
    editConfig: 'Mudar {{chave}} de {{de}} para {{para}}',
    testSettings: 'Mudar os ajustes do teste',
    runTest: 'Testar de novo',
    startBisect: 'Encontrar o mod culpado',
  },
  lado: {
    both: 'Cliente e servidor',
    client: 'Só cliente',
    server: 'Só servidor',
    unknown: 'lado desconhecido',
  },
  ajustes: {
    memoria: 'Memória do teste: {{valor}} MB',
    memoriaAuto: 'Memória do teste: automática',
    java: 'Java {{versao}}',
    javaAuto: 'Java automático',
    argumentos: 'Argumentos da JVM: {{valor}}',
    semArgumentos: 'Argumentos da JVM: nenhum',
    perfil: 'Perfil do teste: {{nome}}',
    perfilPadrao: 'Perfil do teste: Padrão',
    soNeste: 'Só neste computador: o pack não muda.',
  },
  busca: {
    texto:
      'A busca do culpado testa o pack em rodadas, ligando e desligando mods, até achar quais causam o problema.',
  },
  acoes: {
    aplicar: 'Aplicar',
    aplicarRotulo: 'Aplicar a proposta: {{titulo}}',
    descartar: 'Descartar',
    descartarRotulo: 'Descartar a proposta: {{titulo}}',
    descartando: 'Descartando…',
    testarAgora: 'Testar agora',
    comecarBusca: 'Começar a busca',
    desfazer: 'Desfazer',
    esperandoIa: 'Espere a IA terminar de responder para decidir.',
    comJogoAberto: 'Feche o jogo aberto para testar.',
  },
  estado: {
    aplicada: 'Aplicada {{quando}}',
    aplicadaPonto: 'Aplicada {{quando}} · ponto de segurança criado',
    descartada: 'Descartada',
    descartadaTexto: 'Nada foi gravado no pack.',
  },
  dialogo: {
    titulo: 'Aplicar a proposta?',
    sub: 'Confira exatamente o que vai mudar. Antes, o Warden cria um ponto de segurança para você poder voltar.',
    subSemPack: 'Confira exatamente o que vai mudar.',
    calculando: 'Preparando a diferença…',
    erro: 'Não foi possível preparar a proposta. Nada foi gravado.',
    cancelar: 'Cancelar',
    aplicar: 'Aplicar',
    aplicando: 'Aplicando…',
    entram_one: 'Entra {{count}} item no pack:',
    entram_other: 'Entram {{count}} itens no pack:',
    dependencia: 'dependência de {{de}}',
    jaNoPack: 'Já estão no pack: {{nomes}}.',
    semVersao: 'Sem versão compatível com o pack: {{nomes}}.',
    incompativeis: 'Declarados incompatíveis: {{nomes}}.',
    sai: 'Sai do pack: {{nomes}}.',
    deixamDeFuncionar_one: 'Deixa de funcionar sem ele: {{nomes}}.',
    deixamDeFuncionar_other: 'Deixam de funcionar sem ele: {{nomes}}.',
    semUso: 'Ficam sem uso (continuam no pack): {{nomes}}.',
    versao: '{{nome}}: {{de}} → {{para}}',
    rebaixamento: 'É uma versão mais antiga que a instalada.',
    novasDependencias: 'Precisa de dependências que não estão no pack: {{nomes}}.',
    conflitos: 'Declara incompatibilidade com: {{nomes}}.',
    lado: '{{nome}}: {{de}} → {{para}}',
    antes: 'Antes',
    depois: 'Depois',
  },
  desfazer: {
    titulo: 'Desfazer a proposta?',
    texto:
      'O pack volta ao ponto de segurança criado antes de aplicar. O que mudou no pack depois disso também é desfeito; o estado de agora fica guardado em outro ponto de segurança.',
    confirmar: 'Voltar ao ponto de segurança',
    confirmando: 'Voltando…',
  },
  avisos: {
    aplicada: 'Proposta aplicada.',
    descartada: 'Proposta descartada.',
    desfeita: 'O pack voltou ao ponto de segurança.',
  },
  github: {
    titulo: 'Enviado ao GitHub: busca de issues em {{repositorio}} com “{{termos}}”',
    cache:
      'Resposta guardada de uma busca igual há menos de uma hora: nada foi enviado de novo ao GitHub.',
    exato: 'Exatamente o que foi enviado',
    exatoRotulo: 'Pedido enviado ao GitHub',
  },
  enviado: {
    issues: 'busca de issues de {{mod}} com “{{busca}}”',
    changelog: 'changelog de {{mod}}',
    changelogEntre: 'changelog de {{mod}}, de {{de}} a {{para}}',
    raioX: 'o que {{mod}} altera no jogo',
    raioXPack: 'pontos do jogo alterados por mais de um mod',
    busca: 'resultado da busca do culpado',
    proposta: 'proposta: {{tipo}}',
  },
  tipos: {
    propose_add_mod: 'adicionar mod',
    propose_remove_mod: 'remover mod',
    propose_update_mod: 'atualizar mod',
    propose_change_version: 'trocar de versão',
    propose_set_side: 'mudar o lado',
    propose_edit_config: 'mudar uma config',
    propose_test_settings: 'ajustes do teste',
    propose_run_test: 'testar de novo',
    propose_start_bisect: 'encontrar o mod culpado',
  },
  selo: {
    issue: 'issue {{numero}}',
    issues: 'busca de issues',
    changelog: 'changelog {{versao}}',
    changelogSemVersao: 'changelog',
    raioX: 'o que o mod altera no jogo',
    busca: 'busca do culpado',
    rodada: 'rodada {{numero}} da busca do culpado',
  },
} as const;

// Registro do namespace (veja ../catalogo.ts).
declare module '../catalogo' {
  interface Catalogo {
    iaPropostas: typeof iaPropostas;
  }
}
