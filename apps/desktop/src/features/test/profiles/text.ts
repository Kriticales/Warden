/**
 * Textos e regras dos perfis do teste que dependem de valores (L-08): o nome mostrado, o nome
 * curto do botão Testar, o resumo do menu ▾, o aviso de memória e a remoção dos argumentos que
 * impedem a leitura da memória (`W_PERF_FLAGS`). Funções puras sobre o i18n.
 */
import i18n from '../../../i18n';
import type { TestMemory, TestProfile } from '../../../lib/ipc/bindings';
import { BLOCKS_MEMORY_READING } from '../../pack-editor/test-settings/jvm-args';
import { formatMemory } from '../text';

/** Id do perfil Padrão (o mesmo do backend). */
export const DEFAULT_PROFILE = 'default';

/** Maior nome de perfil (o mesmo do backend). */
export const MAX_PROFILE_NAME = 40;

/** Acima desta fração da memória do computador, a memória do teste gera aviso (SPEC T11). */
const MEMORY_WARN_SHARE = 0.75;

const t = i18n.getFixedT(null, 'perfis');

/** O nome do perfil como a tela mostra ("Padrão" para o perfil fixo). */
export function profileName(profile: TestProfile): string {
  return profile.id === DEFAULT_PROFILE ? t('padrao') : profile.name;
}

/**
 * O nome no botão Testar: sem o complemento entre parênteses do fim ("PC fraco (4 GB)" →
 * "PC fraco"), para o botão não crescer demais (CA-T11-04).
 */
export function shortProfileName(profile: TestProfile): string {
  const name = profileName(profile);
  const short = name.replace(/\s*\([^()]*\)\s*$/, '').trim();
  return short === '' ? name : short;
}

/** O resumo do perfil no menu ▾: "4 GB · Java automático · entra no mundo". */
export function profileSummary(profile: TestProfile): string {
  const memory: TestMemory = profile.memory;
  const parts = [
    memory.mode === 'fixed' ? formatMemory(memory.mb) : t('resumo.memoriaAuto'),
    profile.java ? t('resumo.javaEscolhido') : t('resumo.javaAuto'),
  ];
  if (profile.onLaunch.mode === 'world') parts.push(t('resumo.mundo'));
  return parts.join(' · ');
}

/** Se a memória fixa passa de 75% da memória do computador (quando ela é conhecida). */
export function memoryAboveRam(memory: TestMemory, computerMb: number | null): boolean {
  return (
    memory.mode === 'fixed' && computerMb !== null && memory.mb > computerMb * MEMORY_WARN_SHARE
  );
}

/**
 * Tira dos argumentos os que impedem a leitura de memória, mantendo o resto como foi
 * digitado (aspas e ordem).
 */
export function removePerfFlags(text: string): string {
  let result = text;
  for (const flag of BLOCKS_MEMORY_READING) {
    const escaped = flag.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    result = result.replace(new RegExp(`(^|\\s+)${escaped}(?=\\s|$)`, 'g'), '');
  }
  return result.trim();
}
