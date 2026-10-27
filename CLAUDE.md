# Khipu

GUI de escritorio (Rust + egui) para espanso en Windows. Convive con `espansod.exe`.
Antes se llamaba *Espanso Manager*; desde el 2026-10-05 es **Khipu** en todo (decisión de Alex).
Se conservan a propósito con el nombre viejo: la identidad MSIX de pruebas `EspansoManager.Dev`.
El repo es `github.com/alexpaltrc/khipu` (GitHub redirige la dirección vieja `espanso-manager`).

Este archivo es la memoria del proyecto y viaja con el repo: lo leen las sesiones en el equipo
de Alex y las de Claude Code en la nube. Lo que solo vale en el equipo local va en el
`CLAUDE.md` de la carpeta de arriba, que no está en git.

## Estado

- **Última versión publicada: 0.0.7** (etiqueta `v0.0.7` en `011d500`, GitHub Releases el
  2026-10-13; enviada a la Store el mismo día).
- **En curso: `0.0.8-dev`** (abierta en `b6c174e`). Sin publicar hasta que Alex lo pida. El
  `-dev` se quita en el commit que publica, y ese commit lleva la etiqueta de la versión.
- **Microsoft Store**: Store ID `9NXXV15RJ94T`, `https://apps.microsoft.com/detail/9NXXV15RJ94T`.
  Pública desde que se aprobó el envío 2 (0.0.6). Cada actualización es un envío nuevo con
  versión mayor (cuarto número en 0). Lo de Partner Center lo hace Alex.
- En uso diario por compañeros de trabajo de Alex.
- Tema oculto «I got chiiills», con dos L, desde 0.0.6 (↑ ↑ ↓ ↓ ← → ← → en Consejos; en disco
  va como `Light` + `khipu_theme`; tiñe la barra de título con `DWMWA_CAPTION_COLOR`). El README
  lo esconde tras dos spoilers `<details>`.

## Dónde corre la sesión

Khipu es **solo para Windows**: el crate no compila para Linux, y sus pruebas tampoco corren ahí.

**En el equipo de Alex (Windows):** todo. `cargo build --release`, `cargo test`, abrir la app,
medir sobre el binario, desplegar el exe, construir la MSIX.

**En la nube (Linux, `CLAUDE_CODE_REMOTE=true`):** el gancho de inicio instala el destino
`x86_64-pc-windows-msvc`, y con eso:

- `cargo wcheck` — compila (sin enlazar) todo el crate y sus pruebas para Windows. Es la
  verificación que sí se puede hacer en la nube: tipos, préstamos y warnings.
- No se puede: ejecutar `cargo test`, abrir la app, medir, ni tomar capturas. Decirlo así en
  el informe, nunca darlo por probado.
- Trabajar en una rama y empujarla: **GitHub Actions** (`.github/workflows/ci.yml`) compila en
  Windows, corre `cargo test` y deja `Khipu.exe` como artefacto descargable del run. Esa es la
  prueba real; leer su resultado con `gh run list` / `gh run view` antes de decir que pasó.
- Lo que no se puede en la nube (desplegar el exe, el ZIP portable, la MSIX, medir la
  interfaz) queda pendiente para cuando Alex esté en su equipo: anotarlo en el informe.

## Organización

```
Khipu/                  (carpeta local de Alex; solo khipu/ está en git)
├─ khipu/               este repo
│  ├─ src/ assets/ vendor/ packaging/ docs/
│  └─ web/              el sitio de presentación (estático; ver web/README.md)
├─ portable/            la app de uso diario de Alex: Khipu.exe, espansod.exe y SUS datos
├─ paquetes/            ZIP portable y MSIX de cada versión
├─ marca/               identidad, paleta, video, campaña y ficha de la Store, roadmap
└─ archivo/             lo superado: versiones viejas de entregables, ZIPs, respaldos
```

- `portable/` está fuera del repo a propósito: ahí viven `.espanso/match/base.yml` y
  `.khipu/settings.json` con las expansiones reales de Alex. Nada personal entra a git.
