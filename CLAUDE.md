# Khipu

GUI de escritorio (Rust + egui) para espanso en Windows. Convive con `espansod.exe`.
Antes se llamaba *Espanso Manager*; desde el 2026-10-05 es **Khipu** en todo (decisión de Alex).
El repo es `github.com/alexpaltrc/khipu` (GitHub redirige la dirección vieja `espanso-manager`).

Este archivo es la memoria del proyecto: lo leen las sesiones en el equipo de Alex y las de
Claude Code en la nube. Sigue el estándar personal de Alex (skill `alex-vibecode`); la nube no ve
ese skill, así que lo esencial va aquí.

## Cómo trabajar con Alex

- Alex no programa: usa cada día la versión más reciente (Khipu Dev) para encontrar bugs e ideas.
- Informes **en español**, resumidos, para alguien no técnico, y honestos: lo que no se pudo
  medir se dice que no se pudo medir. Al cerrar un cambio, decir exactamente qué probar.
- Prioridades, en orden: ultraminimalista, ultra simple, moderno, nativo de Windows 11,
  rendimiento medido, elegancia. Ante la duda, la opción más simple.
- Código, comentarios y commits en inglés; documentación en español; la app en cuatro idiomas.

## Estado

- **Última versión publicada: 0.0.7** (etiqueta `v0.0.7` en `011d500`, GitHub Releases el
  2026-10-13; enviada a la Store el mismo día).
- **En curso: `0.0.8-dev`** (abierta en `b6c174e`). Sin publicar hasta que Alex lo pida. El
  `-dev` se quita en el commit que publica, y ese commit lleva la etiqueta de la versión.
- **Microsoft Store**: Store ID `9NXXV15RJ94T`, `https://apps.microsoft.com/detail/9NXXV15RJ94T`.
  Pública desde que se aprobó el envío 2 (0.0.6). Cada actualización es un envío nuevo con
  versión mayor (cuarto número en 0). Lo de Partner Center lo hace Alex (`docs/store/GUIA.md`).
- En uso diario por compañeros de trabajo de Alex.
- Tema oculto «I got chiiills», con dos L, desde 0.0.6 (↑ ↑ ↓ ↓ ← → ← → en Consejos; en disco
  va como `Light` + `khipu_theme`; tiñe la barra de título con `DWMWA_CAPTION_COLOR`). El README
  lo esconde tras dos spoilers `<details>`.

## Estable y Dev

Dos copias instaladas, ambas MSIX, con identidad y datos propios. Nunca se abre el exe suelto de
`target/`.

- **Estable: «Khipu», la de la Microsoft Store.** La que Alex usa a diario, como cualquier
  usuario. Solo cambia cuando se publica una versión y la Store la entrega. Mientras se trabaja
  no se cierra, no se abre ni se toca.
- **Dev: «Khipu Dev»** (`AlexPalacios.Khipu.Dev`, publicador `CN=Alex Palacios Dev`). Donde
  llega cada cambio y donde Alex lo prueba cuando quiere. Firmada con un certificado de
  desarrollo creado en el equipo de Alex (`CurrentUser\My`) y confiado solo ahí
  (`LocalMachine\TrustedPeople`, `packaging/trust-dev-cert.ps1`, una vez, con UAC). Sin modo de
  desarrollador. Sus datos: `%LOCALAPPDATA%\Packages\AlexPalacios.Khipu.Dev_6pha5dhx37rs8\LocalState`.
- **No corren las dos a la vez**: comparten el mutex de instancia única (dos espanso escribirían
  doble). Para probar Dev, Alex cierra la estable; Claude no la cierra por él.
- **Actualizar Dev**: `packaging/update-dev.ps1` (`git pull`, compila, firma, instala encima con
  el cuarto número +1). Al terminar cualquier cambio en el equipo de Alex, correrlo con
  `-NoPull` y decir en una línea que ya está actualizada. No la abre: la abre Alex cuando quiera
  probar. Si Claude la cerró y estaba abierta, la vuelve a abrir.

