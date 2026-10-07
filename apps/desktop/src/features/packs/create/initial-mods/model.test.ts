import { describe, expect, it } from 'vitest';

import {
  defaultChoice,
  hasWork,
  NO_INITIAL,
  selectable,
  toRequest,
  toolNames,
  withKit,
  withKitProject,
  withTool,
} from './model';
import { FABRIC_KIT, FABRIC_OFFER, OLD_OFFER } from './initial-mods.fixtures';

describe('mods iniciais', () => {
  it('as duas ferramentas vêm marcadas e o kit desmarcado (D16)', () => {
    expect(defaultChoice(FABRIC_OFFER)).toEqual({
      tools: ['spark', 'crash-assistant'],
      kit: null,
    });
  });

  it('CA-T03-08: o spark da CurseForge sem chave não pode ser marcado e fica fora do padrão', () => {
    expect(OLD_OFFER.items.map(selectable)).toEqual([false, true]);
    expect(defaultChoice(OLD_OFFER).tools).toEqual(['crash-assistant']);
  });

  it('desmarcar as duas não deixa nada a gravar (CA-T03-07, segunda parte)', () => {
    let choice = defaultChoice(FABRIC_OFFER);
    choice = withTool(choice, 'spark', false);
    choice = withTool(choice, 'crash-assistant', false);
    expect(choice).toEqual({ tools: [], kit: null });
    expect(hasWork(choice)).toBe(false);
    expect(toRequest(choice)).toBeNull();
    expect(toRequest(NO_INITIAL)).toBeNull();
  });

  it('marcar duas vezes não repete a ferramenta', () => {
    const choice = withTool(withTool(NO_INITIAL, 'spark', true), 'spark', true);
    expect(choice.tools).toEqual(['spark']);
  });

  it('escolher o kit traz os mods marcados por padrão; trocar mod e tirar o kit', () => {
    let choice = withKit(NO_INITIAL, FABRIC_KIT);
    expect(choice.kit?.projects).toEqual(['AANobbMI', 'gvQqBUqZ', 'uXXizFIs', '5ZwdcRci']);
    choice = withKitProject(choice, 'VSNURh3q');
    choice = withKitProject(choice, 'AANobbMI');
    expect(choice.kit?.projects).toEqual(['gvQqBUqZ', 'uXXizFIs', '5ZwdcRci', 'VSNURh3q']);
    expect(withKit(choice, null).kit).toBeNull();
    // Sem kit escolhido, marcar um mod dele não faz nada.
    expect(withKitProject(NO_INITIAL, 'AANobbMI')).toBe(NO_INITIAL);
  });

  it('kit sem nenhum mod marcado não é enviado', () => {
    let choice = withKit(NO_INITIAL, FABRIC_KIT);
    for (const project of choice.kit?.projects ?? []) choice = withKitProject(choice, project);
    expect(choice.kit?.projects).toEqual([]);
    expect(toRequest(choice)).toBeNull();
    expect(toRequest({ tools: ['spark'], kit: choice.kit })).toEqual({
      tools: ['spark'],
      kit: null,
    });
  });

  it('o pedido leva as ferramentas e o kit', () => {
    const choice = withKit(defaultChoice(FABRIC_OFFER), FABRIC_KIT);
    expect(toRequest(choice)).toEqual({
      tools: ['spark', 'crash-assistant'],
      kit: { id: 'fabric-moderno', projects: ['AANobbMI', 'gvQqBUqZ', 'uXXizFIs', '5ZwdcRci'] },
    });
  });

  it('os nomes do resumo seguem a ordem da oferta', () => {
    expect(toolNames(FABRIC_OFFER, { tools: ['crash-assistant', 'spark'], kit: null })).toEqual([
      'spark',
      'Crash Assistant',
    ]);
    expect(toolNames(undefined, defaultChoice(FABRIC_OFFER))).toEqual([]);
  });
});
