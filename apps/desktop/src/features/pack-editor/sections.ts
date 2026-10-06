/**
 * As 6 seções do menu lateral do pack (SPEC T05; ESTRUTURA §5.2 e §13; ADR-0026 e ADR-0036).
 *
 * Duas listas:
 * - `PACK_SECTIONS`: as 6 seções, na ordem, com nome, descrição e ícone. É fixa: o menu tem
 *   sempre exatamente estas 6 (CA-T05-03), nenhuma sétima e nenhuma aba. Só muda por ADR.
 * - `sectionPages`: a página de cada seção e o contador. **Registro acréscimo-apenas**
 *   (ROADMAP §1): a tarefa dona de cada seção acrescenta uma linha quando a rota existir
 *   (Configs: C-02; Problemas: D-03; ✦ Diagnóstico com IA: D-04; Histórico: V-02; Exportar:
 *   E-01). Seção sem página aparece no menu desabilitada, com o motivo na dica.
 *
 * `useCount` é um hook (chamado sempre, na mesma ordem, porque a lista é fixa): devolve o
 * contador da seção ou `null` para não mostrar nenhum.
 */
import type { LinkProps } from '@tanstack/react-router';
import {
  FileCode,
  History,
  Package,
  Puzzle,
  Sparkles,
  TriangleAlert,
  type LucideIcon,
} from 'lucide-react';

import type { PackId } from '../../lib/ipc/bindings';
import { useModsCount } from './mods/count';

/** Identificador de uma seção do pack. */
export type PackSectionId = 'mods' | 'configs' | 'problemas' | 'ia' | 'historico' | 'exportar';

/** Uma seção do menu lateral. Nome e descrição ficam em `editor.secoes.<id>`. */
export interface PackSection {
  id: PackSectionId;
  icon: LucideIcon;
  /** Seção da IA: ícone ✦ na cor da IA (DESIGN-SYSTEM). */
  ai?: boolean;
}

/** As 6 seções, na ordem do menu (SPEC T05). */
export const PACK_SECTIONS: readonly PackSection[] = [
  { id: 'mods', icon: Puzzle },
  { id: 'configs', icon: FileCode },
  { id: 'problemas', icon: TriangleAlert },
  { id: 'ia', icon: Sparkles, ai: true },
  { id: 'historico', icon: History },
  { id: 'exportar', icon: Package },
];

/** Contador ao lado do nome da seção. */
export interface SectionCount {
  value: number;
  /** Cor do contador: neutro (itens), atenção (não salvas) ou perigo (problemas). */
  kind: 'neutral' | 'warn' | 'danger';
  /** Texto para o leitor de tela ("128 itens"). */
  label: string;
}

/** A página de uma seção. */
export interface SectionPage {
  section: PackSectionId;
  /** Rota da seção, com o parâmetro `$packId`. */
  to: NonNullable<LinkProps['to']>;
  /** Contador da seção (hook). */
  useCount?: (packId: PackId) => SectionCount | null;
}

/** Registro acréscimo-apenas: uma linha por seção com página. */
export const sectionPages: readonly SectionPage[] = [
  { section: 'mods', to: '/packs/$packId/mods', useCount: useModsCount },
];

/** A página registrada de uma seção, se houver. */
export function sectionPage(
  section: PackSectionId,
  pages: readonly SectionPage[] = sectionPages,
): SectionPage | undefined {
  return pages.find((page) => page.section === section);
}

/** A seção que o pack abre (SPEC T05: "O pack abre em Mods"). */
export const DEFAULT_SECTION: PackSectionId = 'mods';