## Bajar y subir cambios

Hay una conversación en el equipo de Alex y otra en la nube (celular). Para que no choquen:

- **Al empezar** (en el equipo de Alex), antes de cualquier otra cosa: `git pull`, unir a `main`
  las ramas de la nube cuya CI pasó, actualizar Khipu Dev, y decir en una línea qué llegó.
- **Al terminar**, subir el trabajo: en el equipo de Alex, `git push` a `main`; en la nube, a su
  rama propia con PR, sin tocar `main`.

## Dónde corre la sesión

Khipu es **solo para Windows**: el crate no compila para Linux, y sus pruebas tampoco corren ahí.

**En el equipo de Alex (Windows):** todo. `cargo build --release`, `cargo test`, medir sobre el
binario, actualizar Khipu Dev, construir la MSIX de la Store.

**En la nube (Linux, `CLAUDE_CODE_REMOTE=true`):** el gancho de inicio instala el destino
`x86_64-pc-windows-msvc`, y con eso:

- `cargo wcheck` — compila (sin enlazar) todo el crate y sus pruebas para Windows. Es la
  verificación que sí se puede hacer en la nube: tipos, préstamos y warnings.
- No se puede: ejecutar `cargo test`, abrir la app, medir, ni tomar capturas. Decirlo así en
  el informe, nunca darlo por probado.
- **GitHub Actions** (`.github/workflows/ci.yml`) compila en Windows, corre `cargo test` y deja
  `Khipu.exe` como artefacto. Es la prueba real: leerla con `gh run list` / `gh run view` antes
  de decir que pasó. Un cambio no está terminado si la CI falla.
- Al terminar, el trabajo llega a Khipu Dev la próxima vez que Alex abra la conversación de su
  equipo. Decírselo en el informe.

## Organización

```
Vibecode\Khipu\          este repo (solo código y documentos)
├─ src/                  la app (mapa abajo)
├─ assets/khipu/         el paquete de identidad, tal cual se entregó
├─ vendor/               eframe y epaint con un cambio cada uno (vendor/README.md)
├─ packaging/            MSIX (Store y Dev), certificado Dev, actualizar Dev, avisos de terceros
├─ docs/                 capturas del README, roadmap y ficha/guía de la Store (docs/store/)
├─ web/                  el sitio de presentación, estático (web/README.md)
└─ .github/ .claude/ .cargo/   CI, arranque en la nube, alias `cargo wcheck`
```

