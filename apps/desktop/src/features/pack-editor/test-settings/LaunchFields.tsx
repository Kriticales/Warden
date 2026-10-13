/**
 * "Ao abrir o jogo" e "Janela do jogo" dos Ajustes do teste (SPEC T11 "Perfis do teste";
 * ROADMAP L-08).
 *
 * "Entrar direto no mundo de teste" usa o Quick Play do próprio jogo, que só existe a partir do
 * Minecraft 1.20: nas versões anteriores a opção aparece desabilitada com o motivo (CA-T11-05).
 * O mundo é um dos mundos de teste da instância (o último jogado, se a pessoa não escolher);
 * sem mundo ainda, o texto diz que o jogo abre no menu até existir um.
 */
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import type { OnLaunch, PackId, QuickPlaySupport, TestWindow } from '../../../lib/ipc/bindings';
import { useInstanceWorlds } from '../../test/api';

/** Tamanhos de janela oferecidos (16:9, do HD ao Full HD). */
const WINDOW_SIZES: readonly [number, number][] = [
  [1280, 720],
  [1600, 900],
  [1920, 1080],
];

/** Valor do `<select>` para "o último que você jogou". */
const LAST_WORLD = '';

function windowValue(window: TestWindow): string {
  switch (window.mode) {
    case 'default':
      return 'default';
    case 'fullscreen':
      return 'fullscreen';
    case 'size':
      return `${String(window.width)}x${String(window.height)}`;
  }
}

function parseWindow(value: string): TestWindow {
  if (value === 'default' || value === 'fullscreen') return { mode: value };
  const [width, height] = value.split('x').map(Number);
  return { mode: 'size', width: width ?? 0, height: height ?? 0 };
}

export interface OnLaunchFieldProps {
  packId: PackId;
  value: OnLaunch;
  onChange: (value: OnLaunch) => void;
  quickPlay: QuickPlaySupport;
  minecraft: string | null;
}

export function OnLaunchField({
  packId,
  value,
  onChange,
  quickPlay,
  minecraft,
}: OnLaunchFieldProps) {
  const { t } = useTranslation('perfis');
  const name = useId();
  const worldId = useId();
  const supported = quickPlay === 'supported';
  const world = value.mode === 'world';
  const worlds = useInstanceWorlds(packId, supported && world);
  const reason = supported
    ? t('abrir.mundoDica')
    : quickPlay === 'tooOld' && minecraft
      ? t('abrir.antigo', { minecraft })
      : t('abrir.desconhecido');
  const names = worlds.data ?? [];
  const chosen = value.mode === 'world' ? (value.world ?? LAST_WORLD) : LAST_WORLD;

  return (
    <fieldset className="field">
      <legend className="field__label">{t('abrir.titulo')}</legend>
      <div className="stack-2">
        {/* O texto do rótulo está no <span> dentro dele (padrão `.check` do design system). */}
        {/* eslint-disable-next-line jsx-a11y/label-has-associated-control */}
        <label className="check">
          <input
            type="radio"
            name={name}
            checked={!world}
            onChange={() => {
              onChange({ mode: 'menu' });
            }}
          />
          <span className="check__text">
            <span>{t('abrir.menu')}</span>
          </span>
        </label>
        {/* eslint-disable-next-line jsx-a11y/label-has-associated-control */}
        <label className="check">
          <input
            type="radio"
            name={name}
            checked={world}
            disabled={!supported}
            aria-describedby={`${name}-motivo`}
            onChange={() => {
              onChange({ mode: 'world', world: null });
            }}
          />
          <span className="check__text">
            <span>{t('abrir.mundo')}</span>
            <span className="check__desc" id={`${name}-motivo`}>
              {reason}
            </span>
          </span>
        </label>
      </div>
      {supported && world ? (
        <div className="field profile-world">
          <label className="field__label" htmlFor={worldId}>
            {t('abrir.qualMundo')}
          </label>
          <select
            id={worldId}
            className="select"
            value={chosen}
            aria-describedby={
              worlds.isSuccess && names.length === 0 ? `${worldId}-hint` : undefined
            }
            onChange={(event) => {
              const picked = event.target.value;
              onChange({ mode: 'world', world: picked === LAST_WORLD ? null : picked });
            }}
          >
            <option value={LAST_WORLD}>{t('abrir.ultimo')}</option>
            {names.map((world) => (
              <option key={world} value={world}>
                {world}
              </option>
            ))}
            {chosen !== LAST_WORLD && !names.includes(chosen) ? (
              <option value={chosen}>{chosen}</option>
            ) : null}
          </select>
          {worlds.isSuccess && names.length === 0 ? (
            <div className="field__hint" id={`${worldId}-hint`}>
              {t('abrir.semMundo')}
            </div>
          ) : null}
        </div>
      ) : null}
    </fieldset>
  );
}

export function WindowField({
  value,
  onChange,
}: {
  value: TestWindow;
  onChange: (value: TestWindow) => void;
}) {
  const { t } = useTranslation('perfis');
  const id = useId();
  const options: { value: string; label: string }[] = [
    { value: 'default', label: t('janela.padrao') },
    ...WINDOW_SIZES.map(([largura, altura]) => ({
      value: `${String(largura)}x${String(altura)}`,
      label: t('janela.tamanho', { largura, altura }),
    })),
    { value: 'fullscreen', label: t('janela.telaCheia') },
  ];
  const current = windowValue(value);
  if (value.mode === 'size' && !options.some((option) => option.value === current)) {
    options.splice(options.length - 1, 0, {
      value: current,
      label: t('janela.tamanho', { largura: value.width, altura: value.height }),
    });
  }
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        {t('janela.titulo')}
      </label>
      <select
        id={id}
        className="select"
        value={current}
        onChange={(event) => {
          onChange(parseWindow(event.target.value));
        }}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}
