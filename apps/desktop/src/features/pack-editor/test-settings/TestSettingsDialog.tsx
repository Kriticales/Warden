/**
 * Ajustes do teste neste computador (SPEC T11): memória, Java do teste (Automático, com o
 * motivo da escolha e "Por que não o Java N?", ou um Java instalado), argumentos extras da JVM
 * (com avisos) e "Recriar instância de teste". Ficam em `packs.json`, não entram no pack e não
 * contam como alteração não salva. Aberto pelo menu ▾ do Testar.
 *
 * Perfis do teste (L-08): no topo, o seletor Perfil (Padrão e os criados) com Novo perfil,
 * Renomear e Apagar. Cada perfil tem memória, Java, argumentos, mods opcionais, janela e "Ao
 * abrir o jogo". O diálogo guarda um rascunho por perfil (trocar de perfil no seletor não perde
 * o que foi mudado) e "Salvar ajustes" grava todos os que mudaram. Memória acima de 75% da RAM
 * gera aviso; argumentos que impedem a leitura de memória (`W_PERF_FLAGS`) ganham "Remover
 * estes argumentos".
 *
 * O Java automático vem de `java_choice` (política da L-01; ADR-0029): para o Forge 1.20.1,
 * "Automático: Java 17" com o porquê de não usar o mais novo; para a versão mais nova do
 * Minecraft, o Java mais novo, sem explicação extra (CA-T11-03).
 */
