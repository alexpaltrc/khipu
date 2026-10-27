#!/usr/bin/env bash
# Runs when a Claude Code session opens. Only a cloud session (Linux) needs anything: the Windows
# target, so that `cargo wcheck` can type-check Khipu there. On Alex's PC it does nothing.
[ "$CLAUDE_CODE_REMOTE" = "true" ] || exit 0

if command -v rustup >/dev/null 2>&1; then
  rustup target add x86_64-pc-windows-msvc >/dev/null 2>&1 \
    && echo "Destino x86_64-pc-windows-msvc listo: usa 'cargo wcheck'. Las pruebas corren en el CI de Windows."
else
  echo "rustup no está en esta máquina: no se puede compilar aquí. Empuja la rama y lee el CI."
fi
exit 0
