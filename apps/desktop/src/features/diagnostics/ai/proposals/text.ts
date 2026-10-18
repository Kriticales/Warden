/**
 * Textos derivados das propostas e das consultas da D-14 (glossário da QUALITY §8): o título
 * de cada cartão, o nome do lado, o título dos blocos "Enviado à IA" das ferramentas da D-14 e
 * o rótulo dos selos de evidência que vêm delas (issue, changelog, raio-x, busca do culpado).
 * Os dois últimos entram pelos ganchos de `../text.ts`.
 */
import i18n from '../../../../i18n';
import type {
  ItemSide,
  Proposal,
  SideChoice,
  TestSettingsValues,
  ToolStep,
} from '../../../../lib/ipc/bindings';

const tt = (key: string, options?: Record<string, unknown>): string =>
  (i18n.t as unknown as (key: string, options?: Record<string, unknown>) => string)(
    `iaPropostas:${key}`,
    options,
  );

/** "Cliente e servidor", "Só cliente", "Só servidor". */
export function sideLabel(side: ItemSide | SideChoice): string {
  return tt(`lado.${side}`);
}

/** O título do cartão ("Atualizar Supplementaries 2.8.17 → 2.8.21"). */
export function proposalTitle(proposal: Proposal): string {
  const { summary, change } = proposal;
  const nome = summary.target ?? '';
  const de = summary.from ?? '';
  const para = summary.to ?? '';
  switch (change.kind) {
    case 'addMod':
      return para
        ? tt('titulo.addModVersao', { nome, versao: para })
        : tt('titulo.addMod', { nome });
    case 'removeMod':
      return tt('titulo.removeMod', { nome });
    case 'updateMod':
      return tt('titulo.updateMod', { nome, de, para });
    case 'changeVersion':
      return tt('titulo.changeVersion', { nome, para });
    case 'setSide':
      return tt('titulo.setSide', { nome, lado: sideLabel(change.side) });
    case 'editConfig':
      return tt('titulo.editConfig', { chave: nome, de, para });
    case 'testSettings':
      return tt('titulo.testSettings');
    case 'runTest':
      return tt('titulo.runTest');
    case 'startBisect':
      return tt('titulo.startBisect');
  }
}

/** "Memória do teste: 6144 MB · Java 21 · Argumentos da JVM: nenhum". */
export function settingsLine(values: TestSettingsValues): string {
  const memory =
    values.memory.mode === 'fixed'
      ? tt('ajustes.memoria', { valor: values.memory.mb })
      : tt('ajustes.memoriaAuto');
  const java =
    values.javaMajor === null
      ? tt('ajustes.javaAuto')
      : tt('ajustes.java', { versao: values.javaMajor });
  const args = values.jvmArgs.trim()
    ? tt('ajustes.argumentos', { valor: values.jvmArgs })
    : tt('ajustes.semArgumentos');
  return [memory, java, args].join(' · ');
}

function text(value: unknown): string {
  return typeof value === 'string' || typeof value === 'number' ? String(value) : '';
}

/** O título do bloco "Enviado à IA" das ferramentas da D-14, ou `null` se não for uma delas. */
export function d14StepLabel(step: ToolStep, args: Record<string, unknown>): string | null {
  const mod = text(args.mod_id);
  if (step.name.startsWith('propose_')) {
    const tipo = `tipos.${step.name}`;
    return tt('enviado.proposta', {
      tipo: i18n.exists(`iaPropostas:${tipo}`) ? tt(tipo) : step.name,
    });
  }
  switch (step.name) {
    case 'search_mod_issues':
      return tt('enviado.issues', { mod, busca: text(args.query) });
    case 'get_mod_changelog':
      return text(args.from) && text(args.to)
        ? tt('enviado.changelogEntre', { mod, de: text(args.from), para: text(args.to) })
        : tt('enviado.changelog', { mod });
    case 'get_mixin_report':
      return mod ? tt('enviado.raioX', { mod }) : tt('enviado.raioXPack');
    case 'get_bisect_result':
      return tt('enviado.busca');
    default:
      return null;
  }
}

/** De onde veio o trecho, para as evidências das consultas da D-14, ou `null`. */
export function d14EvidenceLabel(kind: string, value: string): string | null {
  switch (kind) {
    case 'issue': {
      const number = /#(\d+)$/.exec(value)?.[1];
      return number ? tt('selo.issue', { numero: `#${number}` }) : tt('selo.issues');
    }
    case 'changelog': {
      const version = value.split('@')[1];
      return version ? tt('selo.changelog', { versao: version }) : tt('selo.changelogSemVersao');
    }
    case 'mixin':
      return tt('selo.raioX');
    case 'bisect': {
      const round = /#R(\d+)$/.exec(value)?.[1];
      return round ? tt('selo.rodada', { numero: round }) : tt('selo.busca');
    }
    default:
      return null;
  }
}
