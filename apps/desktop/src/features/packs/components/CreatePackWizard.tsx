/**
 * Criar pack (SPEC T03; protótipo `criar-1` a `criar-5`): assistente com Nome e pasta, Versão
 * do Minecraft, Loader, Mods iniciais (P1-18) e Resumo, num diálogo por cima de Meus packs
 * (decisão D38, relato do dono na issue #1). Esc, ✕, clique fora e Cancelar fecham; com algo
 * preenchido, pedem confirmação antes. Ao fechar, o foco volta ao botão que abriu.
 *
 * Nada é escrito antes de "Criar pack": a etapa 1 só confere o destino (`pack_create_check`,
 * CA-T03-04) e as versões vêm do catálogo (`catalog_*`), que o Rust confere de novo ao criar.
 * Os mods iniciais escolhidos entram num ponto único, depois do "Pack criado"
 * (`initial_mods_apply`, o mesmo caminho de Adicionar): se não entrarem, o pack já existe e o
 * aviso diz como adicioná-los pela página Mods.
 */
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Plus } from 'lucide-react';
import { useCallback, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Steps } from '../../../components/common/Steps';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import { showToast, type ToastInput } from '../../../components/ui/toast';
import type { Loader, PackId } from '../../../lib/ipc/bindings';
import { commandError } from '../../../lib/ipc/query';
import { createCheckQuery, useCreatePack } from '../api';
import { InitialModsStep } from '../create/initial-mods/InitialModsStep';
import { useInitialModsApply } from '../create/initial-mods/api';
import { type InitialChoice, toRequest } from '../create/initial-mods/model';
import { LoaderStep } from './create/LoaderStep';
import { MinecraftStep } from './create/MinecraftStep';
import { NameStep } from './create/NameStep';
import { SummaryStep } from './create/SummaryStep';
import { checkProblem, type Draft } from './create/draft';
import '../packs.css';

const STEP_KEYS = ['nome', 'minecraft', 'loader', 'mods', 'resumo'] as const;
type StepKey = (typeof STEP_KEYS)[number];

const EMPTY: Draft = {
  name: '',
  author: '',
  description: '',
  destination: null,
  minecraft: null,
  loader: null,
  loaderVersion: null,
  loaderReady: false,
  initial: null,
};

/** O rascunho tem algo que se perderia ao fechar. */
function isDirty(draft: Draft, step: number): boolean {
  return (
    step > 0 ||
    draft.name.trim() !== '' ||
    draft.author.trim() !== '' ||
    draft.description.trim() !== '' ||
    draft.destination !== null
  );
}

export interface CreatePackWizardProps {
  /** Fecha o diálogo (volta para Meus packs). */
  onClose: () => void;
  /** Para onde o foco volta ao fechar, se o botão que abriu não estava focado. */
  returnFocusTo: () => HTMLElement | null;
}

