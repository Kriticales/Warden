/**
 * Textos de "Mods opcionais e fixar versão" (T11): "Mais opções" do painel de detalhes do item
 * e "Mods opcionais" dos Ajustes do teste. Termos do glossário (QUALITY §8.2) e do protótipo
 * aprovado (`design/prototipo-final/screens-pack.js` → `detailDrawer`).
 */
export const opcionais = {
  maisOpcoes: 'Mais opções',
  ligado: 'Ligado',
  desligado: 'Desligado',
  fixar: {
    rotulo: 'Fixar versão (não atualizar)',
    dica: 'Um item fixado fica na versão atual: "Atualizar todos" pula ele. Muda o arquivo do pack na hora.',
    fixado: 'Versão fixada.',
    solto: 'Versão solta.',
  },
  opcional: {
    rotulo: 'Opcional para o jogador',
    dica: 'O jogador escolhe se instala este item. Muda o arquivo do pack na hora.',
    descricao: 'Descrição para o jogador',
    descricaoOpcional: '(opcional)',
    descricaoDica: 'Aparece na janela de escolha do packwiz-installer.',
    descricaoPlaceholder: 'Para que serve, em uma frase',
    padrao: 'Ligado por padrão',
    padraoDica: 'Vem marcado para o jogador. Sem janela, o instalador segue este valor.',
    salvar: 'Salvar',
    salvando: 'Salvando…',
    marcado: 'Item marcado como opcional.',
    desmarcado: 'Item deixou de ser opcional.',
    salvo: 'Opção salva.',
    muitoLongo: 'Use no máximo {{max}} caracteres.',
    umaLinha: 'Escreva em uma linha só.',
    semArquivo: 'Este item não tem arquivo de referência (.pw.toml); não pode ser opcional.',
  },
  avisoFormatos:
    'O app do Modrinth instala todos os opcionais; o formato da CurseForge não suporta lado.',
  avisoFormatosTitulo: 'Em outros formatos',
  teste: {
    titulo: 'Mods opcionais',
    texto:
      'Escolha quais opcionais ficam ligados na instância de teste deste computador. Não entram no pack. Cada interruptor grava na hora.',
    nenhum: 'Este pack não tem mods opcionais.',
    padrao: 'Padrão do pack: {{estado}}',
    ligado: 'ligado',
    desligado: 'desligado',
    salvando: 'Salvando…',
    salvo: 'Opcionais do teste salvos.',
    voltarPadrao: 'Voltar ao padrão do pack',
    lista: 'Mods opcionais do pack',
    carregando: 'Lendo os opcionais…',
    aplicaNoProximo: 'Vale a partir do próximo teste.',
  },
} as const;
