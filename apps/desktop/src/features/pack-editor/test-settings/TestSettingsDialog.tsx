/**
 * Ajustes do teste neste computador (SPEC T11): memória, Java do teste (Automático, com o
 * motivo da escolha e "Por que não o Java N?", ou um Java instalado), argumentos extras da JVM
 * (com avisos) e "Recriar instância de teste". Ficam em `packs.json`, não entram no pack e não
 * contam como alteração não salva. Aberto pelo menu ▾ do Testar; a L-08 acrescenta os perfis.
 *
 * O Java automático vem de `java_choice` (política da L-01; ADR-0029): para o Forge 1.20.1,
 * "Automático: Java 17" com o porquê de não usar o mais novo; para a versão mais nova do
 * Minecraft, o Java mais novo, sem explicação extra (CA-T11-03).
 */
import { useQuery } from '@tanstack/react-query';
import { RotateCcw, TriangleAlert } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import {
  commands,
  type InstalledRuntime,
  type JavaChoice,
  type JavaChoiceRequest,
  type LoaderKind,
  type PackId,
  type PackRow,
  type TestMemory,
  type TestSettings,
  type TestSettingsView,
} from '../../../lib/ipc/bindings';
import { commandQuery } from '../../../lib/ipc/query';
import { useJavaRuntimes } from '../../settings/java/hooks/useJavaRuntimes';
import {
  choiceReason,
  decisionReason,
  reasonLabel,
  versionText,
  whyNotNewest,
} from '../../settings/java/lib/java-table';
import { editorKeys, useRecreateInstance, useSaveTestSettings, useTestSettings } from '../api';
import { checkJvmArgs, type JvmArgWarning } from './jvm-args';

/** Valores fixos oferecidos para a memória, em GB (os mesmos de Configurações → Teste). */
const MEMORY_GB = [4, 6, 8, 12, 16] as const;

/** Valor do `<select>` para "Automático" (memória e Java). */
const AUTO = 'auto';

const LOADERS: readonly LoaderKind[] = ['fabric', 'forge', 'neoforge', 'quilt', 'liteloader'];

/** O pedido de escolha do Java para o pack, com a escolha do usuário. */
export function javaRequest(pack: PackRow, userChoice: string | null): JavaChoiceRequest {
  const loader = LOADERS.find((kind) => kind === pack.loader) ?? 'vanilla';
  return {
    minecraft: pack.minecraft ?? '',
    loader,
    loaderVersion: loader === 'vanilla' ? null : pack.loaderVersion,
    versionJson: { kind: 'notLoaded' },
    userChoice,
  };
}

export interface TestSettingsDialogProps {
  packId: PackId;
  pack: PackRow;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function TestSettingsDialog({ packId, pack, open, onOpenChange }: TestSettingsDialogProps) {
  const { t } = useTranslation('editor');
  const view = useTestSettings(packId, open);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open ? (
        view.isSuccess ? (
          <SettingsForm
            pack={pack}
            view={view.data}
            onDone={() => {
              onOpenChange(false);
            }}
          />
        ) : (
          <DialogContent title={t('ajustesTeste.titulo')}>
            {view.isError ? (
              <ErrorPanel
                error={view.error}
                onRetry={() => {
                  void view.refetch();
                }}
              />
            ) : (
              <LoadingState inline label={t('ajustesTeste.carregando')} />
            )}
          </DialogContent>
        )
      ) : null}
    </Dialog>
  );
}

function memoryValue(memory: TestMemory): string {
  return memory.mode === 'auto' ? AUTO : String(memory.mb);
}

