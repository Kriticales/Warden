/**
 * O mesmo arquivo mudou no pack e no jogo (SPEC T15 "Conflito"; CA-T15-05): as duas mudanças a
 * partir do início do teste, lado a lado, e a escolha: "Manter a do pack", "Usar a do jogo",
 * "Juntar as duas" (quando as chaves mudadas são diferentes) e "Editar e combinar".
 */
import { Suspense, lazy, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { DiffView } from '../../../components/common/DiffView';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import type { CaptureFileView, CaptureResolution, PackId } from '../../../lib/ipc/bindings';
import { Choice } from '../../settings/components/fields';
import { useCaptureTexts } from '../api';

// O editor (CodeMirror) só carrega quando a pessoa escolhe "Editar e combinar".
const MergeEditorDialog = lazy(() =>
  import('./MergeEditorDialog').then((module) => ({ default: module.MergeEditorDialog })),
);

export interface ConflictItemProps {
  packId: PackId;
  sessionId: string;
  view: CaptureFileView;
  choice: CaptureResolution;
  onChoose: (choice: CaptureResolution) => void;
}

export function ConflictItem({ packId, sessionId, view, choice, onChoose }: ConflictItemProps) {
  const { t } = useTranslation('captura');
  const path = view.file.path;
  const texts = useCaptureTexts(packId, sessionId, path);
  const [editing, setEditing] = useState(false);
  const mergeable = view.conflict?.mergeable ?? false;
  const name = `conflito-${path}`;
  const edited = choice.kind === 'edited' ? choice.text : null;

  return (
    <div className="change-item change-item--conflict">
      <div className="stack-2">
        <div className="t-sm t-strong path">{path}</div>
        {texts.isPending ? (
          <LoadingState inline label={t('conflito.carregando')} />
        ) : texts.isError ? (
          <ErrorPanel compact error={texts.error} />
        ) : (
          <div className="grid-2">
            <DiffView
              file={t('conflito.noPack')}
              before={texts.data.base ?? ''}
              after={texts.data.pack ?? ''}
            />
            <DiffView
              file={t('conflito.noJogo')}
              before={texts.data.base ?? ''}
              after={texts.data.instance ?? ''}
            />
          </div>
        )}
        <fieldset className="capture-choices">
          <legend className="sr-only">{t('conflito.legenda', { path })}</legend>
          <div className="choice-list choice-list--2">
            <Choice
              name={name}
              title={t('conflito.manter')}
              desc={t('conflito.manterDesc')}
              checked={choice.kind === 'keepPack'}
              onSelect={() => {
                onChoose({ kind: 'keepPack' });
              }}
            />
            <Choice
              name={name}
              title={t('conflito.usar')}
              desc={t('conflito.usarDesc')}
              checked={choice.kind === 'take'}
              onSelect={() => {
                onChoose({ kind: 'take' });
              }}
            />
            {mergeable ? (
              <Choice
                name={name}
                title={t('conflito.juntar')}
                desc={t('conflito.juntarDesc')}
                checked={choice.kind === 'merge'}
                onSelect={() => {
                  onChoose({ kind: 'merge' });
                }}
              />
            ) : null}
            <Choice
              name={name}
              title={t('conflito.editar')}
              desc={t('conflito.editarDesc')}
              checked={choice.kind === 'edited'}
              disabled={!texts.isSuccess}
              onSelect={() => {
                setEditing(true);
              }}
            />
          </div>
        </fieldset>
        {edited !== null ? (
          <p className="t-sm t-2 row row--wrap">
            <span>{t('conflito.combinado')}</span>
            <Button
              variant="link"
              onClick={() => {
                setEditing(true);
              }}
            >
              {t('conflito.reabrir')}
            </Button>
          </p>
        ) : null}
      </div>
      {editing && texts.isSuccess ? (
        <Suspense fallback={<LoadingState inline label={t('conflito.carregando')} />}>
          <MergeEditorDialog
            path={path}
            texts={texts.data}
            initial={edited}
            onUse={(text) => {
              setEditing(false);
              onChoose({ kind: 'edited', text });
            }}
            onCancel={() => {
              setEditing(false);
            }}
          />
        </Suspense>
      ) : null}
    </div>
  );
}
