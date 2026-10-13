/**
 * "Editar e combinar" (SPEC T15 "Conflito"): o editor de texto começa com a versão do pack e,
 * abaixo, o que mudou no jogo (início do teste → jogo agora). "Usar este texto" só guarda a
 * escolha; nada vai para o pack até "Trazer selecionados para o pack".
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { DiffView } from '../../../components/common/DiffView';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import type { CaptureTexts } from '../../../lib/ipc/bindings';
import { CodeEditor } from '../../configs/components/CodeEditor';
import { languageOf } from '../../configs/model';

export interface MergeEditorDialogProps {
  path: string;
  texts: CaptureTexts;
  /** Texto já combinado antes (reabrir o editor). */
  initial: string | null;
  onUse: (text: string) => void;
  onCancel: () => void;
}

export function MergeEditorDialog({
  path,
  texts,
  initial,
  onUse,
  onCancel,
}: MergeEditorDialogProps) {
  const { t } = useTranslation('captura');
  const start = initial ?? texts.pack ?? texts.instance ?? '';
  const [text, setText] = useState(start);
  const separator = texts.lineEnding === '\r\n' ? '\r\n' : '\n';
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <DialogContent
        size="lg"
        title={t('editor.titulo', { path })}
        description={t('editor.descricao')}
        footer={
          <>
            <Button variant="ghost" onClick={onCancel}>
              {t('editor.cancelar')}
            </Button>
            <Button
              variant="primary"
              onClick={() => {
                onUse(text);
              }}
            >
              {t('editor.usar')}
            </Button>
          </>
        }
      >
        <div className="stack-3">
          <div className="capture-editor">
            <CodeEditor
              resetKey={path}
              initial={start}
              saved={texts.pack ?? ''}
              language={languageOf(path)}
              readOnly={false}
              lineSeparator={separator}
              ariaLabel={t('editor.rotulo', { path })}
              onChange={setText}
              onCursor={() => undefined}
            />
          </div>
          <DiffView
            file={t('conflito.noJogo')}
            before={texts.base ?? ''}
            after={texts.instance ?? ''}
          />
        </div>
      </DialogContent>
    </Dialog>
  );
}
