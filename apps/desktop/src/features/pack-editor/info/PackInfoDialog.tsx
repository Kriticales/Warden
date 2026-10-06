/**
 * Informações do pack (SPEC T11; protótipo `info`): nome, autor e descrição, gravados no
 * `pack.toml` mudando só as linhas alteradas (CA-T11-01). Mostra as versões (trocar a versão do
 * loader é da P1-15) e a pasta, com "Mostrar pasta do pack". Apagar pack fica em Meus packs.
 */
import { FolderOpen } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import { showToast } from '../../../components/ui/toast';
import type { PackMeta, PackRow } from '../../../lib/ipc/bindings';
import { TextField } from '../../settings/components/fields';
import { useRevealPack } from '../../packs/api';
import { loaderName } from '../../packs/lib/pack-list';
import { usePackMeta, useUpdateMeta } from '../api';

/** Limites do `pack.toml` (os mesmos do Rust, `warden_project::meta`). */
export const MAX_NAME = 100;
export const MAX_AUTHOR = 100;
export const MAX_DESCRIPTION = 2000;

export interface PackInfoDialogProps {
  pack: PackRow;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function PackInfoDialog({ pack, open, onOpenChange }: PackInfoDialogProps) {
  const { t } = useTranslation('editor');
  const meta = usePackMeta(pack.id, open);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open ? (
        meta.isSuccess ? (
          // A chave refaz o formulário quando o `pack.toml` mudar por fora.
          <InfoForm
            key={`${meta.data.name}|${meta.data.author}|${meta.data.description}`}
            pack={pack}
            meta={meta.data}
            onDone={() => {
              onOpenChange(false);
            }}
          />
        ) : (
          <DialogContent title={t('informacoes.titulo')}>
            {meta.isError ? (
              <ErrorPanel
                error={meta.error}
                onRetry={() => {
                  void meta.refetch();
                }}
              />
            ) : (
              <LoadingState inline label={t('cabecalho.carregando')} />
            )}
          </DialogContent>
        )
      ) : null}
    </Dialog>
  );
}

function InfoForm({ pack, meta, onDone }: { pack: PackRow; meta: PackMeta; onDone: () => void }) {
  const { t } = useTranslation('editor');
  const descriptionId = useId();
  const [name, setName] = useState(meta.name);
  const [author, setAuthor] = useState(meta.author);
  const [description, setDescription] = useState(meta.description);
  const [tried, setTried] = useState(false);
  const update = useUpdateMeta(pack.id);
  const reveal = useRevealPack();

  const nameError =
    name.trim() === ''
      ? t('informacoes.nomeObrigatorio')
      : Array.from(name.trim()).length > MAX_NAME
        ? t('informacoes.muitoLongo', { max: MAX_NAME })
        : null;
  const authorError =
    Array.from(author.trim()).length > MAX_AUTHOR
      ? t('informacoes.muitoLongo', { max: MAX_AUTHOR })
      : null;
  const descriptionError =
    Array.from(description.trim()).length > MAX_DESCRIPTION
      ? t('informacoes.muitoLongo', { max: MAX_DESCRIPTION })
      : null;
  const changed =
    name.trim() !== meta.name ||
    author.trim() !== meta.author ||
    description.trim() !== meta.description;

  const loader = meta.loader
    ? `${loaderName(meta.loader)} ${meta.loaderVersion ?? ''}`.trim()
    : t('cabecalho.vanilla');

  const save = () => {
    setTried(true);
    if (nameError || authorError || descriptionError) return;
    if (!changed) {
      showToast({ kind: 'info', title: t('informacoes.semMudancas') });
      onDone();
      return;
    }
    update.mutate(
      { name: name.trim(), author: author.trim(), description: description.trim() },
      {
        onSuccess: () => {
          showToast({ kind: 'ok', title: t('informacoes.salvo') });
          onDone();
        },
      },
    );
  };

  return (
    <DialogContent
      title={t('informacoes.titulo')}
      description={t('informacoes.descricao')}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('informacoes.cancelar')}</Button>
          </DialogClose>
          <Button variant="primary" loading={update.isPending} onClick={save}>
            {update.isPending ? t('informacoes.salvando') : t('informacoes.salvar')}
          </Button>
        </>
      }
    >
      <form
        className="stack-3"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          save();
        }}
      >
        <TextField
          label={t('informacoes.nome')}
          value={name}
          error={tried ? nameError : null}
          maxLength={MAX_NAME + 20}
          onChange={(event) => {
            setName(event.target.value);
          }}
        />
        <TextField
          label={t('informacoes.autor')}
          value={author}
          error={tried ? authorError : null}
          onChange={(event) => {
            setAuthor(event.target.value);
          }}
        />
        <div className="field">
          <label className="field__label" htmlFor={descriptionId}>
            {t('informacoes.descricaoCampo')}{' '}
            <span className="opt">{t('informacoes.opcional')}</span>
          </label>
          <textarea
            id={descriptionId}
            className="textarea"
            rows={3}
            value={description}
            placeholder={t('informacoes.descricaoPlaceholder')}
            aria-invalid={tried && descriptionError ? true : undefined}
            onChange={(event) => {
              setDescription(event.target.value);
            }}
          />
          {tried && descriptionError ? (
            <div className="field__error" role="alert">
              <span>{descriptionError}</span>
            </div>
          ) : null}
        </div>
        {/* Enter no formulário salva. */}
        <button type="submit" hidden />
      </form>
      <dl className="kv">
        <dt>{t('informacoes.versoes')}</dt>
        <dd>
          {t('informacoes.versoesTexto', { minecraft: meta.minecraft ?? '?', loader })}
          <div className="field__hint">{t('informacoes.versoesDica')}</div>
        </dd>
        <dt>{t('informacoes.pasta')}</dt>
        <dd className="path">{pack.path}</dd>
      </dl>
      {update.isError ? <ErrorPanel compact error={update.error} /> : null}
      <hr className="sep" />
      <div className="btn-row">
        <Button
          size="sm"
          icon={FolderOpen}
          onClick={() => {
            reveal.mutate(pack.id, {
              onError: () => {
                showToast({ kind: 'danger', title: t('informacoes.mostrarFalhou') });
              },
            });
          }}
        >
          {t('informacoes.mostrarPasta')}
        </Button>
      </div>
    </DialogContent>
  );
}