import { useQuery } from '@tanstack/react-query';
import { Eraser, RotateCcw, TriangleAlert } from 'lucide-react';
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
  type ProfileFields,
  type TestMemory,
  type TestProfile,
  type TestProfilesView,
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
import { useSaveProfiles, useTestProfiles } from '../../test/profiles/api';
import { DEFAULT_PROFILE, memoryAboveRam, removePerfFlags } from '../../test/profiles/text';
import { formatMemory } from '../../test/text';
import { editorKeys, useRecreateInstance, useTestSettings } from '../api';
import { OptionalChoicesSection } from '../item-options/OptionalChoicesSection';
import { checkJvmArgs, type JvmArgWarning } from './jvm-args';
import { OnLaunchField, WindowField } from './LaunchFields';
import { ProfileOptionals } from './ProfileOptionals';
import { ProfilePicker } from './ProfilePicker';

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
  const profiles = useTestProfiles(packId, open);
  const failed = view.isError ? view : profiles.isError ? profiles : null;
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open ? (
        view.isSuccess && profiles.isSuccess ? (
          <SettingsForm
            pack={pack}
            view={view.data}
            profiles={profiles.data}
            onDone={() => {
              onOpenChange(false);
            }}
          />
        ) : (
          <DialogContent title={t('ajustesTeste.titulo')}>
            {failed ? (
              <ErrorPanel
                error={failed.error}
                onRetry={() => {
                  void view.refetch();
                  void profiles.refetch();
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

/** Os perfis do servidor com os rascunhos do diálogo por cima (perfis novos entram, apagados
 * saem; o nome vem sempre do servidor, que é onde Renomear grava). */
function mergeDrafts(
  server: TestProfile[],
  drafts: Partial<Record<string, TestProfile>>,
): TestProfile[] {
  return server.map((profile) => {
    const draft = drafts[profile.id];
    return draft ? { ...draft, name: profile.name } : profile;
  });
}

/** Os campos de um perfil (sem id e nome). */
function fieldsOf(profile: TestProfile): ProfileFields {
  return {
    memory: profile.memory,
    java: profile.java,
    jvmArgs: profile.jvmArgs,
    optionals: profile.optionals,
    window: profile.window,
    onLaunch: profile.onLaunch,
  };
}

function SettingsForm({
  pack,
  view,
  profiles,
  onDone,
}: {
  pack: PackRow;
  view: TestSettingsView;
  profiles: TestProfilesView;
  onDone: () => void;
}) {
  const { t } = useTranslation('editor');
  const { t: tp } = useTranslation('perfis');
  const memoryId = useId();
  const javaId = useId();
  const argsId = useId();
  const [drafts, setDrafts] = useState<Partial<Record<string, TestProfile>>>({});
  const [editing, setEditing] = useState(profiles.active);
  const list = mergeDrafts(profiles.profiles, drafts);
  // O Padrão sempre existe (é o primeiro): vale quando o editado foi apagado por fora.
  const current = list.find((profile) => profile.id === editing) ?? list[0];
  const save = useSaveProfiles(pack.id);
  const runtimes = useJavaRuntimes();
  // Campos ausentes (gravados por outra versão) valem como o padrão.
  const memory: TestMemory = current?.memory ?? { mode: 'auto' };
  const java = current?.java ?? null;
  const jvmArgs = current?.jvmArgs ?? '';
  const choice = useQuery({
    ...commandQuery([...editorKeys.testSettings(pack.id), 'java', java ?? AUTO], () =>
      commands.javaChoice(javaRequest(pack, java)),
    ),
    staleTime: 0,
  });
  if (!current) return null;

  const update = (change: Partial<ProfileFields>) => {
    setDrafts((previous) => ({ ...previous, [current.id]: { ...current, ...change } }));
  };

  const installed: InstalledRuntime[] = runtimes.data?.runtimes.map((row) => row.runtime) ?? [];
  const major = choice.data?.major ?? null;
  const warnings = checkJvmArgs(jvmArgs, major);
  const blocksMemoryReading = warnings.some((warning) => warning.kind === 'leituraMemoria');
  const memoryHigh = memoryAboveRam(memory, profiles.computerMemoryMb);

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
    const changed = list.filter((profile) => drafts[profile.id] !== undefined);
    save.mutate(changed, {
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
      <p className="t-xs t-3">{tp('efeito')}</p>
      <ProfilePicker
        packId={pack.id}
        profiles={list}
        active={profiles.active}
        editing={current.id}
        onEdit={setEditing}
        draft={fieldsOf(current)}
        onCreated={(profile) => {
          setEditing(profile.id);
        }}
        onDeleted={(id) => {
          setDrafts((previous) => ({ ...previous, [id]: undefined }));
          setEditing(DEFAULT_PROFILE);
        }}
      />
      <hr className="sep" />
      <div className="field">
        <label className="field__label" htmlFor={memoryId}>
          {t('ajustesTeste.memoria')}
        </label>
        <select
          id={memoryId}
          className="select"
          value={memoryValue(memory)}
          aria-describedby={`${memoryId}-hint${memoryHigh ? ` ${memoryId}-aviso` : ''}`}
          onChange={(event) => {
            const value = event.target.value;
            update({
              memory: value === AUTO ? { mode: 'auto' } : { mode: 'fixed', mb: Number(value) },
            });
          }}
        >
          {memoryOptions.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        {memoryHigh && profiles.computerMemoryMb !== null ? (
          <ul className="jvmwarn" id={`${memoryId}-aviso`}>
            <li>
              <Icon icon={TriangleAlert} size="sm" />
              <span>{tp('memoriaAlta', { total: formatMemory(profiles.computerMemoryMb) })}</span>
            </li>
          </ul>
        ) : null}
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
            update({ java: value === AUTO ? null : value });
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

      <OnLaunchField
        packId={pack.id}
        value={current.onLaunch}
        onChange={(onLaunch) => {
          update({ onLaunch });
        }}
        quickPlay={profiles.quickPlay}
        minecraft={profiles.minecraft}
      />
      <WindowField
        value={current.window}
        onChange={(window) => {
          update({ window });
        }}
      />

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
            update({ jvmArgs: event.target.value });
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
        {blocksMemoryReading ? (
          <div className="btn-row">
            <Button
              size="sm"
              icon={Eraser}
              onClick={() => {
                update({ jvmArgs: removePerfFlags(jvmArgs) });
              }}
            >
              {tp('removerArgumentos')}
            </Button>
          </div>
        ) : null}
        <div className="field__hint" id={`${argsId}-hint`}>
          {t('ajustesTeste.argumentosDica')}
        </div>
      </div>

      {save.isError ? <ErrorPanel compact error={save.error} /> : null}
      <hr className="sep" />
      {current.id === DEFAULT_PROFILE ? (
        <OptionalChoicesSection packId={pack.id} />
      ) : (
        <ProfileOptionals
          packId={pack.id}
          value={current.optionals}
          onChange={(optionals) => {
            update({ optionals });
          }}
        />
      )}
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
