/**
 * Lógica da tabela de Java de Configurações → Teste (SPEC T21; CA-T21-04) e dos motivos da
 * escolha do Java (ADR-0029). Funções puras, sem React: recebem o que o backend devolve
 * (`java_runtimes_list`, `java_choice`) e dizem o que mostrar.
 *
 * Os motivos viram uma chave do catálogo `java` (`motivo.*`) com os parâmetros. Ajustes do
 * teste (P1-08) usa `decisionReason` e `whyNotNewest` para "Automático: Java 17" e "Por que não
 * o Java 25?".
 */
import type {
  InstalledRuntime,
  JavaChoice,
  JavaDecision,
  JavaOverview,
  JavaVersion,
  PackJavaUse,
} from '../../../../lib/ipc/bindings';

/** Uma frase do catálogo `java` com os parâmetros. */
export interface ReasonText {
  key: string;
  params: Record<string, string | number>;
}

/** Versão curta: `8u312` no Java 8, `21.0.12.1` nos outros (igual ao `Display` do Rust). */
export function versionText(version: JavaVersion): string {
  if (version.major <= 8) {
    return `${String(version.major)}u${String(version.security)}`;
  }
  const base = `${String(version.major)}.${String(version.minor)}.${String(version.security)}`;
  return version.patch > 0 ? `${base}.${String(version.patch)}` : base;
}

/** O motivo da decisão automática. */
export function decisionReason(decision: JavaDecision): ReasonText {
  switch (decision.reason) {
    case 'NEWEST_PROVEN_FOR_RANGE':
      return {
        key: 'motivo.NEWEST_PROVEN_FOR_RANGE',
        params: {
          range: decision.rangeLabel ?? '',
          major: decision.requirement.major,
        },
      };
    case 'FROM_VERSION_JSON':
      return decision.versionJsonMajor === null
        ? { key: 'motivo.FROM_VERSION_JSON_ANTIGA', params: {} }
        : { key: 'motivo.FROM_VERSION_JSON', params: { required: decision.versionJsonMajor } };
    case 'USER_CHOICE':
    case 'FORGE_LEGACY_JAVA8':
    case 'FORGE_1165_OLD':
    case 'NEWEST_AVAILABLE':
      return { key: `motivo.${decision.reason}`, params: {} };
  }
}

/** O motivo de uma escolha (a do usuário, ou a automática). */
export function choiceReason(choice: JavaChoice): ReasonText {
  if (choice.reason === 'USER_CHOICE') {
    return { key: 'motivo.USER_CHOICE', params: {} };
  }
  return decisionReason(choice.automatic);
}

/**
 * "Por que não o Java N?": o N quando o Java automático não é o mais novo que o Warden usa;
 * `null` quando é (a versão mais nova do Minecraft não ganha explicação extra; CA-T11-03).
 */
export function whyNotNewest(decision: JavaDecision): number | null {
  return decision.requirement.major < decision.newestMajor ? decision.newestMajor : null;
}

/** Situação de uma linha da tabela. */
export type RowState = 'emUso' | 'semUso' | 'substituido' | 'naoBaixado';

/** Uma linha da tabela: um Java instalado, ou um Java que ainda será baixado. */
export interface JavaTableRow {
  /** Chave estável da linha (id do Java, ou `baixar-<major>-<teto>`). */
  key: string;
  major: number;
  /** O Java instalado; `null` quando ainda será baixado. */
  runtime: InstalledRuntime | null;
  /** Os packs, em ordem alfabética. */
  packs: PackJavaUse[];
  /** Os motivos distintos dos packs desta linha (mesma ordem dos packs). */
  reasons: ReasonText[];
  /** Um jogo aberto está usando este Java. */
  inGame: boolean;
  state: RowState;
  /** Versão do Java que substituiu este numa atualização. */
  supersededBy: string | null;
}

function sameReason(a: ReasonText, b: ReasonText): boolean {
  if (a.key !== b.key) return false;
  const keys = Object.keys(a.params);
  return (
    keys.length === Object.keys(b.params).length &&
    keys.every((name) => a.params[name] === b.params[name])
  );
}

function distinctReasons(packs: PackJavaUse[]): ReasonText[] {
  const reasons: ReasonText[] = [];
  for (const pack of packs) {
    const reason = choiceReason(pack.choice);
    if (!reasons.some((existing) => sameReason(existing, reason))) {
      reasons.push(reason);
    }
  }
  return reasons;
}

function byName(a: PackJavaUse, b: PackJavaUse): number {
  return a.name.localeCompare(b.name, 'pt-BR', { sensitivity: 'base' });
}

function compareVersions(a: JavaVersion, b: JavaVersion): number {
  return (
    a.major - b.major ||
    a.minor - b.minor ||
    a.security - b.security ||
    a.patch - b.patch ||
    a.build - b.build
  );
}

/**
 * As linhas da tabela: os Javas instalados e, para os packs cujo Java ainda não foi baixado,
 * uma linha por Java que falta. Ordem: Java mais novo primeiro.
 */
export function buildRows(overview: JavaOverview): JavaTableRow[] {
  const installed = overview.runtimes.map<JavaTableRow>((row) => {
    const packs = [...row.packs].sort(byName);
    const replacement = row.runtime.supersededBy
      ? overview.runtimes.find((other) => other.runtime.id === row.runtime.supersededBy)
      : undefined;
    let state: RowState;
    if (row.runtime.supersededBy) {
      state = 'substituido';
    } else if (packs.length > 0 || row.inGame) {
      state = 'emUso';
    } else {
      state = 'semUso';
    }
    return {
      key: row.runtime.id,
      major: row.runtime.version.major,
      runtime: row.runtime,
      packs,
      reasons: distinctReasons(packs),
      inGame: row.inGame,
      state,
      supersededBy: row.runtime.supersededBy
        ? replacement
          ? versionText(replacement.runtime.version)
          : row.runtime.supersededBy
        : null,
    };
  });

  const missing = new Map<string, JavaTableRow>();
  for (const pack of overview.packsToDownload) {
    const key = `baixar-${String(pack.choice.major)}-${String(pack.choice.maxUpdate ?? 'ultima')}`;
    const row = missing.get(key) ?? {
      key,
      major: pack.choice.major,
      runtime: null,
      packs: [],
      reasons: [],
      inGame: false,
      state: 'naoBaixado' as const,
      supersededBy: null,
    };
    row.packs.push(pack);
    missing.set(key, row);
  }
  for (const row of missing.values()) {
    row.packs.sort(byName);
    row.reasons = distinctReasons(row.packs);
  }

  return [...installed, ...missing.values()].sort((a, b) => {
    if (b.major !== a.major) return b.major - a.major;
    // No mesmo major: instalado antes do que falta; entre instalados, o mais novo primeiro.
    if (a.runtime && b.runtime) return compareVersions(b.runtime.version, a.runtime.version);
    if (a.runtime) return -1;
    if (b.runtime) return 1;
    return a.key.localeCompare(b.key);
  });
}

/** Quantos Javas "Remover Javas sem uso" apagaria (sem pack e sem jogo aberto). */
export function unusedCount(overview: JavaOverview): number {
  return overview.runtimes.filter((row) => row.packs.length === 0 && !row.inGame).length;
}

/** Os nomes dos packs, juntos para uma frase ("A, B e C"). */
export function joinNames(names: string[]): string {
  return new Intl.ListFormat('pt-BR', { style: 'long', type: 'conjunction' }).format(names);
}
