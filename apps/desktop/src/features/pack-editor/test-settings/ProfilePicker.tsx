/**
 * O seletor "Perfil do teste" no topo dos Ajustes do teste (SPEC T11; ROADMAP L-08): qual
 * perfil está sendo editado, com Novo perfil, Renomear e Apagar. Novo perfil e Renomear pedem
 * o nome numa linha do próprio diálogo (sem abrir outro diálogo por cima); Apagar pergunta
 * antes. O Padrão não pode ser renomeado nem apagado.
 */
import { Pencil, Plus, Trash2 } from 'lucide-react';
import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { PackId, ProfileFields, TestProfile } from '../../../lib/ipc/bindings';
import { useCreateProfile, useDeleteProfile, useRenameProfile } from '../../test/profiles/api';
import { DEFAULT_PROFILE, MAX_PROFILE_NAME, profileName } from '../../test/profiles/text';

export interface ProfilePickerProps {
  packId: PackId;
  /** Os perfis, com o rascunho do diálogo (nomes como gravados). */
  profiles: TestProfile[];
  /** O perfil em uso no Testar. */
  active: string;
  /** O perfil sendo editado. */
  editing: string;
  onEdit: (id: string) => void;
  /** Os ajustes na tela, para o perfil novo começar com eles. */
  draft: ProfileFields;
  /** Um perfil foi criado (o diálogo passa a editá-lo). */
  onCreated: (profile: TestProfile) => void;
  /** Um perfil foi apagado. */
  onDeleted: (id: string) => void;
}

type Naming = { mode: 'new' | 'rename'; value: string } | null;

