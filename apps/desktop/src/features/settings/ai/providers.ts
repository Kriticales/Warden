/**
 * Os provedores da IA na interface (ADR-0060; D-18): a ordem da tela, a chave de cada um, a
 * página oficial para criar a chave e as frases que dependem do provedor.
 *
 * O id do provedor (`AiProvider`) e o tipo da chave (`SecretKind`) são a mesma palavra
 * (`gemini`, `openai`, `anthropic`); `secretOf` deixa isso explícito para o TypeScript.
 */
import i18n from '../../../i18n';
import type {
  AiErrorCode,
  AiProvider,
  AppError,
  ModelChoices,
  SearchMode,
  SecretKind,
} from '../../../lib/ipc/bindings';
import { appErrorMessage, errorCodeText } from '../../../lib/ipc/errors';
import { notePhrase } from '../../../lib/ipc/last-error';

/** Na ordem da tela; o Gemini primeiro (o padrão, com plano gratuito). */
export const AI_PROVIDERS: readonly AiProvider[] = ['gemini', 'openai', 'anthropic'];

/** A chave de cada provedor no cofre. */
export function secretOf(provider: AiProvider): SecretKind & AiProvider {
  return provider;
}

/** O nome curto (glossário, QUALITY §8): Gemini, OpenAI, Claude. */
export function providerName(provider: AiProvider): string {
  return i18n.t(`ia:provedores.${provider}.nome`);
}

/** O nome no consentimento: "Google Gemini", "OpenAI", "Claude" (glossário, QUALITY §8). */
export function providerFullName(provider: AiProvider): string {
  return i18n.t(`ia:provedores.${provider}.completo`);
}

/**
 * O modelo que o backend usa no modo (ADR-0064; sem ele, o do outro modo, como no Rust). No
 * Gemini não há modo: os dois campos têm o mesmo Flash.
 */
export function modelFor(models: ModelChoices | undefined, mode: SearchMode): string | null {
  if (!models) return null;
  return mode === 'deep'
    ? (models.careful ?? models.economical)
    : (models.economical ?? models.careful);
}

/**
 * No Gemini, se o modelo da conversa é o mais forte do plano gratuito (o Flash), com o mais leve
 * para a troca na cota diária (ADR-0060, complemento de 19/10/2026). Sem o Flash na conta, as
 * duas opções usam o mais leve e não há troca (`lighter` vazio ou igual ao modelo). Sem a lista
 * de modelos ainda, vale a regra comum: o Flash.
 */
export function usesStrongest(models: ModelChoices | undefined, model: string): boolean {
  if (!models) return true;
  return models.lighter != null && models.lighter !== model;
}

/** Erros da IA cuja frase muda com o provedor (`ia.erros`). */
const BY_PROVIDER = [
  'KEY_INVALID',
  'RATE_LIMITED',
  'QUOTA_EXCEEDED',
  'TIMEOUT',
  'BILLING',
  'UNAVAILABLE',
  'NO_MODEL',
  'REJECTED',
] as const satisfies readonly AiErrorCode[];
type ByProvider = (typeof BY_PROVIDER)[number];

function byProvider(code: string): code is ByProvider {
  return (BY_PROVIDER as readonly string[]).includes(code);
}

/**
 * A frase de um erro quando o provedor é conhecido (a conversa, o consentimento, a chave em
 * Configurações): "A OpenAI recusou a chave." em vez da frase neutra do catálogo de erros. Os
 * outros erros seguem `appErrorMessage`.
 */
export function aiErrorMessage(error: AppError, provider: AiProvider): string {
  if (error.code.domain !== 'ai' || !byProvider(error.code.code)) {
    return appErrorMessage(error);
  }
  const message = i18n.t(`ia:erros.${error.code.code}.${provider}`, {
    ...error.params,
    status: error.params.status ?? i18n.t('erro.parametroAusente'),
  });
  notePhrase(errorCodeText(error.code), message);
  return message;
}