- Fuera del repo, en `Documentos\Vibecode Archivos\Khipu Archivos\`: el material de marca
  (videos, campaña e imágenes de la Store, paleta). No es código; no traerlo de vuelta.
- Nada personal entra a git: las expansiones de Alex viven en el LocalState de Khipu Dev.
- Versiones publicadas: GitHub Releases, etiquetas `v0.0.1` a `v0.0.7`. Desde la 0.0.5 el ZIP se
  sube como `Khipu-Portable-<versión>.zip`, con guiones (GitHub cambia los espacios por puntos).
  Lleva una carpeta `Khipu <versión>/` con solo `Khipu.exe` y `Program Files/`.
- `PRIVACY.md` en `main` es la política de privacidad de la Store: no moverlo.
- Datos: `.khipu/` (era `.espanso-manager/` hasta 0.0.4; sin migración).
- **MSIX**: `packaging/build-msix.ps1` (`-Channel Store` por defecto, `-Channel Dev` para Khipu
  Dev). Sale en `target\msix\out\`. Toma espanso del ZIP portable de la última versión de GitHub
  Releases (se baja una vez a `target\msix\`), o de `-RuntimeZip`. La de la Store va sin firmar
  (la firma la Store) con `AlexPalacios.Khipu` / `CN=A3B4D4CD-AECB-4B6D-9268-AEAB19F8F17B`; no
  instalarla en un equipo que la vaya a recibir de la Store. Canales: Store, winget (solo fuente
  msstore) y el ZIP.
- **Icono**: el paquete de identidad (2026-10-05) vive tal cual en `assets/khipu/` (su
  `README.md` e `inventario.json` describen cada archivo; no reordenarlo). El exe toma
  `app/Khipu.ico`; la MSIX copia `msix/Assets/*.png`; bandeja, ventana y bienvenida los dibuja
  `src/icons.rs` desde el vector, y sus pruebas los comparan con los PNG del paquete.

## Mapa del código

Cada archivo de `src/` abre con una cabecera `//!` que dice qué le toca decidir, qué
invariante sostiene y qué **no** va ahí. Léela antes de editar el archivo; esta tabla es
solo el índice para saber cuál abrir.

**Arranque y estado**

| Archivo | Qué manda |
|---|---|
| `main.rs` | El orden del arranque: idioma → hook de pánico → mutex de instancia única → ventana |
| `install.rs` | Portable o instalada (MSIX): dónde están los recursos, dónde los datos, y la semilla del primer arranque |
| `app.rs` | Cuatro barrios: `AppState` (toda mutación y todo guardado), la paleta de Windows 11, la geometría de ventana y el `App` de eframe |
| `settings.rs` | `settings.json`. Las carpetas son asignaciones **por disparador**, no contenedores |
| `theme.rs` · `sysevents.rs` | Qué lleva puesto Windows, y enterarse de que cambió sin preguntárselo en bucle |
| `display.rs` | Que la ventana quepa en la pantalla en la que abre, no en la que la guardó |
| `pacing.rs` | Dónde espera cada fotograma: en el compositor (`DwmFlush`), no dentro del driver (vsync apagado) |
| `reload.rs` | Reiniciar espanso tras guardar en otro hilo: uno a la vez, y Pausa y Salir lo esperan |
| `fonts.rs` · `i18n.rs` | Las caras del sistema (Segoe UI Variable, Cascadia Mono, iconos); las cadenas en cuatro idiomas |
| `autostart.rs` · `hotkey.rs` | La casilla de inicio con Windows (clave Run en la portable, StartupTask en la instalada); el atajo global del launcher y su captura en Ajustes (encendido desde 0.0.4; su historia: `git show 58753f7:LAUNCHER-BACKUP.txt`) |

**Espanso**

| Archivo | Qué manda |
|---|---|
| `espanso_ctl.rs` | Todo lo que se le dice a `espansod`. Todo bloquea el hilo de la interfaz: nada sin plazo |
| `config_patch.rs` | Retoques quirúrgicos al `default.yml` de espanso, que es archivo ajeno |
| `tray.rs` | Icono, menú y los dos hilos que siguen vivos cuando no hay ventana |
| `icons.rs` | El pájaro de Khipu rasterizado desde el vector al tamaño de cada superficie, y sus tonos |
| `paste.rs` | Pegar un texto en la ventana de enfrente como Win+V: portapapeles, Ctrl+V y devolverlo como estaba |

**Los datos del usuario**

| Archivo | Qué manda |
|---|---|
| `yaml/model.rs` | La forma de `base.yml` y la promesa impresa arriba: lo que no se puede mostrar se conserva |
| `yaml/io.rs` | Cargar, validar, respaldar y guardar. **Nada aquí escribe a consola: no hay** |
| `yaml/presets.rs` · `datefmt.rs` | Los cuatro formatos de fecha, y la vista previa en vivo de cualquier formato |
| `yaml/form.rs` | Expansiones con campos por rellenar, y el ayudante que hace que espanso abra la ventana de Khipu |
| `folders.rs` | Carpetas y cambios de prefijo, derivados de los disparadores reales; los planes no tocan disco |
| `transfer.rs` | Exportar e importar entre máquinas |

**Pantallas** (`ui/`, ninguna toca el disco: todas llaman a `AppState`)

| Archivo | Qué manda |
|---|---|
| `list_view.rs` | La pantalla principal: virtualización, resaltado a un fotograma, arrastrar y soltar |
| `edit_form.rs` · `date_blocks.rs` | Crear y editar una expansión; armar un formato de fecha por bloques |
| `settings_view.rs` · `tips_view.rs` · `onboarding_view.rs` | Ajustes, consejos, primera vez |
| `folder_view.rs` | Las opciones de una carpeta: renombrar, borrar |
| `search_view.rs` · `controls.rs` | El launcher de expansiones; los controles compartidos |
| `form_view.rs` | La ventana que abre un formulario (`Khipu.exe --form`): sus campos y el texto resultante |
| `whats_new_view.rs` | «Novedades»: la tarjeta que sale una vez tras actualizar; no es el changelog |
| `studio.rs` · `glide.rs` | Piezas compartidas (iconos dibujados, filtro de carpetas); el resaltado que se desliza entre controles |
| `keys.rs` · `glyphs.rs` | Todos los atajos de teclado en una tabla; los iconos Segoe Fluent |

## Reglas permanentes

- Ninguna copia de Khipu vive ni se abre desde esta carpeta: la estable viene de la Store y Khipu
  Dev se instala con su MSIX.
- Limpio desde el principio: sin archivos sobrantes, carpetas vacías ni código muerto.
- **Si un cambio hace algo más feo, no lo hagas.** La interfaz está terminada y aprobada.
- El texto cortado en el borde de la ventana quita elegancia, prestigio y belleza.
- **Nada de subagentes ni workflows.** Son lo más caro del plan. Verifica tú mismo.
- Los hallazgos de la revisión se trabajan **uno por uno**, para llevar el control del
  uso del plan. No adelantarse al lote que el usuario pidió.
- Los informes al usuario van **en español**, y son honestos: lo que no se pudo medir
  se dice que no se pudo medir.
- Cada conversación termina subiendo su trabajo («Bajar y subir cambios»). Publicar una versión
  (etiqueta, GitHub Releases, Store) solo cuando Alex lo decide.

## Flujo por cada cambio

1. **Verificar personalmente** contra las fuentes reales de los crates (los hallazgos de
   revisores, humanos o no, casi siempre resultan "confirmado-*con corrección*").
2. Si viene de una revisión, decir en qué se equivocó y aplicar la corrección propia.
3. Local: `cargo build --release` y `cargo test`, sin warnings.
   Nube: `cargo wcheck` sin warnings, y después el CI de la rama en verde.
4. Local: **probarlo por medición sobre el binario real**, contra el build del commit anterior
   (o el ZIP de la última versión en GitHub Releases), mismo escenario antes y después. En la
   nube no se puede: dejarlo anotado como pendiente.
5. Local: actualizar Khipu Dev (`packaging/update-dev.ps1 -NoPull`).
6. Commit con la sección *Unreleased* de `CHANGELOG.md` al día.
7. Informar en español, incluyendo lo que quedó sin probar y por qué.

## Entorno local (Windows)

- Git en `C:\Program Files\Git\cmd`; GitHub CLI (`gh`) con la sesión de `alexpaltrc`.
- Rust estable con `rustup`; fuentes de los crates para verificar en
  `%USERPROFILE%\.cargo\registry\src\index.crates.io-*\`.
- Windows en español: Ctrl+A es *Abrir*, no *Seleccionar todo*.
- Las expansiones se prueban en **Brave**, nunca en el Bloc de notas, y con un disparador que
  Alex tenga de verdad (p. ej. `;fecha`).
- Al probar con gestos sintéticos, re-enfocar la ventana antes de cada gesto y de cada
  captura: `PrintWindow` devuelve un fotograma viejo si la ventana no está al frente.
