/**
 * Linha de item da lista de Mods (`design/system/components.js` → `modRow`): seleção, ícone e
 * nome (abre os detalhes), descrição curta, versão legível, fonte e lado editável na linha.
 * Arquivo inválido: o nome do arquivo, o erro curto e as ações Ver erro e Abrir no editor de
 * texto; os outros itens não são afetados (SPEC T06).
 */
import { FileCode } from 'lucide-react';
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { NameTile } from '../../../components/common/PixelArt';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { cn } from '../../../lib/cn';
import type { InventoryItem, ItemSide, SideChoice } from '../../../lib/ipc/bindings';
import { ItemMarks, SourceTag } from './marks';

export const SIDE_CHOICES: readonly SideChoice[] = ['both', 'client', 'server'];

/** Valor do `<select>` de lado quando o arquivo tem um lado que o packwiz não conhece. */
export const UNKNOWN_SIDE: ItemSide = 'unknown';

export interface ModRowProps {
  item: InventoryItem;
  selected: boolean;
  /** Índice da linha na tabela (para `aria-rowindex` quando a lista é virtualizada). */
  rowIndex?: number | undefined;
  sideBusy: boolean;
  onToggle: (path: string) => void;
  onOpen: (path: string) => void;
  onSide: (path: string, side: SideChoice) => void;
  onShowError: (item: InventoryItem) => void;
  onOpenFile: (path: string) => void;
}

export const ModRow = memo(function ModRow({
  item,
  selected,
  rowIndex,
  sideBusy,
  onToggle,
  onOpen,
  onSide,
  onShowError,
  onOpenFile,
}: ModRowProps) {
  const { t } = useTranslation('editor');
  const invalid = item.state === 'invalid';
  const label = invalid ? (item.fileName ?? item.path) : item.name;
  return (
    <tr
      className={cn(
        'modrow',
        invalid && 'modrow--invalid modrow--error',
        item.state === 'outsideIndex' && 'modrow--warn',
      )}
      aria-selected={selected}
      aria-rowindex={rowIndex}
      data-path={item.path}
    >
      <td className="modrow__check">
        <label className="check">
          <input
            type="checkbox"
            checked={selected}
            aria-label={t('mods.tabela.selecionarNome', { name: label })}
            onChange={() => {
              onToggle(item.path);
            }}
          />
        </label>
      </td>
      <td>
        <div className="modrow__name">
          {invalid ? (
            <span className="tile modrow__badtile" aria-hidden="true">
              <Icon icon={FileCode} />
            </span>
          ) : (
            <NameTile seed={item.name} />
          )}
          <div className="modrow__titles">
            <span className="modrow__line">
              <button
                type="button"
                className="modrow__title"
                onClick={() => {
                  onOpen(item.path);
                }}
              >
                {invalid ? item.path : item.name}
              </button>
              <ItemMarks item={item} />
              {invalid ? (
                <span className="modrow__actions">
                  <Button
                    variant="link"
                    size="sm"
                    onClick={() => {
                      onShowError(item);
                    }}
                  >
                    {t('mods.acoesInvalido.verErro')}
                  </Button>
                  <Button
                    variant="link"
                    size="sm"
                    onClick={() => {
                      onOpenFile(item.path);
                    }}
                  >
                    {t('mods.acoesInvalido.abrirEditor')}
                  </Button>
                </span>
              ) : null}
            </span>
            <span className="modrow__desc">{rowDescription(item, t)}</span>
          </div>
        </div>
      </td>
      <td>
        <span className="modrow__ver">
          {item.version ?? (invalid ? '' : t('mods.tabela.semVersao'))}
        </span>
      </td>
      <td>{invalid ? null : <SourceTag source={item.source} />}</td>
      <td>
        {item.sideEditable ? (
          <select
            className="select select--sm"
            aria-label={t('mods.tabela.ladoDe', { name: item.name })}
            value={item.side}
            disabled={sideBusy}
            onChange={(event) => {
              const side = SIDE_CHOICES.find((choice) => choice === event.target.value);
              if (side) onSide(item.path, side);
            }}
          >
            {item.side === 'unknown' ? (
              <option value={UNKNOWN_SIDE} disabled>
                {t('mods.lado.unknown')}
              </option>
            ) : null}
            {SIDE_CHOICES.map((side) => (
              <option key={side} value={side}>
                {t(`mods.lado.${side}`)}
              </option>
            ))}
          </select>
        ) : invalid ? null : (
          <span className="t-xs t-3">{t(`mods.lado.${item.side}`)}</span>
        )}
      </td>
    </tr>
  );
});

type EditorT = ReturnType<typeof useTranslation<'editor'>>['t'];

/** A segunda linha: o motivo do problema, ou a descrição do projeto, ou o arquivo. */
function rowDescription(item: InventoryItem, t: EditorT): string {
  if (item.state === 'invalid') return t('mods.marcas.invalidoTexto');
  if (item.state === 'outsideIndex') return t('mods.marcas.foraTexto');
  return item.summary ?? item.fileName ?? '';
}
