/**
 * Larguras da coluna de conteúdo (HANDOFF §5; tokens `--size-content-*`). A coluna fica
 * centralizada na área principal; o cabeçalho de página e as ações acompanham a mesma coluna.
 *
 * - `read`: leitura e formulários (Configurações), até 880 px.
 * - `default`: o padrão, até 1180 px.
 * - `wide`: listas e grades que aproveitam telas largas (Meus packs, Mods, Adicionar), até 1600 px.
 * - `full`: telas em colunas que ocupam a janela toda, sem a margem da área principal (Configs).
 */
import { cn } from '../../lib/cn';

export type ContentWidth = 'read' | 'default' | 'wide' | 'full';

export function contentClass(width: ContentWidth): string {
  return cn('content', width !== 'default' && `content--${width}`);
}