- Binario desplegado: `portable/Khipu.exe`, copia directa de `target/release/Khipu.exe`.
- Paquete de prueba de la versión en curso: `paquetes/Khipu Portable <versión>.zip`, con una
  carpeta `Khipu <versión>/` que lleva solo `Khipu.exe` y `Program Files/`. `portable/` es plana:
  no copiarla tal cual.
- Versiones publicadas: GitHub Releases, etiquetas `v0.0.1` a `v0.0.7`. Desde la 0.0.5 el ZIP se
  sube como `Khipu-Portable-<versión>.zip`, con guiones (GitHub cambia los espacios por puntos).
- `PRIVACY.md` en `main` es la política de privacidad de la Store: no moverlo.
- Datos: `.khipu/` (era `.espanso-manager/` hasta 0.0.4; sin migración).
- **Edición instalada (MSIX)**: `packaging/build-msix.ps1` (`-Register` la instala en el equipo,
  con modo desarrollador); sale en `paquetes/Khipu <versión>.msix` y toma espanso del ZIP portable
  más nuevo de `paquetes/`. `-Revision N` sube el cuarto número para actualizar sin borrar
  LocalState. La registrada corre desde `target\msix\layout`. Identidad por defecto: la de la
  Store (`AlexPalacios.Khipu` / `CN=A3B4D4CD-AECB-4B6D-9268-AEAB19F8F17B`). **No registrar la
  identidad de la Store en un equipo que vaya a instalarla desde la Store.** Canales: Store,
  winget (solo fuente msstore) y el ZIP.
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

- `portable/` contiene **únicamente la aplicación portable, no el instalador**.
- **Si un cambio hace algo más feo, no lo hagas.** La interfaz está terminada y aprobada.
- El texto cortado en el borde de la ventana quita elegancia, prestigio y belleza.
- **Nada de subagentes ni workflows.** Son lo más caro del plan. Verifica tú mismo.
- Los hallazgos de la revisión se trabajan **uno por uno**, para llevar el control del
  uso del plan. No adelantarse al lote que el usuario pidió.
- Los informes al usuario van **en español**, y son honestos: lo que no se pudo medir
  se dice que no se pudo medir.
- `git push` solo cuando Alex lo pida. En la nube, trabajar en una rama propia y abrir PR; no
  empujar a `main` ni publicar versiones sin que Alex lo pida.

## Flujo por cada cambio

1. **Verificar personalmente** contra las fuentes reales de los crates (los hallazgos de
   revisores, humanos o no, casi siempre resultan "confirmado-*con corrección*").
2. Si viene de una revisión, decir en qué se equivocó y aplicar la corrección propia.
3. Local: `cargo build --release` y `cargo test`, sin warnings.
   Nube: `cargo wcheck` sin warnings, y después el CI de la rama en verde.
4. Local: **probarlo por medición sobre el binario real**, contra el build del commit anterior
   (o el paquete de `paquetes/`), mismo escenario antes y después. En la nube no se puede:
   dejarlo anotado como pendiente.
5. Local: desplegar el `.exe`, refrescar `paquetes/Khipu Portable <versión>.zip`.
6. Commit con la sección *Unreleased* de `CHANGELOG.md` al día.
7. Informar en español, incluyendo lo que quedó sin probar y por qué.

## Entorno local (Windows)

- Git en `C:\Program Files\Git\cmd`; GitHub CLI (`gh`) con la sesión de `alexpaltrc`.
- Rust estable con `rustup`; fuentes de los crates para verificar en
  `%USERPROFILE%\.cargo\registry\src\index.crates.io-*\`.
- Windows en español: Ctrl+A es *Abrir*, no *Seleccionar todo*.
- Al probar con gestos sintéticos, re-enfocar la ventana antes de cada gesto y de cada
  captura: `PrintWindow` devuelve un fotograma viejo si la ventana no está al frente.
