/**
 * Aviso passageiro do cabeçalho: arquivos que não devem ir para os jogadores (higiene do T04),
 * que o "Agora não" de Abrir pack deixou para depois. Abre um diálogo com a mesma tabela e o
 * "Limpar N arquivos" (com ponto de segurança antes). Fica aqui até a seção Exportar existir
 * (E-01), que passa a mostrar a verificação antes de exportar.
 */
import { TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import type { HygieneFinding, PackId } from '../../../lib/ipc/bindings';
import { useHygieneFix, useHygieneScan } from '../../packs/api';
import { HygieneTable } from '../../packs/components/HygieneTable';
import type { HeaderSlotProps } from '../header/slots';

export function HygieneAlert({ packId, pack }: HeaderSlotProps) {
  const { t } = useTranslation('editor');
  const [open, setOpen] = useState(false);
  const scan = useHygieneScan(packId, pack.readOnlyReason === null);
  if (!scan.isSuccess || scan.data.length === 0) {
    return null;
  }
  return (
    <>
      <button
        type="button"
        className="headalert headalert--warn"
        onClick={() => {
          setOpen(true);
        }}
      >
        <Icon icon={TriangleAlert} size="sm" />
        {t('cabecalho.higiene', { count: scan.data.length })}
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        {open ? (
          <HygieneCleanup
            key={scan.dataUpdatedAt}
            packId={packId}
            items={scan.data}
            onDone={() => {
              setOpen(false);
            }}
          />
        ) : null}
      </Dialog>
    </>
  );
}

function HygieneCleanup({
  packId,
  items,
  onDone,
}: {
  packId: PackId;
  items: HygieneFinding[];
  onDone: () => void;
}) {
  const { t } = useTranslation(['editor', 'packs']);
  const fix = useHygieneFix();
  const [selected, setSelected] = useState<ReadonlySet<string>>(
    () => new Set(items.map((item) => item.path)),
  );
  return (
    <DialogContent
      size="lg"
      title={t('cabecalho.higiene', { count: items.length })}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('ajustesTeste.cancelar')}</Button>
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
