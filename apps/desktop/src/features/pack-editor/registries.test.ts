/**
 * Contrato dos pontos de extensão do pack aberto (várias tarefas futuras registram aqui):
 * seções do menu (`sections.ts`), botões do cabeçalho (`header/slots.ts`) e blocos dos
 * detalhes do item (`details/blocks.ts`).
 */
import { describe, expect, it } from 'vitest';

import { editor } from '../../i18n/pt-BR/editor';
import { detailBlocks, orderedBlocks, type DetailBlock } from './details/blocks';
import { headerSlots, slotsFor, type HeaderSlot } from './header/slots';
import { DEFAULT_SECTION, PACK_SECTIONS, sectionPage, sectionPages } from './sections';

const Nada = () => null;

describe('seções do pack (sections.ts)', () => {
  it('são exatamente as 6 da SPEC T05, na ordem, com o ✦ só na IA', () => {
    expect(PACK_SECTIONS.map((section) => section.id)).toEqual([
      'mods',
      'configs',
      'problemas',
      'ia',
      'historico',
      'exportar',
    ]);
    expect(PACK_SECTIONS.filter((section) => section.ai).map((section) => section.id)).toEqual([
      'ia',
    ]);
  });

  it('nomes e descrições seguem a ESTRUTURA §13', () => {
    const texts = PACK_SECTIONS.map((section) => editor.secoes[section.id]);
    expect(texts).toEqual([
      { nome: 'Mods', desc: 'Mods, resource packs e shaders' },
      { nome: 'Configs', desc: 'Arquivos de ajuste e scripts do pack' },
      { nome: 'Problemas', desc: 'Saúde do pack, problemas e travamentos' },
      { nome: 'Diagnóstico com IA', desc: 'Conversar com a IA sobre um problema do pack' },
      { nome: 'Histórico', desc: 'Versões salvas e publicação' },
      { nome: 'Exportar', desc: 'Gerar o pack para quem vai jogar' },
    ]);
  });

  it('o pack abre em Mods, que tem página e contador', () => {
    expect(DEFAULT_SECTION).toBe('mods');
    const mods = sectionPage('mods');
    expect(mods?.to).toBe('/packs/$packId/mods');
    expect(mods?.useCount).toBeTypeOf('function');
  });

  it('cada página registrada é de uma seção existente, uma por seção', () => {
    const ids = sectionPages.map((page) => page.section);
    expect(new Set(ids).size).toBe(ids.length);
    for (const id of ids) {
      expect(PACK_SECTIONS.some((section) => section.id === id)).toBe(true);
    }
    expect(sectionPage('configs', [])).toBeUndefined();
  });
});

describe('cabeçalho do pack (header/slots.ts)', () => {
  it('a P1-08 registra Salvar versão e Testar (ações) e o aviso de higiene', () => {
    expect(slotsFor('actions').map((slot) => slot.id)).toEqual(['save', 'test']);
    expect(slotsFor('alerts').map((slot) => slot.id)).toEqual(['hygiene']);
    const ids = headerSlots.map((slot) => slot.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('ordena por `order` dentro do lugar', () => {
    const slots: HeaderSlot[] = [
      { id: 'c', placement: 'actions', order: 300, component: Nada },
      { id: 'a', placement: 'actions', order: 100, component: Nada },
      { id: 'x', placement: 'alerts', order: 50, component: Nada },
      { id: 'b', placement: 'actions', order: 200, component: Nada },
    ];
    expect(slotsFor('actions', slots).map((slot) => slot.id)).toEqual(['a', 'b', 'c']);
    expect(slotsFor('alerts', slots).map((slot) => slot.id)).toEqual(['x']);
  });
});

describe('detalhes do item (details/blocks.ts)', () => {
  it('blocos da P1-08 e da P1-14 na ordem, com espaço para a D-07 (500) e o raio-x (600)', () => {
    expect(orderedBlocks().map((block) => block.id)).toEqual([
      'cabecalho',
      'origem',
      'descricao',
      'versao',
      'lado',
      'novidades',
      'mais-opcoes',
    ]);
    const orders = detailBlocks.map((block) => block.order);
    expect(orders.some((order) => order === 500 || order === 600)).toBe(false);
    const ids = detailBlocks.map((block) => block.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('um bloco novo entra na posição dada por `order`', () => {
    const dependencias: DetailBlock = { id: 'dependencias', order: 500, component: Nada };
    expect(orderedBlocks([...detailBlocks, dependencias]).map((block) => block.id)).toEqual([
      'cabecalho',
      'origem',
      'descricao',
      'versao',
      'lado',
      'dependencias',
      'novidades',
      'mais-opcoes',
    ]);
  });
});
