/**
 * Desenho do pack inteiro em canvas (Cytoscape.js + `cytoscape-dagre`; ARCHITECTURE §18 e
 * §19.2). Carregado só quando "Ver o pack inteiro" desenha (`lazy` no diálogo), para o resto
 * do app não pagar o peso da biblioteca. O canvas não é acessível por si: a mesma informação
 * está na lista em texto ao lado, e o desenho se apresenta como imagem com rótulo.
 *
 * As cores vêm dos tokens do Deep Dark lidos do CSS (`--color-*`); os rótulos usam
 * `--color-text` sobre `--color-surface-2` (contraste medido no teste do modelo, ≥ 4,5:1).
 * O tempo de desenho (criação + layout) fica em `data-draw-ms`, para medir num pack grande.
 */
import cytoscape, { type Core, type ElementDefinition } from 'cytoscape';
import dagre from 'cytoscape-dagre';
import { useEffect, useRef, useState } from 'react';

import { LoadingState } from '../../../components/common/LoadingState';

import type { FocusReport } from '../../../lib/ipc/bindings';
import { LAYOUT, wholeElements, wholeStyle, type Palette } from './whole';

cytoscape.use(dagre);

/** Lê os tokens de cor do tema (o canvas não entende `var(--…)`). */
function palette(element: Element): Palette {
  const css = getComputedStyle(element);
  const token = (name: string) => css.getPropertyValue(name).trim();
  return {
    text: token('--color-text'),
    text2: token('--color-text-2'),
    node: token('--color-surface-2'),
    border: token('--color-border-strong'),
    group: token('--color-bg-sunken'),
    edge: token('--color-text-3'),
    danger: token('--color-danger'),
    info: token('--color-info'),
    primary: token('--color-primary'),
    font: token('--font-ui'),
  };
}

export interface WholeGraphCanvasProps {
  report: FocusReport;
  /** Rótulo do desenho para o leitor de tela. */
  label: string;
  /** Nome do grupo das bibliotecas. */
  libraries: string;
  /** Aviso enquanto o layout roda. */
  drawing: string;
  onOpen: (path: string) => void;
  onFail: () => void;
}

export default function WholeGraphCanvas({
  report,
  label,
  libraries,
  drawing,
  onOpen,
  onFail,
}: WholeGraphCanvasProps) {
  const host = useRef<HTMLDivElement>(null);
  const [busy, setBusy] = useState(true);
  const handlers = useRef({ onOpen, onFail });
  useEffect(() => {
    handlers.current = { onOpen, onFail };
  });

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    let cy: Core | null = null;
    setBusy(true);
    // O layout roda depois de o aviso "Desenhando…" aparecer (ele ocupa a thread da tela).
    const timer = window.setTimeout(() => {
      try {
        const started = performance.now();
        const elements: ElementDefinition[] = wholeElements(report, libraries);
        cy = cytoscape({
          container: element,
          elements,
          style: wholeStyle(palette(element)),
          layout: LAYOUT,
          wheelSensitivity: 0.3,
          minZoom: 0.05,
          maxZoom: 3,
        });
        element.dataset.drawMs = String(Math.round(performance.now() - started));
        cy.on('tap', 'node', (event) => {
          const target = event.target as cytoscape.NodeSingular;
          const path: unknown = target.data('path');
          if (typeof path === 'string') handlers.current.onOpen(path);
        });
      } catch {
        handlers.current.onFail();
      }
      setBusy(false);
    }, 0);
    return () => {
      window.clearTimeout(timer);
      cy?.destroy();
    };
  }, [report, libraries]);

  return (
    <>
      {busy ? <LoadingState inline label={drawing} /> : null}
      <div ref={host} className="grafo-inteiro__canvas" role="img" aria-label={label} />
    </>
  );
}
