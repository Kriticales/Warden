/**
 * Criar pack (SPEC T03; protótipo `criar-1` a `criar-5`): assistente com Nome e pasta, Versão
 * do Minecraft, Loader e Resumo. A etapa "Mods iniciais" é da P1-18, que a acrescenta em
 * `STEP_KEYS` entre Loader e Resumo (ROADMAP P1-07, D4).
 *
 * Nada é escrito antes de "Criar pack": a etapa 1 só confere o destino (`pack_create_check`,
 * CA-T03-04) e as versões vêm do catálogo (`catalog_*`), que o Rust confere de novo ao criar.
 */
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Plus } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { PageHead } from '../../../app/layout/PageHead';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Steps } from '../../../components/common/Steps';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { Loader } from '../../../lib/ipc/bindings';
import { commandError } from '../../../lib/ipc/query';
import { createCheckQuery, useCreatePack } from '../api';
import { LoaderStep } from './create/LoaderStep';
import { MinecraftStep } from './create/MinecraftStep';
import { NameStep } from './create/NameStep';
import { SummaryStep } from './create/SummaryStep';
import { checkProblem, type Draft } from './create/draft';
import '../packs.css';

const STEP_KEYS = ['nome', 'minecraft', 'loader', 'resumo'] as const;
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
};

export function CreatePackWizard() {
  const { t } = useTranslation('packs');
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const create = useCreatePack();
  const [step, setStep] = useState(0);
  const [draft, setDraft] = useState<Draft>(EMPTY);
  const [checking, setChecking] = useState(false);
  const [nameError, setNameError] = useState<unknown>(null);
  const key: StepKey = STEP_KEYS[step] ?? 'nome';

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
      case 'resumo':
        return !create.isPending;
    }
  })();

  const goBack = () => {
    if (step === 0) {
      void navigate({ to: '/packs' });
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
          showToast({ kind: 'ok', title: t('criar.feito', { name: draft.name.trim() }) });
          void navigate({ to: '/packs/$packId', params: { packId: created.id } });
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

  const stepNames = STEP_KEYS.map((name) => t(`criar.etapa.${name}`));
  const submitError =
    create.isError && checkProblem(commandError(create.error)) === null ? create.error : null;

  return (
    <AppPage back={{ to: '/packs', label: t('pack.voltar') }} where={t('criar.onde')}>
      <div className="wizard">
        <PageHead title={t('criar.titulo')} />
        <Steps items={stepNames} current={step} label={t('criar.etapas')} />
        <form
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
                  update({ minecraft, loader: null, loaderVersion: null, loaderReady: false });
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
                update({ loader, loaderVersion, loaderReady: true });
              }}
            />
          ) : null}
          {key === 'resumo' ? <SummaryStep draft={draft} /> : null}
          {submitError !== null ? <ErrorPanel error={submitError} className="mt-4" /> : null}
          <div className="wizard__foot">
            <Button variant="ghost" onClick={goBack} disabled={create.isPending}>
              {step === 0 ? t('criar.cancelar') : t('criar.voltar')}
            </Button>
            {key === 'resumo' ? (
              <Button
                type="submit"
                variant="primary"
                icon={Plus}
                loading={create.isPending}
                disabled={!canAdvance}
              >
                {create.isPending ? t('criar.criando') : t('criar.criar')}
              </Button>
            ) : (
              <Button type="submit" variant="primary" loading={checking} disabled={!canAdvance}>
                {checking ? t('criar.conferindo') : t('criar.proximo')}
              </Button>
            )}
          </div>
        </form>
      </div>
    </AppPage>
  );
}
