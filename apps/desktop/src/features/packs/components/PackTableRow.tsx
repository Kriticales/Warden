/**
 * Uma linha de Meus packs (`packRow()` do design system). Três formas: pack pronto (Abrir e
 * menu ⋯), pasta não encontrada (Localizar… e Remover da lista; CA-T02-02) e pack ilegível
 * (Ver detalhes, Mostrar na pasta e Remover da lista).
 */
import { useNavigate } from '@tanstack/react-router';
import {
  CircleCheck,
  CircleHelp,
  CircleX,
  Ellipsis,
  FolderOpen,
  Lock,
  Trash2,
  X,
  type LucideIcon,
} from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { NameTile } from '../../../components/common/PixelArt';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import {
  Menu,
  MenuContent,
  MenuItem,
  MenuSeparator,
  MenuIconTrigger,
} from '../../../components/ui/menu';
import { showToast } from '../../../components/ui/toast';
import { Tooltip } from '../../../components/ui/tooltip';
import type { PackRow } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { dayRelation, formatDate, formatTime } from '../../../lib/format';
import { useChooseFolder, useRelocatePack, useRevealPack } from '../api';
import { lastTestKind, loaderName, type LastTestKind } from '../lib/pack-list';
import type { RowDialog } from './PacksPage';

interface RowProps {
  row: PackRow;
  onDialog: (dialog: RowDialog) => void;
}

export function PackTableRow({ row, onDialog }: RowProps) {
  switch (row.status) {
    case 'ready':
      return <ReadyRow row={row} onDialog={onDialog} />;
    case 'folderMissing':
      return <MissingRow row={row} onDialog={onDialog} />;
    case 'invalidPack':
      return <InvalidRow row={row} onDialog={onDialog} />;
  }
}

/** "Minecraft 1.20.1 · Forge 47.3.0" (ou "· vanilla"). */
export function useVersionsLine(row: Pick<PackRow, 'minecraft' | 'loader' | 'loaderVersion'>) {
  const { t } = useTranslation('packs');
  const minecraft = t('lista.minecraft', { version: row.minecraft ?? '?' });
  const loader = row.loader
    ? t('lista.loader', { loader: loaderName(row.loader), version: row.loaderVersion ?? '' })
    : t('lista.vanilla');
  return `${minecraft} · ${loader.trim()}`;
}

const TEST_STATUS: Record<
  LastTestKind,
  {
    kind: 'ok' | 'danger' | 'muted';
    icon: LucideIcon;
    key: 'abriu' | 'travou' | 'nunca' | 'desconhecido';
  }
> = {
  ok: { kind: 'ok', icon: CircleCheck, key: 'abriu' },
  crashed: { kind: 'danger', icon: CircleX, key: 'travou' },
  never: { kind: 'muted', icon: CircleHelp, key: 'nunca' },
  unknown: { kind: 'muted', icon: CircleHelp, key: 'desconhecido' },
};

function LastTest({ value }: { value: string | null }) {
  const { t } = useTranslation('packs');
  const status = TEST_STATUS[lastTestKind(value)];
  return (
    <span className={`status status--${status.kind}`}>
      <Icon icon={status.icon} />
      {t(`teste.${status.key}`)}
    </span>
  );
}

function When({ ms }: { ms: number | null }) {
  const { t } = useTranslation('packs');
  const [now] = useState(() => Date.now());
  if (ms === null) return <>{t('lista.semData')}</>;
  const relation = dayRelation(ms, now);
  if (relation === 'today') return <>{t('lista.hoje', { time: formatTime(ms) })}</>;
  if (relation === 'yesterday') return <>{t('lista.ontem', { time: formatTime(ms) })}</>;
  return <>{formatDate(ms)}</>;
}

function toastError(error: unknown) {
  showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
}

