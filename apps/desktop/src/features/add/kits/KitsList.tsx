/**
 * "Kits de desempenho" (SPEC T08; ADR-0033): a lista que a página de descoberta mostra no início
 * (P1-16) e a que o pack aberto usa para aplicar um kit. Cada kit mostra os mods que traz, com
 * caixas; "Adicionar kit" abre o **diálogo de dependências** (T09) com os itens marcados, então
 * nada entra em silêncio.
 */
import { ChevronDown, ChevronRight, Plus } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import type { AddResult, Kit, Loader, PackId } from '../../../lib/ipc/bindings';
import { DependenciesDialog, type DependenciesRequest } from '../dependencies/DependenciesDialog';
import { loaderName } from '../../packs/lib/pack-list';
import { useKits } from './api';
import { KitItems } from './KitItems';
import { addChoices, defaultSelection, selectedItems, toggled } from './model';

export interface KitsListProps {
  packId: PackId;
  minecraft: string;
  loader: Loader | null;
  /** Depois de gravar o kit pelo diálogo. */
  onAdded?: (result: AddResult) => void;
}

export function KitsList({ packId, minecraft, loader, onAdded }: KitsListProps) {
  const { t } = useTranslation('modsIniciais');
  const kits = useKits(minecraft, loader);
  const [request, setRequest] = useState<DependenciesRequest | null>(null);
  const headingId = useId();

  return (
    <section aria-labelledby={headingId} className="stack-2">
      <h2 className="group-title" id={headingId}>
        {t('kits.titulo')}
      </h2>
      <p className="t-sm t-2">{t('kits.texto')}</p>
      {kits.isPending ? (
        <LoadingState label={t('kits.carregando')} />
      ) : kits.isError ? (
        <ErrorPanel error={kits.error} />
      ) : kits.data.length === 0 ? (
        <p className="t-sm t-3">
          {t('kits.nenhum', {
            minecraft,
            loader: loader === null ? '' : loaderName(loader),
          })}
        </p>
      ) : (
        <ul className="dlist">
          {kits.data.map((kit) => (
            <KitRow
              key={kit.id}
              kit={kit}
              onAdd={(choices, titles) => {
                setRequest({ choices, titles });
              }}
            />
          ))}
        </ul>
      )}
      <DependenciesDialog
        packId={packId}
        request={request}
        target={{ minecraft, loader }}
        onClose={() => {
          setRequest(null);
        }}
        onAdded={(result) => {
          setRequest(null);
          onAdded?.(result);
        }}
      />
    </section>
  );
}

function KitRow({
  kit,
  onAdd,
}: {
  kit: Kit;
  onAdd: (choices: ReturnType<typeof addChoices>, titles: string[]) => void;
}) {
  const { t } = useTranslation('modsIniciais');
  const [open, setOpen] = useState(false);
  const [selection, setSelection] = useState(() => defaultSelection(kit));
  const listId = useId();
  const chosen = selectedItems(kit, selection);
  return (
    <li className="drow drow--compact">
      <span className="drow__main">
        <span className="drow__line">
          <b>{kit.name}</b>
        </span>
        <span className="drow__desc">{kit.description}</span>
        <span className="drow__meta">
          <Button
            variant="link"
            icon={open ? ChevronDown : ChevronRight}
            aria-expanded={open}
            aria-controls={listId}
            onClick={() => {
              setOpen(!open);
            }}
          >
            {open ? t('kit.esconder') : t('kit.ver')}
          </Button>
          <span>{t('kits.itensMarcados', { count: chosen.length })}</span>
        </span>
        <span id={listId} hidden={!open}>
          {open ? (
            <KitItems
              kit={kit}
              selection={selection}
              onToggle={(projectId) => {
                setSelection(toggled(selection, projectId));
              }}
            />
          ) : null}
        </span>
      </span>
      <span className="drow__end">
        <Button
          icon={Plus}
          disabled={chosen.length === 0}
          title={chosen.length === 0 ? t('kits.semItens') : undefined}
          onClick={() => {
            onAdd(
              addChoices(kit, selection),
              chosen.map((item) => item.name),
            );
          }}
        >
          {t('kits.adicionarKit')}
        </Button>
      </span>
    </li>
  );
}