export function ProfilePicker({
  packId,
  profiles,
  active,
  editing,
  onEdit,
  draft,
  onCreated,
  onDeleted,
}: ProfilePickerProps) {
  const { t } = useTranslation('perfis');
  const selectId = useId();
  const nameId = useId();
  const [naming, setNaming] = useState<Naming>(null);
  const selectRef = useRef<HTMLSelectElement>(null);
  const wasNaming = useRef(false);
  // A linha do nome fechou (criou, renomeou ou desistiu): o foco volta para o seletor.
  useEffect(() => {
    if (wasNaming.current && naming === null) selectRef.current?.focus();
    wasNaming.current = naming !== null;
  }, [naming]);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const create = useCreateProfile(packId);
  const rename = useRenameProfile(packId);
  const remove = useDeleteProfile(packId);
  const current = profiles.find((profile) => profile.id === editing) ?? profiles[0];
  const isDefault = current?.id === DEFAULT_PROFILE;

  const nameError = (() => {
    if (!naming) return null;
    const name = naming.value.trim();
    if (name === '') return t('nomeVazio');
    // Em caracteres, como o backend (`chars().count()`), não em unidades UTF-16.
    if (Array.from(name).length > MAX_PROFILE_NAME)
      return t('nomeLongo', { max: MAX_PROFILE_NAME });
    const lower = name.toLowerCase();
    const taken =
      lower === t('padrao').toLowerCase() ||
      lower === 'padrao' ||
      profiles.some(
        (profile) =>
          profile.id !== DEFAULT_PROFILE &&
          !(naming.mode === 'rename' && profile.id === editing) &&
          profile.name.toLowerCase() === lower,
      );
    return taken ? t('nomeRepetido') : null;
  })();
  const [touched, setTouched] = useState(false);
  const mutation = naming?.mode === 'rename' ? rename : create;

  const submitName = () => {
    setTouched(true);
    if (!naming || nameError) return;
    const name = naming.value.trim();
    if (naming.mode === 'new') {
      create.mutate(
        { name, fields: draft },
        {
          onSuccess: (profile) => {
            setNaming(null);
            onCreated(profile);
            showToast({ kind: 'ok', title: t('criado', { nome: profile.name }) });
          },
        },
      );
    } else {
      rename.mutate(
        { id: editing, name },
        {
          onSuccess: () => {
            setNaming(null);
            showToast({ kind: 'ok', title: t('renomeado') });
          },
        },
      );
    }
  };

  const startNaming = (mode: 'new' | 'rename') => {
    create.reset();
    rename.reset();
    setTouched(false);
    setNaming({ mode, value: mode === 'rename' && current ? current.name : '' });
  };

  return (
    <div className="field">
      <label className="field__label" htmlFor={selectId}>
        {t('perfil')}
      </label>
      <div className="row row--wrap">
        <select
          ref={selectRef}
          id={selectId}
          className="select"
          value={editing}
          aria-describedby={`${selectId}-hint`}
          disabled={naming !== null}
          onChange={(event) => {
            onEdit(event.target.value);
          }}
        >
          {profiles.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {profile.id === active
                ? t('emUso', { nome: profileName(profile) })
                : profileName(profile)}
            </option>
          ))}
        </select>
        <Button
          size="sm"
          icon={Plus}
          disabled={naming !== null}
          onClick={() => {
            startNaming('new');
          }}
        >
          {t('novo')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          icon={Pencil}
          disabled={isDefault || naming !== null}
          onClick={() => {
            startNaming('rename');
          }}
        >
          {t('renomear')}
        </Button>
        <Button
          size="sm"
          variant="danger-ghost"
          icon={Trash2}
          disabled={isDefault || naming !== null}
          onClick={() => {
            setConfirmDelete(true);
          }}
        >
          {t('apagar')}
        </Button>
      </div>
      <div className="field__hint" id={`${selectId}-hint`}>
        {isDefault ? `${t('perfilDica')} ${t('padraoFixo')}` : t('perfilDica')}
      </div>

      {naming ? (
        <form
          className="stack-2 profile-name"
          onSubmit={(event) => {
            event.preventDefault();
            submitName();
          }}
        >
          <label className="field__label" htmlFor={nameId}>
            {t('nome')}
          </label>
          <input
            id={nameId}
            className="input"
            // O campo aparece por um clique do usuário: o foco vai para ele.
            // eslint-disable-next-line jsx-a11y/no-autofocus
            autoFocus
            maxLength={MAX_PROFILE_NAME * 2}
            placeholder={t('nomeExemplo')}
            value={naming.value}
            aria-invalid={touched && nameError !== null}
            aria-describedby={`${nameId}-hint${touched && nameError ? ` ${nameId}-err` : ''}`}
            onChange={(event) => {
              setNaming({ ...naming, value: event.target.value });
            }}
          />
          {touched && nameError ? (
            <div className="field__error" id={`${nameId}-err`} role="alert">
              <span>{nameError}</span>
            </div>
          ) : null}
          <div className="field__hint" id={`${nameId}-hint`}>
            {naming.mode === 'new' ? t('novoDica') : null}
          </div>
          {mutation.isError ? <ErrorPanel compact error={mutation.error} /> : null}
          <div className="btn-row">
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setNaming(null);
              }}
            >
              {t('cancelar')}
            </Button>
            <Button size="sm" variant="primary" type="submit" loading={mutation.isPending}>
              {naming.mode === 'new'
                ? create.isPending
                  ? t('criando')
                  : t('criar')
                : rename.isPending
                  ? t('renomeando')
                  : t('renomearConfirmar')}
            </Button>
          </div>
        </form>
      ) : null}

      {current && !isDefault ? (
        <ConfirmDialog
          open={confirmDelete}
          onOpenChange={setConfirmDelete}
          title={t('apagarTitulo', { nome: current.name })}
          description={t('apagarTexto')}
          confirmLabel={t('apagarConfirmar')}
          confirmingLabel={t('apagando')}
          cancelLabel={t('cancelar')}
          onConfirm={async () => {
            await remove.mutateAsync(current.id);
            onDeleted(current.id);
            showToast({ kind: 'ok', title: t('apagado') });
          }}
        />
      ) : null}
    </div>
  );
}
