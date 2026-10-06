/**
 * Etapa 2, "Versão do Minecraft" (T03): as versões *release* na ordem oficial da Mojang (da
 * mais nova para a mais antiga; nunca por comparação de texto), com busca. Antes da 1.7.10,
 * "melhor esforço". Snapshots não aparecem na v1. Lista guardada de quando a fonte não
 * respondeu: "Lista de versões de <data>". Sem internet e sem lista guardada: o erro com
 * "Tentar de novo".
 */
import { RotateCcw, Search } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../../../components/common/EmptyState';
import { ProgressBar } from '../../../../components/common/ProgressBar';
import { Alert } from '../../../../components/ui/alert';
import { Button } from '../../../../components/ui/button';
import { Icon } from '../../../../components/ui/icon';
import type { MinecraftVersions } from '../../../../lib/ipc/bindings';
import { appErrorMessage } from '../../../../lib/ipc/errors';
import { commandError } from '../../../../lib/ipc/query';
import { formatDate } from '../../../../lib/format';
import { useMinecraftVersions } from '../../api';

export interface MinecraftStepProps {
  value: string | null;
  onChange: (minecraft: string) => void;
}

export function MinecraftStep({ value, onChange }: MinecraftStepProps) {
  const { t } = useTranslation('packs');
  const { t: tc } = useTranslation();
  const versions = useMinecraftVersions();

  // A versão mais nova vem marcada, para o Próximo já funcionar.
  const latest = versions.data?.latestRelease ?? null;
  useEffect(() => {
    if (value === null && latest !== null) onChange(latest);
  }, [value, latest, onChange]);

  if (versions.isPending) {
    return (
      <div className="wizard__body">
        <ProgressBar value={null} label={t('criar.minecraft.carregando')} />
      </div>
    );
  }
  if (versions.isError) {
    const error = commandError(versions.error);
    const offline = error?.code.domain === 'core' && error.code.code === 'NETWORK_UNAVAILABLE';
    return (
      <EmptyState
        kind="error"
        glyph="x"
        compact
        title={t('criar.minecraft.erroTitulo')}
        text={
          offline || error === null ? t('criar.minecraft.erroSemInternet') : appErrorMessage(error)
        }
        actions={
          <Button
            icon={RotateCcw}
            onClick={() => {
              void versions.refetch();
            }}
          >
            {tc('acoes.tentarDeNovo')}
          </Button>
        }
      />
    );
  }
  return <VersionList data={versions.data} value={value} onChange={onChange} />;
}

function VersionList({
  data,
  value,
  onChange,
}: {
  data: MinecraftVersions;
  value: string | null;
  onChange: (minecraft: string) => void;
}) {
  const { t } = useTranslation('packs');
  const [query, setQuery] = useState('');
  const name = useId();
  const needle = query.trim();
  const releases = data.versions.filter((version) => version.kind === 'release');
  const shown = needle === '' ? releases : releases.filter((v) => v.id.includes(needle));
  return (
    <div className="wizard__body">
      {data.freshness.offline ? (
        <Alert
          kind="info"
          compact
          className="mb-3"
          title={t('criar.minecraft.cache', { data: formatDate(data.freshness.fetchedAtMs) })}
        >
          <div className="alert__text">{t('criar.minecraft.cacheTexto')}</div>
        </Alert>
      ) : null}
      <div className="inputwrap verlist__search">
        <Icon icon={Search} />
        <input
          className="input"
          type="search"
          value={query}
          placeholder={t('criar.minecraft.buscar')}
          aria-label={t('criar.minecraft.buscar')}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
        />
      </div>
      <fieldset className="fieldset-bare">
        <legend className="sr-only">{t('criar.minecraft.legenda')}</legend>
        <div className="verlist">
          {shown.length === 0 ? (
            <p className="verlist__note t-sm t-3">
              {t('criar.minecraft.nenhuma', { busca: needle })}
            </p>
          ) : (
            shown.map((version) => (
              <label key={version.id} className="check">
                <input
                  type="radio"
                  name={name}
                  value={version.id}
                  checked={value === version.id}
                  onChange={() => {
                    onChange(version.id);
                  }}
                />
                <span className="check__text">
                  <span className="t-mono">{version.id}</span>
                  {version.bestEffort ? (
                    <span className="check__desc">{t('criar.minecraft.melhorEsforco')}</span>
                  ) : null}
                </span>
              </label>
            ))
          )}
          <p className="verlist__note t-xs t-3">{t('criar.minecraft.antigas')}</p>
        </div>
      </fieldset>
      <p className="field__hint mt-2">{t('criar.minecraft.dica')}</p>
    </div>
  );
}