export function CreatePackWizard({ onClose, returnFocusTo }: CreatePackWizardProps) {
  const { t } = useTranslation('packs');
  const { t: tMods } = useTranslation('modsIniciais');
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const create = useCreatePack();
  const applyInitial = useInitialModsApply();
  const [finishing, setFinishing] = useState(false);
  const [step, setStep] = useState(0);
  const [draft, setDraft] = useState<Draft>(EMPTY);
  const [checking, setChecking] = useState(false);
  const [nameError, setNameError] = useState<unknown>(null);
  const [confirmClose, setConfirmClose] = useState(false);
  const formId = useId();
  const key: StepKey = STEP_KEYS[step] ?? 'nome';

  const setInitial = useCallback((initial: InitialChoice) => {
    setDraft((current) => ({ ...current, initial }));
  }, []);

  const update = (patch: Partial<Draft>) => {
    setDraft((current) => ({ ...current, ...patch }));
    if ('name' in patch || 'destination' in patch) setNameError(null);
  };

  const canAdvance = (() => {
    switch (key) {
      case 'nome':
        return draft.name.trim() !== '' && !checking;
      case 'minecraft':
        return draft.minecraft !== null;
      case 'loader':
        return draft.loaderReady && (draft.loader === null || draft.loaderVersion !== null);
      case 'mods':
        // Espera a oferta chegar (ou falhar): a etapa preenche a escolha com o padrão. Seguir
        // antes disso criaria o pack sem os mods que a tela mostra marcados.
        return draft.loader === null || draft.initial !== null;
      case 'resumo':
        return !create.isPending && !finishing;
    }
  })();

  const busy = create.isPending || finishing;

  /** Esc, ✕, clique fora ou Cancelar: com algo preenchido, confirma antes de descartar. */
  const requestClose = () => {
    if (busy) return;
    if (isDirty(draft, step)) setConfirmClose(true);
    else onClose();
  };

  const goBack = () => {
    if (step === 0) {
      requestClose();
    } else {
      setStep(step - 1);
    }
  };

  const goNext = async () => {
    if (key === 'nome') {
      // Confere de novo agora: a pasta pode ter ganhado arquivos desde a última conferência.
      setChecking(true);
      try {
        await queryClient.query(createCheckQuery(draft.name, draft.destination));
        setStep(step + 1);
      } catch (error) {
        setNameError(error);
      } finally {
        setChecking(false);
      }
      return;
    }
    if (key === 'resumo') {
      submit();
      return;
    }
    setStep(step + 1);
  };

  const submit = () => {
    if (draft.minecraft === null) return;
    create.mutate(
      {
        name: draft.name.trim(),
        author: draft.author.trim(),
        description: draft.description.trim(),
        destination: draft.destination,
        minecraft: draft.minecraft,
        loader: draft.loader,
        loaderVersion: draft.loader === null ? null : draft.loaderVersion,
      },
      {
        onSuccess: (created) => {
          void finish(created.id);
        },
        onError: (error) => {
          // Destino que ganhou arquivos depois da etapa 1: volta para ela com o motivo.
          if (checkProblem(commandError(error)) !== null) {
            setNameError(error);
            setStep(0);
          }
        },
      },
    );
  };

  /**
   * Depois do "Pack criado": grava os mods iniciais e abre o pack. Se eles não entrarem, o pack
   * já existe; o aviso diz isso e o pack abre do mesmo jeito.
   */
  const finish = async (packId: PackId) => {
    const name = draft.name.trim();
    const request = draft.initial === null ? null : toRequest(draft.initial);
    let toast: ToastInput = { kind: 'ok', title: t('criar.feito', { name }) };
    if (request !== null) {
      setFinishing(true);
      try {
        const result = await applyInitial.mutateAsync({ packId, request });
        toast = {
          kind: result.leftOut.length > 0 ? 'warn' : 'ok',
          title: t('criar.feito', { name }),
          text:
            result.leftOut.length > 0
              ? tMods('resultado.ficaramDeFora', { nomes: result.leftOut.join(', ') })
              : tMods('resultado.feito', { count: result.added.length }),
        };
      } catch {
        toast = {
          kind: 'warn',
          title: t('criar.feito', { name }),
          text: tMods('resultado.falhou'),
        };
      } finally {
        setFinishing(false);
      }
    }
    showToast(toast);
    void navigate({ to: '/packs/$packId', params: { packId } });
  };

  const stepNames = STEP_KEYS.map((name) => t(`criar.etapa.${name}`));
  const submitError =
    create.isError && checkProblem(commandError(create.error)) === null ? create.error : null;

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) requestClose();
      }}
    >
      <DialogContent
        size="lg"
        title={t('criar.titulo')}
        returnFocusTo={returnFocusTo}
        footerSplit
        footer={
          <>
            <Button variant="ghost" onClick={goBack} disabled={busy}>
              {step === 0 ? t('criar.cancelar') : t('criar.voltar')}
            </Button>
            {key === 'resumo' ? (
              <Button
                type="submit"
                form={formId}
                variant="primary"
                icon={Plus}
                loading={busy}
                disabled={!canAdvance}
              >
                {busy ? t('criar.criando') : t('criar.criar')}
              </Button>
            ) : (
              <Button
                type="submit"
                form={formId}
                variant="primary"
                loading={checking}
                disabled={!canAdvance}
              >
                {checking ? t('criar.conferindo') : t('criar.proximo')}
              </Button>
            )}
          </>
        }
      >
        <div className="newpack__steps">
          <Steps items={stepNames} current={step} label={t('criar.etapas')} />
        </div>
        <form
          id={formId}
          className="newpack__form"
          noValidate
          onSubmit={(event) => {
            event.preventDefault();
            if (canAdvance) void goNext();
          }}
        >
          {key === 'nome' ? (
            <NameStep draft={draft} onChange={update} submitError={nameError} />
          ) : null}
          {key === 'minecraft' ? (
            <MinecraftStep
              value={draft.minecraft}
              onChange={(minecraft) => {
                if (minecraft !== draft.minecraft) {
                  update({
                    minecraft,
                    loader: null,
                    loaderVersion: null,
                    loaderReady: false,
                    initial: null,
                  });
                }
              }}
            />
          ) : null}
          {key === 'loader' && draft.minecraft !== null ? (
            <LoaderStep
              minecraft={draft.minecraft}
              loader={draft.loader}
              version={draft.loaderVersion}
              ready={draft.loaderReady}
              onChange={(loader: Loader | null, loaderVersion: string | null) => {
                update({ loader, loaderVersion, loaderReady: true, initial: null });
              }}
            />
          ) : null}
          {key === 'mods' && draft.minecraft !== null ? (
            <InitialModsStep
              minecraft={draft.minecraft}
              loader={draft.loader}
              choice={draft.initial}
              onChange={setInitial}
            />
          ) : null}
          {key === 'resumo' ? <SummaryStep draft={draft} /> : null}
          {submitError !== null ? <ErrorPanel error={submitError} className="mt-4" /> : null}
        </form>
      </DialogContent>
      <ConfirmDialog
        open={confirmClose}
        onOpenChange={setConfirmClose}
        title={t('criar.descartar.titulo')}
        description={t('criar.descartar.texto')}
        confirmLabel={t('criar.descartar.confirmar')}
        cancelLabel={t('criar.descartar.continuar')}
        onConfirm={onClose}
      />
    </Dialog>
  );
}
