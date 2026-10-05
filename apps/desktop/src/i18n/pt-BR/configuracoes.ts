/**
 * Textos de Configurações (T21) e das partes que a primeira execução (T01) reaproveita: nome
 * do jogador, pasta dos packs e chaves. O aviso do `.env` tem a redação da SPEC (T21).
 */
export const configuracoes = {
  titulo: 'Configurações',
  subtitulo: 'Valem para o Warden inteiro. O que é de um pack fica dentro do pack.',
  voltar: 'Meus packs',
  carregando: 'Lendo as configurações…',
  salvo: 'Configuração salva.',
  naoSalvou: 'A configuração não foi salva',
  geral: {
    titulo: 'Geral',
    revisarBoasVindas: 'Rever boas-vindas',
  },
  pasta: {
    rotulo: 'Pasta dos packs',
    escolher: 'Escolher…',
    escolhendo: 'Escolhendo…',
    dica: 'Cada pack novo vira uma pasta aqui dentro. Os packs que já existem continuam onde estão.',
    alterada: 'Pasta dos packs alterada.',
    motivo: {
      notFound: 'Essa pasta não existe mais. Escolha outra.',
      notDirectory: 'O caminho escolhido não é uma pasta. Escolha uma pasta.',
      symlink:
        'Essa pasta é um atalho (link ou junção) para outro lugar. Escolha a pasta de destino diretamente.',
      insideAppData:
        'Essa pasta fica dentro dos dados do próprio Warden, que podem ser apagados ao limpar o cache. Escolha outra, como Documentos.',
      notWritable:
        'O Warden não tem permissão para gravar nessa pasta. Escolha outra ou ajuste as permissões dela no Windows.',
    },
  },
  jogador: {
    rotulo: 'Nome do jogador nos testes',
    dica: 'Aparece no jogo de teste. O teste é offline.',
    regra: 'Letras, números e _, de 3 a 16 caracteres.',
    invalido: 'Use só letras sem acento, números e _ (sem espaço), de 3 a 16 caracteres.',
    salvar: 'Salvar nome',
    salvo: 'Nome do jogador salvo.',
  },
  chaves: {
    titulo: 'Chaves e contas',
    texto:
      'Depois de salvas, as chaves não aparecem de novo: o Warden só mostra se estão configuradas.',
    carregando: 'Lendo o estado das chaves…',
    configurada: 'Configurada',
    funcionando: 'Funcionando',
    naoTestavel: 'Esta versão do Warden ainda não sabe testar esta chave.',
    campo: 'Cole a chave',
    campoToken: 'Cole o token',
    salvar: 'Salvar',
    salvando: 'Salvando…',
    testar: 'Testar',
    testando: 'Testando…',
    substituir: 'Substituir',
    cancelar: 'Cancelar',
    remover: 'Remover',
    salva: '{{nome}}: chave salva.',
    removida: '{{nome}}: chave removida.',
    comoCriar: 'Como criar',
    abrirPagina: 'Abrir a página no navegador',
    removerTitulo: 'Remover a chave da {{nome}}?',
    removerTexto:
      'A chave sai de onde está guardada. O que depende dela para de funcionar até você salvar outra.',
    removerConfirmar: 'Remover chave',
    removendo: 'Removendo…',
    nomes: {
      curseforge: 'CurseForge',
      gemini: 'Gemini',
      github: 'GitHub',
    },
    rotulos: {
      curseforge: 'Chave da CurseForge',
      gemini: 'Chave do Gemini',
      github: 'Token do GitHub',
    },
    usos: {
      curseforge: 'Sem ela, a busca mostra só o Modrinth.',
      gemini: 'Usada só no Diagnóstico com IA, e sempre com a sua confirmação antes de cada envio.',
      github: 'Usado para publicar versões para os jogadores.',
    },
    passos: {
      curseforge: [
        'Entre no console da CurseForge para desenvolvedores com a sua conta.',
        'Abra "API keys" e copie a chave que aparece lá.',
        'Cole a chave aqui e clique em Salvar.',
      ],
      gemini: [
        'Entre no Google AI Studio com a sua conta do Google.',
        'Clique em "Create API key" e copie a chave criada.',
        'Cole a chave aqui e clique em Salvar.',
      ],
      github: [
        'No GitHub, crie um token "fine-grained" (Settings → Developer settings → Personal access tokens).',
        'Em "Repository access", escolha "All repositories", para o Warden poder criar o repositório do pack.',
        'Permissões necessárias para publicar: Administration (leitura e escrita), para criar o repositório; Contents (leitura e escrita), para enviar os arquivos e criar a versão no GitHub; Metadata (leitura), que vem marcada.',
        'Gere o token, copie e cole aqui.',
      ],
    },
  },
  guardar: {
    legenda: 'Onde guardar as chaves',
    cofre: 'Cofre do Windows',
    cofreDesc: 'Gerenciador de Credenciais, protegido pelo seu usuário do Windows.',
    recomendado: 'Recomendado',
    env: 'Arquivo .env',
    envDesc: '%APPDATA%\\dev.kriticales.warden\\.env, fora dos packs e do GitHub.',
    trocando: 'Movendo as chaves…',
    movidasEnv: 'As chaves foram para o arquivo .env.',
    movidasCofre: 'As chaves voltaram para o cofre do Windows e o .env foi apagado.',
    confirmarTitulo: 'Guardar as chaves num arquivo .env?',
    confirmarTexto:
      'As chaves da CurseForge, do Gemini e do GitHub saem do cofre do Windows e vão para o arquivo <path>%APPDATA%\\dev.kriticales.warden\\.env</path>.',
    confirmarAvisoTitulo: 'O .env é um arquivo de texto comum.',
    confirmarAviso:
      'Qualquer programa no seu computador, e qualquer backup ou sincronização dessa pasta, consegue ler as chaves.',
    confirmarRodape:
      'O arquivo fica fora dos packs e nunca vai para o GitHub. Dá para voltar ao cofre quando quiser: as chaves voltam e o .env é apagado.',
    manter: 'Manter no cofre',
    usarEnv: 'Usar arquivo .env',
    fixoTitulo: 'As chaves estão num arquivo de texto.',
    fixoTexto:
      'Qualquer programa no seu computador, e qualquer backup ou sincronização dessa pasta, consegue ler o .env.',
    voltarCofre: 'Voltar para o cofre',
  },
  teste: {
    titulo: 'Teste',
    memoria: 'Memória padrão',
    memoriaAuto: 'Automático (pelo número de mods)',
    memoriaGb: '{{gb}} GB',
    memoriaMb: '{{mb}} MB',
    memoriaDica: 'Cada pack pode ter o seu ajuste no menu ▾ do Testar.',
    beta: 'Mostrar versões beta e alpha dos mods',
    ligado: 'Ligado',
    desligado: 'Desligado',
    atualizacoes: 'Verificar atualizações dos mods',
    intervalo: {
      '6': 'A cada 6 horas',
      '24': 'A cada 24 horas',
      '168': 'Uma vez por semana',
      '0': 'Só quando eu pedir',
      outro: 'A cada {{horas}} horas',
    },
  },
  editor: {
    titulo: 'Editor de configs',
    diferencas: 'Mostrar as diferenças antes de salvar',
    diferencasDesc: 'Antes de gravar uma config, o Warden mostra o que muda, linha a linha.',
  },
  registros: {
    titulo: 'Privacidade e registros',
    nivel: 'Nível de detalhe dos registros',
    niveis: {
      normal: 'Normal',
      detailed: 'Detalhado (para relatar um problema)',
    },
    abrirPasta: 'Abrir pasta de registros',
    dica: 'Os registros do Warden nunca guardam chaves. Antes de qualquer envio à IA, você vê o texto exato.',
  },
} as const;