function SettingsForm({
  pack,
  view,
  onDone,
}: {
  pack: PackRow;
  view: TestSettingsView;
  onDone: () => void;
}) {
  const { t } = useTranslation('editor');
  const memoryId = useId();
  const javaId = useId();
  const argsId = useId();
  // Campos ausentes (gravados por outra versão) valem como o padrão.
  const [memory, setMemory] = useState<TestMemory>(view.settings.memory ?? { mode: 'auto' });
  const [java, setJava] = useState<string | null>(view.settings.java ?? null);
  const [jvmArgs, setJvmArgs] = useState(view.settings.jvmArgs ?? '');
  const save = useSaveTestSettings(pack.id);
  const runtimes = useJavaRuntimes();
  const choice = useQuery({
    ...commandQuery([...editorKeys.testSettings(pack.id), 'java', java ?? AUTO], () =>
      commands.javaChoice(javaRequest(pack, java)),
    ),
    staleTime: 0,
  });

  const installed: InstalledRuntime[] = runtimes.data?.runtimes.map((row) => row.runtime) ?? [];
  const major = choice.data?.major ?? null;
  const warnings = checkJvmArgs(jvmArgs, major);

  const memoryOptions: { value: string; label: string }[] = [
    { value: AUTO, label: t('ajustesTeste.memoriaAuto') },
    ...MEMORY_GB.map((gb) => ({
      value: String(gb * 1024),
      label: t('ajustesTeste.memoriaGb', { gb }),
    })),
  ];
  if (!memoryOptions.some((option) => option.value === memoryValue(memory))) {
    memoryOptions.push({
      value: memoryValue(memory),
      label: t('ajustesTeste.memoriaMb', { mb: memoryValue(memory) }),
    });
  }

  const submit = () => {
    const settings: TestSettings = { memory, java, jvmArgs };
    save.mutate(settings, {
      onSuccess: () => {
        showToast({ kind: 'ok', title: t('ajustesTeste.salvo') });
        onDone();
      },
    });
  };

  return (
    <DialogContent
      title={t('ajustesTeste.titulo')}
      description={t('ajustesTeste.descricao')}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('ajustesTeste.cancelar')}</Button>
          </DialogClose>
          <Button variant="primary" loading={save.isPending} onClick={submit}>
            {save.isPending ? t('ajustesTeste.salvando') : t('ajustesTeste.salvar')}
          </Button>
        </>
      }
    >
      <p className="t-xs t-3">{t('ajustesTeste.efeito')}</p>
      <div className="field">
        <label className="field__label" htmlFor={memoryId}>
          {t('ajustesTeste.memoria')}
        </label>
        <select
          id={memoryId}
          className="select"
          value={memoryValue(memory)}
          aria-describedby={`${memoryId}-hint`}
          onChange={(event) => {
            const value = event.target.value;
            setMemory(value === AUTO ? { mode: 'auto' } : { mode: 'fixed', mb: Number(value) });
          }}
        >
          {memoryOptions.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <div className="field__hint" id={`${memoryId}-hint`}>
          {t('ajustesTeste.memoriaDica')}
        </div>
      </div>

      <div className="field">
        <label className="field__label" htmlFor={javaId}>
          {t('ajustesTeste.java')}
        </label>
        <select
          id={javaId}
          className="select"
          value={java ?? AUTO}
          aria-describedby={`${javaId}-hint`}
          onChange={(event) => {
            const value = event.target.value;
            setJava(value === AUTO ? null : value);
          }}
        >
          <option value={AUTO}>
            {choice.data
              ? t('ajustesTeste.javaAuto', { major: choice.data.automatic.requirement.major })
              : t('ajustesTeste.memoriaAuto')}
          </option>
          {installed.map((runtime) => (
            <option key={runtime.id} value={runtime.id}>
              {t('ajustesTeste.javaInstalado', {
                version: versionText(runtime.version),
                source: t(`ajustesTeste.javaFonte.${runtime.source}`),
              })}
            </option>
          ))}
          {java !== null && !installed.some((runtime) => runtime.id === java) ? (
            <option value={java}>{java}</option>
          ) : null}
        </select>
        <div className="field__hint" id={`${javaId}-hint`}>
          <JavaExplanation
            choice={choice.data}
            loading={choice.isPending}
            failed={choice.isError}
            noneInstalled={runtimes.isSuccess && installed.length === 0}
          />
        </div>
      </div>

      <div className="field">
        <label className="field__label" htmlFor={argsId}>
          {t('ajustesTeste.argumentos')}{' '}
          <span className="opt">{t('ajustesTeste.argumentosOpcional')}</span>
        </label>
        <textarea
          id={argsId}
          className="textarea textarea--mono"
          rows={2}
          spellCheck={false}
          value={jvmArgs}
          aria-describedby={`${argsId}-hint${warnings.length > 0 ? ` ${argsId}-avisos` : ''}`}
          onChange={(event) => {
            setJvmArgs(event.target.value);
          }}
        />
        {warnings.length > 0 ? (
          <ul className="jvmwarn" id={`${argsId}-avisos`}>
            {warnings.map((warning, index) => (
              <li key={index}>
                <Icon icon={TriangleAlert} size="sm" />
                <span>{warningText(t, warning)}</span>
              </li>
            ))}
          </ul>
        ) : null}
        <div className="field__hint" id={`${argsId}-hint`}>
          {t('ajustesTeste.argumentosDica')}
        </div>
      </div>

      {save.isError ? <ErrorPanel compact error={save.error} /> : null}
      <hr className="sep" />
      <InstanceSection packId={pack.id} view={view} />
    </DialogContent>
  );
}

