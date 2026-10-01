#!/usr/bin/env bash
# Sobe um Xvfb sem root (WSL2 sem DISPLAY). Baixa os .deb com apt-get download e
# extrai com dpkg -x numa pasta de dados fora do repositório. O socket Unix do WSLg
# (/tmp/.X11-unix) é somente leitura, então o servidor escuta só via TCP (porta 6000+N).
# Uso: scripts/xvfb.sh [display]   ->  depois: export DISPLAY=127.0.0.1:<display>
set -euo pipefail
DISP="${1:-77}"
BASE="${WARDEN_SPIKE_DATA_ROOT:-$HOME/.local/share/warden-spike}/xvfb"
ROOT="$BASE/root"
mkdir -p "$BASE"
if [ ! -x "$ROOT/usr/bin/Xvfb" ]; then
  (cd "$BASE" && apt-get download xvfb libunwind8 libxfont2 xserver-common x11-xkb-utils xkb-data libfontenc1)
  for f in "$BASE"/*.deb; do dpkg -x "$f" "$ROOT"; done
fi
export LD_LIBRARY_PATH="$ROOT/usr/lib/x86_64-linux-gnu"
exec "$ROOT/usr/bin/Xvfb" ":$DISP" -screen 0 1280x720x24 -xkbdir "$ROOT/usr/share/X11/xkb" \
  -nolisten unix -listen tcp
