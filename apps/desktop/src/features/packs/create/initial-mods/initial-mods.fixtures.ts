/**
 * Dados de teste da etapa "Mods iniciais" e dos kits: no formato do `bindings.ts`, com os
 * valores que o Rust devolveria para Fabric 1.21.1 e Forge 1.12.2 (`data/initial-mods.toml` e
 * `data/kits.toml`).
 */
import type {
  InitialModsResult,
  InitialOffer,
  Kit,
  KitItem,
  OfferItem,
} from '../../../../lib/ipc/bindings';

export function makeOfferItem(patch: Partial<OfferItem> & Pick<OfferItem, 'id'>): OfferItem {
  return {
    checked: true,
    source: 'modrinth',
    projectId: 'AAAAAAAA',
    title: patch.id,
    version: null,
    old: false,
    side: null,
    with: [],
    unavailable: null,
    ...patch,
  };
}

/** O que o Rust oferece para Fabric 1.21.1. */
export const FABRIC_OFFER: InitialOffer = {
  items: [
    makeOfferItem({
      id: 'spark',
      projectId: 'l6YH9Als',
      title: 'spark',
      version: '1.10.109-fabric',
      side: 'both',
      with: ['Fabric API'],
    }),
    makeOfferItem({
      id: 'crash-assistant',
      projectId: 'ix1qq8Ux',
      title: 'Crash Assistant',
      version: '1.11.12',
    }),
  ],
};

/** O que o Rust oferece para Forge 1.12.2 sem a chave da CurseForge. */
export const OLD_OFFER: InitialOffer = {
  items: [
    makeOfferItem({
      id: 'spark',
      source: 'curseforge',
      projectId: '361579',
      title: 'spark',
      version: '1.6.3',
      old: true,
      side: 'both',
      unavailable: 'curseforgeOff',
    }),
    makeOfferItem({
      id: 'crash-assistant',
      projectId: 'ix1qq8Ux',
      title: 'Crash Assistant',
      version: '1.11.12',
    }),
  ],
};

export function makeKitItem(patch: Partial<KitItem> & Pick<KitItem, 'name'>): KitItem {
  return {
    source: 'modrinth',
    projectId: patch.name.slice(0, 8).padEnd(8, 'x'),
    side: null,
    checked: true,
    note: null,
    ...patch,
  };
}

/** O kit do Fabric 1.21 (resumido a cinco mods, com o C2ME desmarcado e avisado). */
export const FABRIC_KIT: Kit = {
  id: 'fabric-moderno',
  name: 'Desempenho para Fabric 1.21 em diante',
  description:
    'Mods que aumentam os quadros por segundo e diminuem o uso de memória sem mudar o jogo.',
  items: [
    makeKitItem({ name: 'Sodium', projectId: 'AANobbMI', side: 'client' }),
    makeKitItem({ name: 'Lithium', projectId: 'gvQqBUqZ', side: 'both' }),
    makeKitItem({ name: 'FerriteCore', projectId: 'uXXizFIs', side: 'both' }),
    makeKitItem({ name: 'ImmediatelyFast', projectId: '5ZwdcRci', side: 'client' }),
    makeKitItem({
      name: 'C2ME',
      projectId: 'VSNURh3q',
      side: 'both',
      checked: false,
      note: 'worldgen',
    }),
  ],
};

export function makeInitialResult(patch: Partial<InitialModsResult> = {}): InitialModsResult {
  return {
    added: [
      { key: 'modrinth:l6YH9Als', title: 'spark', path: 'mods/spark.pw.toml' },
      { key: 'modrinth:P7dR8mSH', title: 'Fabric API', path: 'mods/fabric-api.pw.toml' },
      {
        key: 'modrinth:ix1qq8Ux',
        title: 'Crash Assistant',
        path: 'mods/crash-assistant.pw.toml',
      },
    ],
    leftOut: [],
    crashAssistantConfig: true,
    playerTools: ['spark', 'crash-assistant'],
    ...patch,
  };
}
