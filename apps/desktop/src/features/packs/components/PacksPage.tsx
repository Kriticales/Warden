/**
 * Meus packs (SPEC T02; protótipo `packs`): a tela inicial do app. Uma linha por pack com
 * nome, versão do Minecraft e loader, versão do pack, último teste, alterações não salvas e a
 * data da última alteração; Abrir e o menu ⋯ (Mostrar na pasta, Remover da lista, Apagar
 * pack…). Linhas especiais para pasta não encontrada (Localizar…) e pack ilegível (Ver
 * detalhes). A coluna Saúde fica para a D-08.
 *
 * A lista vem do registro (`packs_list`) sem rodar diagnóstico; nada aqui escreve na pasta de
 * um pack, exceto "Apagar pack", que só manda a pasta para a Lixeira depois de digitar o nome.
 */
import { useNavigate } from '@tanstack/react-router';
import { FolderOpen, Plus, Search } from 'lucide-react';
import { useDeferredValue, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { AppPage } from '../../../app/layout/AppPage';
import { PageHead } from '../../../app/layout/PageHead';
import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Skeleton } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import type { PackRow } from '../../../lib/ipc/bindings';
import { usePacksList, useForgetPack, useTrashPack } from '../api';
import { useOpenOrImport } from '../hooks/useOpenOrImport';
import { filterRows, SORT_ORDERS, sortRows, type SortOrder } from '../lib/pack-list';
import { PackDetailsDialog } from './PackDetailsDialog';
import { PackTableRow } from './PackTableRow';
import '../packs.css';

/** Diálogo aberto a partir de uma linha. */
export type RowDialog =
  | { kind: 'forget'; row: PackRow }
  | { kind: 'trash'; row: PackRow }
  | { kind: 'details'; row: PackRow };

const ORDER_LABEL = { modified: 'alterados', name: 'nome', lastTest: 'teste' } as const;

export function PacksPage() {
  const { t } = useTranslation('packs');
  const list = usePacksList();
  const navigate = useNavigate();
  const openOrImport = useOpenOrImport();

  const openButton = (
    <Tooltip content={t('lista.abrirOuImportarDica')}>
      <Button icon={FolderOpen} loading={openOrImport.pending} onClick={openOrImport.start}>
        {openOrImport.pending ? t('lista.escolhendoPasta') : t('lista.abrirOuImportar')}
      </Button>
    </Tooltip>
  );
  const createButton = (
    <Button
      variant="primary"
      icon={Plus}
      onClick={() => {
        void navigate({ to: '/packs/novo' });
      }}
    >
      {t('lista.criar')}
    </Button>
  );
  const actions = (
    <>
      {openButton}
      {createButton}
    </>
  );

  let body;
  if (list.isPending) {
    body = (
      <>
        <PageHead title={t('lista.titulo')} sub={t('lista.lendo')} actions={actions} />
        <LoadingTable />
      </>
    );
  } else if (list.isError) {
    body = (
      <>
        <PageHead title={t('lista.titulo')} actions={actions} />
        <ErrorPanel
          error={list.error}
          onRetry={() => {
            void list.refetch();
          }}
        />
      </>
    );
  } else if (list.data.length === 0) {
    body = (
      <>
        <PageHead title={t('lista.titulo')} />
        <EmptyState
          glyph="plus"
          title={t('lista.vazio.titulo')}
          text={t('lista.vazio.texto')}
          actions={
            <>
              {createButton}
              {openButton}
            </>
          }
        />
      </>
    );
  } else {
    body = (
      <>
        <PageHead
          title={t('lista.titulo')}
          sub={t('lista.contagem', { count: list.data.length })}
          actions={actions}
        />
        <PackList rows={list.data} />
      </>
    );
  }

  return <AppPage>{body}</AppPage>;
}

