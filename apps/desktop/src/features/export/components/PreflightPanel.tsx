/**
 * "1. Antes de exportar" (SPEC T19): higiene (a mesma de T04, com Revisar e limpar), erros do
 * diagnóstico (quando a D-01 informar) e alterações não salvas.
 */
import { CircleCheck, CircleX, TriangleAlert, type LucideIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import type { HygieneFinding, PackId, Preflight } from '../../../lib/ipc/bindings';
import { useHygieneFix } from '../../packs/api';
import { HygieneTable } from '../../packs/components/HygieneTable';

const LINE: Record<'ok' | 'bad' | 'warn', { className: string; icon: LucideIcon }> = {
  ok: { className: 'is-ok', icon: CircleCheck },
  bad: { className: 'is-bad', icon: CircleX },
  warn: { className: 'is-warn', icon: TriangleAlert },
};

function Line({ kind, children }: { kind: keyof typeof LINE; children: ReactNode }) {
  const { className, icon } = LINE[kind];
  return (
    <li className={className}>
      <Icon icon={icon} />
      <span>{children}</span>
    </li>
  );
}

export function PreflightPanel({
  packId,
  preflight,
  readOnly,
}: {
  packId: PackId;
  preflight: Preflight;
  readOnly: boolean;
}) {
  const { t } = useTranslation('exportar');
  const [cleaning, setCleaning] = useState(false);
  const hygiene = preflight.hygiene;
  const errors = preflight.diagnosticErrors;

  return (
    <section className="panel" aria-labelledby="ex-antes">
      <h2 className="panel__title panel__title--sans" id="ex-antes">
        {t('antes.titulo')}
      </h2>
      <ul className="checklist mt-3">
        {hygiene.length === 0 ? (
          <Line kind="ok">{t('antes.higieneOk')}</Line>
        ) : (
          <Line kind="bad">
            {t('antes.higiene', { count: hygiene.length })}{' '}
            {readOnly ? null : (
              <Button
                variant="link"
                onClick={() => {
                  setCleaning(true);
                }}
              >
                {t('antes.revisar')}
              </Button>
            )}
          </Line>
        )}
        {errors !== null && errors > 0 ? (
          <Line kind="bad">{t('antes.erros', { count: errors })}</Line>
        ) : null}
        {preflight.unsavedChanges ? (
          <Line kind="warn">{t('antes.naoSalvas')}</Line>
        ) : (
          <Line kind="ok">{t('antes.salvas')}</Line>
        )}
      </ul>
      <Dialog open={cleaning} onOpenChange={setCleaning}>
        {cleaning ? (
          <HygieneCleanup
            packId={packId}
            items={hygiene}
            onDone={() => {
              setCleaning(false);
            }}
          />
        ) : null}
      </Dialog>
    </section>
  );
}

/** A tabela da higiene com "Limpar N arquivos" (ponto de segurança antes, no backend). */
function HygieneCleanup({
  packId,
  items,
  onDone,
}: {
  packId: PackId;
  items: HygieneFinding[];
  onDone: () => void;
}) {
  const { t } = useTranslation(['exportar', 'packs', 'comum']);
  const fix = useHygieneFix();
  const [selected, setSelected] = useState<ReadonlySet<string>>(
    () => new Set(items.map((item) => item.path)),
  );
  return (
    <DialogContent
      size="lg"
      title={t('antes.limparTitulo')}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('acoes.cancelar', { ns: 'comum' })}</Button>
          </DialogClose>
          <Button
            variant="primary"
            loading={fix.isPending}
            disabled={selected.size === 0}
            onClick={() => {
              fix.mutate(
                { packId, paths: [...selected] },
                {
                  onSuccess: (deleted) => {
                    showToast({
                      kind: 'ok',
                      title: t('abrir.limpos', { ns: 'packs', count: deleted.length }),
                    });
                    onDone();
                  },
                },
              );
            }}
          >
            {fix.isPending
              ? t('pack.limpando', { ns: 'packs' })
              : t('pack.limpar', { ns: 'packs', count: selected.size })}
          </Button>
        </>
      }
    >
      <HygieneTable items={items} selected={selected} onSelectedChange={setSelected} />
      {fix.isError ? <ErrorPanel compact error={fix.error} /> : null}
    </DialogContent>
  );
}