/** O motivo do Java escolhido (L-01) e, quando não é o mais novo, "Por que não o Java N?". */
function JavaExplanation({
  choice,
  loading,
  failed,
  noneInstalled,
}: {
  choice: JavaChoice | undefined;
  loading: boolean;
  failed: boolean;
  noneInstalled: boolean;
}) {
  const { t } = useTranslation('editor');
  const { t: tj } = useTranslation('java');
  if (failed) return <span>{t('ajustesTeste.javaSemDecisao')}</span>;
  if (loading || !choice) return <span>{t('ajustesTeste.javaCarregando')}</span>;
  const automatic = choice.automatic;
  const newest = whyNotNewest(automatic);
  return (
    <span className="javawhy">
      {choice.userChoiceUnavailable ? (
        <span className="t-warn">{tj('motivo.escolhaIndisponivel')}</span>
      ) : null}
      {choice.reason === 'USER_CHOICE' ? (
        <span>
          {reasonLabel(tj, choiceReason(choice))} {t('ajustesTeste.javaEscolhidoAtencao')}
        </span>
      ) : null}
      {newest !== null ? (
        <details className="disclosure">
          <summary>{tj('motivo.porQueNao', { newest })}</summary>
          <span className="t-sm">{reasonLabel(tj, decisionReason(automatic))}</span>
        </details>
      ) : null}
      {noneInstalled ? <span>{t('ajustesTeste.javaDicaSemInstalados')}</span> : null}
    </span>
  );
}

type EditorT = ReturnType<typeof useTranslation<'editor'>>['t'];

function warningText(t: EditorT, warning: JvmArgWarning): string {
  switch (warning.kind) {
    case 'memoria':
      return t('ajustesTeste.avisos.memoria', { arg: warning.arg });
    case 'leituraMemoria':
      return t('ajustesTeste.avisos.leituraMemoria', { arg: warning.arg });
    case 'removido':
      return t('ajustesTeste.avisos.removido', { arg: warning.arg, major: warning.major });
    case 'naoNoJava8':
      return t('ajustesTeste.avisos.naoNoJava8', { arg: warning.arg });
    case 'desconhecido':
      return t('ajustesTeste.avisos.desconhecido', { arg: warning.arg });
    case 'aspas':
      return t('ajustesTeste.avisos.aspas');
  }
}

/** "Recriar instância de teste" (apaga a instância; pergunta se mantém os mundos). */
function InstanceSection({ packId, view }: { packId: PackId; view: TestSettingsView }) {
  const { t } = useTranslation('editor');
  const recreate = useRecreateInstance(packId);
  const [keepWorlds, setKeepWorlds] = useState(true);
  return (
    <div className="stack-2">
      <div className="field__label">{t('ajustesTeste.instancia')}</div>
      <p className="t-sm t-2">
        {view.instanceExists
          ? t('ajustesTeste.instanciaTexto')
          : t('ajustesTeste.instanciaNaoExiste')}
      </p>
      <div className="btn-row">
        <ConfirmDialog
          title={t('ajustesTeste.recriarTitulo')}
          description={t('ajustesTeste.recriarTexto')}
          confirmLabel={t('ajustesTeste.recriarConfirmar')}
          confirmingLabel={t('ajustesTeste.recriando')}
          cancelLabel={t('ajustesTeste.cancelar')}
          trigger={
            <Button size="sm" icon={RotateCcw} disabled={!view.instanceExists}>
              {t('ajustesTeste.recriar')}
            </Button>
          }
          onConfirm={async () => {
            await recreate.mutateAsync(view.hasWorlds && keepWorlds);
            showToast({ kind: 'ok', title: t('ajustesTeste.recriada') });
          }}
        >
          {view.hasWorlds ? (
            // O texto do rótulo está no <span> dentro dele (padrão `.check` do design system).
            // eslint-disable-next-line jsx-a11y/label-has-associated-control
            <label className="check">
              <input
                type="checkbox"
                checked={keepWorlds}
                onChange={(event) => {
                  setKeepWorlds(event.target.checked);
                }}
              />
              <span className="check__text">
                <span>{t('ajustesTeste.manterMundos')}</span>
              </span>
            </label>
          ) : null}
        </ConfirmDialog>
      </div>
    </div>
  );
}
