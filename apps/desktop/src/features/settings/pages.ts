/**
 * As páginas de Configurações (T21; esboço aprovado `design/bug-01/configuracoes.html`, versão
 * 2 · menu lateral), na ordem do menu, cada uma com o seu endereço (`/configuracoes/<id>`).
 * "Sobre o Warden" (T23) é sempre a última.
 */
import {
  FileCode,
  FileText,
  Gamepad2,
  HardDrive,
  Info,
  KeyRound,
  Settings,
  Sparkles,
  type LucideIcon,
} from 'lucide-react';

export const SETTINGS_PAGE_IDS = [
  'geral',
  'chaves',
  'sensor-sculk',
  'teste',
  'editor',
  'registros',
  'armazenamento',
  'sobre',
] as const;

export type SettingsPageId = (typeof SETTINGS_PAGE_IDS)[number];

/** Chaves e contas e Sensor Sculk, que outras páginas abrem por link. */
export const KEYS_PAGE: SettingsPageId = 'chaves';
export const SCULK_PAGE: SettingsPageId = 'sensor-sculk';

/** A página aberta por `/configuracoes`. */
export const DEFAULT_SETTINGS_PAGE: SettingsPageId = 'geral';

export function isSettingsPage(value: string): value is SettingsPageId {
  return (SETTINGS_PAGE_IDS as readonly string[]).includes(value);
}

/** O ícone de cada página no menu; o ✦ (brilhinho) só no Sensor Sculk. */
export const SETTINGS_PAGE_ICONS: Record<SettingsPageId, LucideIcon> = {
  geral: Settings,
  chaves: KeyRound,
  'sensor-sculk': Sparkles,
  teste: Gamepad2,
  editor: FileCode,
  registros: FileText,
  armazenamento: HardDrive,
  sobre: Info,
};
