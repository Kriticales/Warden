/**
 * As frases de um achado (SPEC T14 "Cada achado"): título e explicação em linguagem simples,
 * a evidência ("Como sabemos") e o texto de cada correção. As frases ficam no catálogo
 * `diagnostico` (`regras.<CÓDIGO>` do pré-teste, `achados.<chave>` do pós-crash); sem frase
 * conhecida, um texto genérico com o código.
 */
import i18n from '../../i18n';
import type {
  Evidence,
  FindingSide,
  ItemRef,
  ReportFinding,
  ReportItem,
  Severity,
  SuggestedFix,
} from '../../lib/ipc/bindings';

type Translate = (key: string, options?: Record<string, unknown>) => string;

const t: Translate = (key, options) =>
  (i18n.t as unknown as Translate)(`diagnostico:${key}`, options);

function exists(key: string): boolean {
  return i18n.exists(`diagnostico:${key}`);
}

/** Classe visual da gravidade (`issue--danger`, `badge--warn`…). */
export function severityKind(severity: Severity): 'danger' | 'warn' | 'info' {
  if (severity === 'error') return 'danger';
  if (severity === 'warning') return 'warn';
  return 'info';
}

function sameRef(a: ItemRef, b: ItemRef): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** O item do pack de uma referência do achado. */
export function itemOf(entry: ReportFinding, reference: ItemRef): ReportItem | undefined {
  const index = entry.finding.items.findIndex((item) => sameRef(item, reference));
  return index >= 0 ? entry.items[index] : undefined;
}

function refLabel(reference: ItemRef): string {
  if (reference.kind === 'metafile') {
    const name = reference.path.split('/').pop() ?? reference.path;
    return name.replace(/\.pw\.toml$/, '');
  }
  return reference.kind === 'modId' ? reference.id : reference.name;
}

/** O nome de um item citado numa correção ou evidência. */
export function refName(entry: ReportFinding, reference: ItemRef): string {
  return itemOf(entry, reference)?.name ?? refLabel(reference);
}

/** A dependência que falta (pela primeira correção "Adicionar"). */
function missingDependency(entry: ReportFinding): string | undefined {
  for (const fix of entry.finding.fixes) {
    if (fix.kind === 'addDependency') {
      return entry.labels[fix.modId] ?? fix.modId;
    }
  }
  return undefined;
}

/** Chaves que as frases do pós-crash usam: sem valor no log, aparecem como "?". */
const LOG_PARAMS = [
  'description',
  'exception',
  'entity_type',
  'block_entity',
  'range',
  'actual',
  'dependency',
  'other_name',
  'package',
  'module',
  'other',
  'file',
  'other_loader',
  'provider',
  'needed',
  'found',
  'mixin_config',
  'size',
  'argument',
  'config',
  'class',
  'mod',
  'datapack',
  'thread',
  'crash_report',
];

/** Os parâmetros das frases de um achado. */
export function phraseParams(entry: ReportFinding): Record<string, string> {
  const params = entry.finding.params;
  const first = entry.items[0]?.name;
  const second = entry.items[1]?.name;
  const values: Record<string, string> = {};
  for (const key of LOG_PARAMS) values[key] = '?';
  Object.assign(values, params);
  const nome = first ?? params.mod_name ?? params.mod ?? params.file ?? '?';
  const dep = missingDependency(entry) ?? params.dependency_name ?? params.dependency ?? '?';
  return {
    ...values,
    nome,
    outro: second ?? params.other_name ?? params.other ?? '?',
    dep,
    modNome: params.mod_name ?? first ?? params.mod ?? '?',
    depNome: params.dependency_name ?? params.dependency ?? dep,
    other_name: params.other_name ?? second ?? params.other ?? '?',
    faixa: params.range ? ` (${params.range})` : '',
  };
}

/** A chave das frases de um achado (`regras.E_MISSING_DEP`, `achados.crashReport`). */
export function phraseKey(entry: ReportFinding): string | null {
  const { finding } = entry;
  const prefix = 'diagnostico.achados.';
  if (finding.titleKey.startsWith(prefix)) {
    const key = `achados.${finding.titleKey.slice(prefix.length)}`;
    if (exists(`${key}.titulo`)) return key;
  }
  const rule =
    finding.rule === 'E_JAVA' && finding.items.length === 0 ? 'E_JAVA_PACK' : finding.rule;
  return exists(`regras.${rule}.titulo`) ? `regras.${rule}` : null;
}