function ReadyRow({ row, onDialog }: RowProps) {
  const { t } = useTranslation('packs');
  const navigate = useNavigate();
  const reveal = useRevealPack();
  const versions = useVersionsLine(row);
  const open = () => {
    void navigate({ to: '/packs/$packId', params: { packId: row.id } });
  };
  return (
    <tr className="packrow">
      <td>
        <div className="packrow__name">
          <NameTile seed={row.name} size="lg" />
          <div>
            <button type="button" className="packrow__title" onClick={open}>
              {row.name}
            </button>
            <div className="packrow__sub t-xs t-3">
              <span>{versions}</span>
              {row.readOnlyReason ? (
                <Tooltip content={t('lista.soLeituraDica', { motivo: row.readOnlyReason })}>
                  <button type="button" className="tag tag--warn">
                    <Icon icon={Lock} />
                    {t('lista.soLeitura')}
                  </button>
                </Tooltip>
              ) : null}
            </div>
          </div>
        </div>
      </td>
      <td className="t-mono">{row.version ?? t('lista.semData')}</td>
      <td>
        <LastTest value={row.lastTest} />
      </td>
      <td className="num">
        <span
          className={row.unsavedFiles > 0 ? 't-warn' : 't-3'}
          aria-label={t('lista.naoSalvas', { count: row.unsavedFiles })}
        >
          {row.unsavedFiles}
        </span>
      </td>
      <td className="t-3">
        <When ms={row.modifiedAtMs} />
      </td>
      <td className="shrink">
        <div className="row">
          <Button
            size="sm"
            variant="primary"
            aria-label={t('lista.abrirNome', { name: row.name })}
            onClick={open}
          >
            {t('lista.abrir')}
          </Button>
          <Menu>
            <MenuIconTrigger icon={Ellipsis} label={t('lista.maisAcoes', { name: row.name })} />
            <MenuContent>
              <MenuItem
                icon={FolderOpen}
                onSelect={() => {
                  reveal.mutate(row.id, { onError: toastError });
                }}
              >
                {t('menu.mostrarNaPasta')}
              </MenuItem>
              <MenuItem
                icon={X}
                description={t('menu.removerDaListaDesc')}
                onSelect={() => {
                  onDialog({ kind: 'forget', row });
                }}
              >
                {t('menu.removerDaLista')}
              </MenuItem>
              <MenuSeparator />
              <MenuItem
                icon={Trash2}
                danger
                description={t('menu.apagarDesc')}
                onSelect={() => {
                  onDialog({ kind: 'trash', row });
                }}
              >
                {t('menu.apagar')}
              </MenuItem>
            </MenuContent>
          </Menu>
        </div>
      </td>
    </tr>
  );
}

function MissingRow({ row, onDialog }: RowProps) {
  const { t } = useTranslation('packs');
  const choose = useChooseFolder();
  const relocate = useRelocatePack();
  const busy = choose.isPending || relocate.isPending;
  const locate = () => {
    choose.mutate('relocate', {
      onError: toastError,
      onSuccess: (path) => {
        if (path === null) return;
        relocate.mutate(
          { packId: row.id, path },
          {
            onSuccess: () => {
              showToast({ kind: 'ok', title: t('localizar.feito', { name: row.name }) });
            },
            onError: (error) => {
              const appError = toAppError(error);
              const otherPack =
                appError.code.domain === 'project' && appError.params.reason === 'otherPack';
              showToast({
                kind: 'danger',
                title: otherPack
                  ? t('localizar.outroPack', { name: row.name })
                  : appErrorMessage(appError),
              });
            },
          },
        );
      },
    });
  };
  return (
    <tr className="packrow packrow--missing">
      <td>
        <div className="packrow__name">
          <NameTile seed={row.name} size="lg" />
          <div>
            <span className="t-strong">{row.name}</span>
            <div className="t-xs t-danger">{t('linha.pastaSumiu', { path: row.path })}</div>
          </div>
        </div>
      </td>
      <td colSpan={4} className="t-3">
        {t('linha.pastaSumiuMotivo')}
      </td>
      <td className="shrink">
        <div className="row row--wrap">
          <Button size="sm" loading={busy} onClick={locate}>
            {busy ? t('linha.localizando') : t('linha.localizar')}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              onDialog({ kind: 'forget', row });
            }}
          >
            {t('linha.removerDaLista')}
          </Button>
        </div>
      </td>
    </tr>
  );
}

function InvalidRow({ row, onDialog }: RowProps) {
  const { t } = useTranslation('packs');
  const reveal = useRevealPack();
  return (
    <tr className="packrow packrow--missing">
      <td>
        <div className="packrow__name">
          <NameTile seed={row.name || row.path} size="lg" />
          <div>
            <span className="t-strong">{row.name || row.path}</span>
            <div className="t-xs t-danger">{t('linha.ilegivel')}</div>
          </div>
        </div>
      </td>
      <td colSpan={4} className="t-3">
        {t('linha.ilegivelMotivo')}
      </td>
      <td className="shrink">
        <div className="row row--wrap">
          <Button
            size="sm"
            onClick={() => {
              onDialog({ kind: 'details', row });
            }}
          >
            {t('linha.verDetalhes')}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              reveal.mutate(row.id, { onError: toastError });
            }}
          >
            {t('linha.mostrarNaPasta')}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              onDialog({ kind: 'forget', row });
            }}
          >
            {t('linha.removerDaLista')}
          </Button>
        </div>
      </td>
    </tr>
  );
}