function PackList({ rows }: { rows: PackRow[] }) {
  const { t } = useTranslation('packs');
  const [query, setQuery] = useState('');
  const [order, setOrder] = useState<SortOrder>('modified');
  const [dialog, setDialog] = useState<RowDialog | null>(null);
  const deferredQuery = useDeferredValue(query);
  const visible = sortRows(filterRows(rows, deferredQuery), order);
  const unreadable = rows.filter((row) => row.status === 'invalidPack').length;

  return (
    <>
      {unreadable > 0 ? (
        <Alert
          kind="danger"
          className="packs-banner"
          title={t('lista.ilegiveis', { count: unreadable })}
        >
          <div className="alert__text">{t('lista.ilegiveisTexto')}</div>
        </Alert>
      ) : null}
      <div className="packs-tools">
        <div className="inputwrap">
          <Icon icon={Search} />
          <input
            className="input"
            type="search"
            value={query}
            placeholder={t('lista.buscar')}
            aria-label={t('lista.buscar')}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
        </div>
        <select
          className="select"
          aria-label={t('lista.ordenar')}
          value={order}
          onChange={(event) => {
            const next = SORT_ORDERS.find((value) => value === event.target.value);
            if (next) setOrder(next);
          }}
        >
          {SORT_ORDERS.map((value) => (
            <option key={value} value={value}>
              {t(`lista.ordem.${ORDER_LABEL[value]}`)}
            </option>
          ))}
        </select>
      </div>
      {visible.length === 0 ? (
        <EmptyState
          compact
          glyph="search"
          title={t('lista.semResultado.titulo', { busca: deferredQuery.trim() })}
          text={t('lista.semResultado.texto')}
          actions={
            <Button
              size="sm"
              onClick={() => {
                setQuery('');
              }}
            >
              {t('lista.semResultado.limpar')}
            </Button>
          }
        />
      ) : (
        <div className="tablewrap">
          <table className="table">
            <caption className="sr-only">{t('lista.legenda')}</caption>
            <TableHead />
            <tbody>
              {visible.map((row) => (
                <PackTableRow key={row.id} row={row} onDialog={setDialog} />
              ))}
            </tbody>
          </table>
        </div>
      )}
      <RowDialogs
        dialog={dialog}
        onClose={() => {
          setDialog(null);
        }}
      />
    </>
  );
}

function TableHead() {
  const { t } = useTranslation('packs');
  return (
    <thead>
      <tr>
        <th scope="col">{t('lista.coluna.pack')}</th>
        <th scope="col">{t('lista.coluna.versao')}</th>
        <th scope="col">{t('lista.coluna.ultimoTeste')}</th>
        <th scope="col" className="num">
          {t('lista.coluna.naoSalvas')}
        </th>
        <th scope="col">{t('lista.coluna.alterado')}</th>
        <th scope="col" className="shrink">
          <span className="sr-only">{t('lista.coluna.acoes')}</span>
        </th>
      </tr>
    </thead>
  );
}

/** Esqueleto com a forma da tabela enquanto o registro é lido. */
function LoadingTable() {
  const { t } = useTranslation('packs');
  return (
    <>
      <div className="packs-tools" aria-hidden="true">
        <Skeleton shape="btn" width="300px" />
      </div>
      <div className="tablewrap">
        <table className="table" aria-busy="true">
          <caption className="sr-only">{t('lista.lendo')}</caption>
          <TableHead />
          <tbody>
            {['50%', '60%', '70%'].map((width) => (
              <tr key={width} aria-hidden="true">
                <td>
                  <div className="row row--gap-3">
                    <Skeleton shape="tile" />
                    <div className="grow">
                      <Skeleton width={width} />
                      <Skeleton width="70%" />
                    </div>
                  </div>
                </td>
                {['60%', '80%', '30%', '50%'].map((cell) => (
                  <td key={cell}>
                    <Skeleton width={cell} />
                  </td>
                ))}
                <td>
                  <Skeleton shape="btn" width="90px" />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}

function RowDialogs({ dialog, onClose }: { dialog: RowDialog | null; onClose: () => void }) {
  const { t } = useTranslation('packs');
  const forget = useForgetPack();
  const trash = useTrashPack();
  const onOpenChange = (open: boolean) => {
    if (!open) onClose();
  };
  if (dialog === null) return null;
  const { row } = dialog;
  switch (dialog.kind) {
    case 'forget':
      return (
        <ConfirmDialog
          open
          onOpenChange={onOpenChange}
          destructive={false}
          title={t('remover.titulo', { name: row.name })}
          description={t('remover.texto')}
          confirmLabel={t('remover.confirmar')}
          confirmingLabel={t('remover.confirmando')}
          onConfirm={async () => {
            await forget.mutateAsync(row.id);
            showToast({ kind: 'ok', title: t('remover.feito', { name: row.name }) });
          }}
        />
      );
    case 'trash':
      return (
        <ConfirmDialog
          open
          onOpenChange={onOpenChange}
          title={t('apagar.titulo', { name: row.name })}
          description={t('apagar.texto')}
          confirmLabel={t('apagar.confirmar')}
          confirmingLabel={t('apagar.confirmando')}
          confirmText={row.name}
          onConfirm={async () => {
            await trash.mutateAsync({ packId: row.id, confirmation: row.name });
            showToast({ kind: 'ok', title: t('apagar.feito', { name: row.name }) });
          }}
        />
      );
    case 'details':
      return <PackDetailsDialog row={row} onOpenChange={onOpenChange} />;
  }
}