export function findingTitle(entry: ReportFinding): string {
  const key = phraseKey(entry);
  return key
    ? t(`${key}.titulo`, phraseParams(entry))
    : t('regraGenerica.titulo', { regra: entry.finding.rule });
}

export function findingText(entry: ReportFinding): string {
  const key = phraseKey(entry);
  return key ? t(`${key}.texto`, phraseParams(entry)) : t('regraGenerica.texto');
}

/** Uma evidência: a origem (arquivo, linha, campo) e o trecho exato. */
export interface EvidenceView {
  label: string;
  excerpt: string | null;
}

export function evidenceView(entry: ReportFinding, evidence: Evidence): EvidenceView {
  switch (evidence.kind) {
    case 'jarMetadata':
      return {
        label: t('evidencia.jar', {
          arquivo: evidence.fileInJar,
          item: refName(entry, evidence.item),
        }),
        excerpt: evidence.excerpt,
      };
    case 'api':
      return {
        label: t('evidencia.api', {
          fonte: t(`fontes.${evidence.source}`),
          campo: evidence.field,
        }),
        excerpt: t('evidencia.endereco', { url: evidence.url }),
      };
    case 'log':
      return {
        label: t('evidencia.log', {
          arquivo: evidence.file,
          linha: evidence.line.toLocaleString('pt-BR'),
        }),
        excerpt: evidence.excerpt,
      };
    case 'curated':
      return {
        label: t('evidencia.curada', { regra: evidence.ruleId }),
        excerpt: evidence.note,
      };
    case 'packFile':
      return {
        label: t('evidencia.arquivo', { caminho: evidence.path }),
        excerpt: evidence.excerpt,
      };
  }
}

/** O texto do botão de uma correção. */
export function fixLabel(entry: ReportFinding, fix: SuggestedFix): string {
  switch (fix.kind) {
    case 'addDependency':
      return t('correcao.adicionar', { nome: entry.labels[fix.modId] ?? fix.modId });
    case 'removeItem':
      return t('correcao.remover', { nome: refName(entry, fix.item) });
    case 'updateItem':
      return t('correcao.atualizar', { nome: refName(entry, fix.item) });
    case 'setSide':
      return t(`correcao.lado.${fix.side satisfies FindingSide}`);
    case 'changeJava':
      return fix.major === null
        ? t('correcao.java')
        : t('correcao.javaVersao', { major: fix.major });
    case 'changeMemory':
      return t('correcao.memoria');
    case 'removeJvmArgument':
      return t('correcao.argumento', { argumento: fix.argument });
    case 'restoreConfig':
      return t('correcao.config', { caminho: fix.path });
    case 'restoreDefault':
      return t('correcao.padrao', { chave: fix.key });
  }
}

/** Uma correção leva a um item que está no pack? (Remover, Atualizar e Mudar lado só valem
 * para itens do pack.) */
export function fixAvailable(entry: ReportFinding, fix: SuggestedFix): boolean {
  if (fix.kind === 'removeItem' || fix.kind === 'updateItem' || fix.kind === 'setSide') {
    return itemOf(entry, fix.item)?.path != null;
  }
  return true;
}

/** Quantos achados de cada gravidade valem (sem os ignorados). */
export function countBySeverity(findings: readonly ReportFinding[]): Record<Severity, number> {
  const counts: Record<Severity, number> = { error: 0, warning: 0, info: 0 };
  for (const entry of findings) {
    if (!entry.ignored) counts[entry.finding.severity] += 1;
  }
  return counts;
}

/** "há pouco", "há 3 minutos", "há 2 horas", "ontem". */
export function formatAgo(atMs: number, nowMs: number = Date.now()): string {
  const minutes = Math.round((nowMs - atMs) / 60_000);
  const format = new Intl.RelativeTimeFormat('pt-BR', { numeric: 'auto' });
  if (minutes < 1) return t('secao.agora');
  if (minutes < 60) return format.format(-minutes, 'minute');
  const hours = Math.round(minutes / 60);
  if (hours < 24) return format.format(-hours, 'hour');
  return format.format(-Math.round(hours / 24), 'day');
}
