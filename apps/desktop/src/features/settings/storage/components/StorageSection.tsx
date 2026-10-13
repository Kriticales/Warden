/**
 * Configurações → Armazenamento (SPEC T21, P1; A-06): o espaço usado pelo cache de downloads,
 * pelos arquivos do Minecraft e dos loaders, pelos Javas e pelas instâncias de teste, e
 * "Limpar cache".
 *
 * A medição roda no Rust, fora da thread da interface; enquanto ela não chega, cada valor diz
 * "Medindo…". "Limpar cache" fica indisponível (com o motivo) enquanto houver jogo aberto ou
 * tarefa em andamento; o Rust recusa do mesmo jeito se o pedido chegar assim mesmo.
 */
import { Trash2 } from 'lucide-react';
import { Fragment, useId } from 'react';
import { useTranslation } from 'react-i18next';

import { useOperations } from '../../../../app/tasks/useOperations';
import { isFinished } from '../../../../app/tasks/operations';
import { ConfirmDialog } from '../../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../../components/common/ErrorPanel';
import { Button } from '../../../../components/ui/button';
import { showToast } from '../../../../components/ui/toast';
import { formatBytes } from '../../../../lib/format';
import type { StorageUsage } from '../../../../lib/ipc/bindings';
import { useTestStore } from '../../../test/store';
import { SettingsPanel } from '../../components/SettingsPanel';
import { useClearCache, useStorageUsage, useStorageUsageSync } from '../api';

const ITEMS = [
  ['cache', 'cache'],
  ['minecraft', 'minecraft'],
  ['javas', 'javas'],
  ['instances', 'instancias'],
] as const satisfies readonly (readonly [keyof StorageUsage, string])[];

export function StorageSection() {
  const { t } = useTranslation('armazenamento');
  useStorageUsageSync();
  const usage = useStorageUsage();
  return (
    <SettingsPanel title={t('titulo')}>
      <p className="t-sm t-2">{t('descricao')}</p>
      {usage.isError ? (
        <ErrorPanel
          compact
          error={usage.error}
          onRetry={() => {
            void usage.refetch();
          }}
        />
      ) : (
        <UsageList usage={usage.data ?? null} />
      )}
      <ClearCache cacheBytes={usage.data?.clearable ?? null} />
    </SettingsPanel>
  );
}

function UsageList({ usage }: { usage: StorageUsage | null }) {
  const { t } = useTranslation('armazenamento');
  return (
    <dl className="kv" aria-label={t('legenda')} aria-busy={usage === null}>
      {ITEMS.map(([field, label]) => (
        <Fragment key={field}>
          <dt>{t(`itens.${label}`)}</dt>
          <dd className={usage ? undefined : 't-3'}>
            {usage ? formatBytes(usage[field]) : t('medindo')}
          </dd>
        </Fragment>
      ))}
    </dl>
  );
}

/** Por que "Limpar cache" não está disponível agora, ou `null`. */
function useBlockedReason(): string | null {
  const { t } = useTranslation('armazenamento');
  const game = useTestStore((store) => store.game);
  const operations = useOperations();
  if (game && game.state !== 'exited') {
    return t('emJogo', { packName: game.packName });
  }
  const running = (operations.data ?? []).filter((operation) => !isFinished(operation)).length;
  if (running > 0) {
    return t('emTarefa', { count: running });
  }
  return null;
}

/** `cacheBytes`: o que a limpeza apaga (o banco de metadados, aberto pelo app, fica). */
function ClearCache({ cacheBytes }: { cacheBytes: number | null }) {
  const { t } = useTranslation('armazenamento');
  const hintId = useId();
  const clear = useClearCache();
  const blocked = useBlockedReason();
  const trigger = (
    <Button
      size="sm"
      icon={Trash2}
      disabled={blocked !== null}
      aria-describedby={blocked ? hintId : undefined}
    >
      {t('limpar')}
    </Button>
  );
  return (
    <div className="stack-2">
      <div className="btn-row">
        {blocked ? (
          trigger
        ) : (
          <ConfirmDialog
            trigger={trigger}
            title={t('confirmar.titulo')}
            description={
              cacheBytes === null
                ? t('confirmar.textoSemTamanho')
                : t('confirmar.texto', { size: formatBytes(cacheBytes) })
            }
            confirmLabel={t('limpar')}
            confirmingLabel={t('limpando')}
            cancelLabel={t('confirmar.cancelar')}
            onConfirm={async () => {
              const cleared = await clear.mutateAsync(undefined);
              showToast({
                kind: cleared.skipped > 0 ? 'warn' : 'ok',
                title: t('resultado.limpo', { size: formatBytes(cleared.freed) }),
                ...(cleared.skipped > 0
                  ? { text: t('resultado.emUso', { count: cleared.skipped }) }
                  : {}),
              });
            }}
          />
        )}
      </div>
      {blocked ? (
        <p className="field__hint" id={hintId}>
          {blocked}
        </p>
      ) : null}
    </div>
  );
}
