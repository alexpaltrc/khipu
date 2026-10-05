# Khipu · Percha anudada

Paquete de identidad y recursos, 5 de octubre de 2026. Conserva la **cuarta variante elegida por el usuario**: pájaro mirando a la derecha, ojo circular en negativo, percha ligeramente curva y dos nudos. El violeta de la marca queda fijado por decisión del usuario. Los tonos exactos de pausa son una propuesta refinada, pendiente de aprobación.

## Archivos y usos

| Archivo o carpeta | Uso |
|---|---|
| `Khipu-lamina-final.png` | Lámina de consulta y revisión inmediata, 1600 × 1150 px |
| `presentacion/Khipu-identidad-final.pptx` | Presentación de 10 diapositivas, con texto editable, descripciones de imágenes y fuentes en notas |
| `presentacion/Khipu-identidad.pdf` | Copia visual estática de consulta; el PPTX conserva el contenido editable |
| `fuentes/khipu-simbolo-maestro.svg` | Geometría vectorial maestra, lienzo de 40 unidades y ojo realmente transparente |
| `fuentes/khipu-aplicacion.svg` | Ícono grande en cuadrado violeta con esquinas redondeadas |
| `fuentes/khipu-simbolo-optico-16-24.svg` | Ajuste óptico del ojo, la cuerda y los nudos para 16–24 px |
| `fuentes/tokens.json` | Tokens de marca, adaptación a temas y estado propuesto |
| `fuentes/generar-assets.mjs` | Exportador determinista de PNG e ICO, requiere Node.js y `sharp` |
| `app/Khipu.ico` | Recurso Win32 con PNG RGBA en 16, 20, 24, 32, 40, 48, 64, 128 y 256 px |
| `app/Khipu-256.png` | PNG de aplicación, útil como recurso de ventana o respaldo |
| `bandeja/` | 28 PNG RGBA: 2 temas × 2 estados × 7 tamaños físicos |
| `msix/Assets/` | 62 imágenes para las cuatro entradas del manifiesto y variantes AppList |
| `store/Khipu-app-tile-300.png` | PNG de 300 × 300 px para el campo **1:1 App tile icon** de la ficha Store |
| `inventario.json` | Correspondencia de cada PNG operativo con dimensiones y uso |
| `VALIDACION.md` | Controles realizados y límites de la entrega |

No se necesitan carteles de juego, banners promocionales, BadgeLogo, SplashScreen ni nuevos tamaños de mosaico para el manifiesto revisado. La lámina de identidad es material de diseño y no una captura de la app para la ficha Store. Las capturas reales de producto quedan fuera de este encargo de íconos.

## Paleta

| Tipo de token | Valor | Aplicación |
|---|---|---|
| Marca / violeta | `#A99BE7` | Fondo del ícono grande y ficha Store |
| Marca / tinta | `#211B37` | Pájaro y percha sobre violeta |
| Adaptación / activo oscuro | `#F3F4F7` | Silueta sobre bandeja oscura |
| Adaptación / activo claro | `#22252C` | Silueta sobre bandeja clara |
| Propuesta / pausa oscura | `#9497A1` | Menor intensidad sobre oscuro |
| Propuesta / pausa clara | `#747781` | Menor intensidad sobre claro |
| Muestra / superficie oscura | `#25272D` | Fondo de demostración |
| Muestra / superficie clara | `#ECEEF2` | Fondo de demostración |
| Presentación / fondo | `#17181B` | Fondo editorial |

Los recursos de bandeja tienen transparencia. Las superficies anteriores sirven para demostrar y medir contraste. El tema de la bandeja debe corresponder al shell de Windows, que puede diferir del tema seleccionado dentro de la app. Este paquete no modifica cómo la interfaz sigue el tema o el acento de Windows.

## Pausa y tamaños pequeños

La pausa conserva exactamente la forma y cobertura alfa del activo y cambia el color de la silueta. No contiene barras. Los colores propuestos mantienen contraste de al menos 3:1 contra sus superficies de muestra. El contraste de los píxeles de borde depende del antialiasing.

La intensidad por sí sola puede resultar ambigua. En una integración posterior, acompañarla con tooltip **“Khipu · Activo”** o **“Khipu · Pausado”**, texto de estado visible y la acción **Reanudar**. Una pausa con duración debe indicar el tiempo restante por texto. En alto contraste, usar colores del sistema y conservar esa indicación textual, aunque desaparezca la diferencia de intensidad. Estas recomendaciones no se aplicaron al código.

Los tamaños de bandeja son 16, 20, 24, 32, 40, 48 y 64 px, correspondientes a las escalas de Windows de 100% a 400%. Los archivos indican el tamaño **físico**, no el tamaño lógico. A 16–24 px, el ojo pasa de radio 2.1 a 2.45 unidades, la cuerda de grosor 1.8 a 2.5 y los nudos de radio 2.35 a 2.6. Se conservan el perfil, el pico, la curva y los dos nudos. Los PNG se rasterizan directamente del vector al tamaño indicado.

## Correspondencia MSIX

Se inspeccionó el proyecto localizado en `C:\Users\aleks\Documents\Vibecode\Espanso Manager\espanso-portable\espanso-manager-src`. Es una app Win32 de Rust/egui con empaquetado MSIX. La ruta anterior del chat `Espanso-Win-Portable-x86_64` ya no existe.

El manifiesto `packaging/AppxManifest.xml` apunta a estas cuatro entradas. Los nombres base se resuelven mediante sus variantes calificadas y un índice `resources.pri` generado en el proceso de empaquetado.

| Entrada del manifiesto | Archivos en `msix/Assets` | Dimensiones a 100 / 125 / 150 / 200 / 400% |
|---|---|---|
| `Assets\Square44x44Logo.png` | `Square44x44Logo.scale-N.png` | 44 / 55 / 66 / 88 / 176 px |
| `Assets\Square150x150Logo.png` | `Square150x150Logo.scale-N.png` | 150 / 188 / 225 / 300 / 600 px |
| `Assets\Wide310x150Logo.png` | `Wide310x150Logo.scale-N.png` | 310×150 / 388×188 / 465×225 / 620×300 / 1240×600 px |
| `Assets\StoreLogo.png` | `StoreLogo.scale-N.png` | 50 / 63 / 75 / 100 / 200 px |

`Square44x44Logo.targetsize-T.png` representa AppList con el nombre base usado por este manifiesto. Los tamaños T son 16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96 y 256 px. Cada tamaño incluye la variante default, `_altform-unplated` y `_altform-lightunplated`. Las tres conservan el fondo propio violeta de la marca y transparencia en las esquinas. El calificador evita que Windows añada una placa adicional y no obliga a eliminar el fondo de marca.

Los recursos de mosaico cuadrado y ancho llevan fondo violeta a todo el lienzo, con el símbolo centrado dentro de una caja de 100 unidades sobre una altura de 150. No repiten el cuadrado redondeado dentro del mosaico. Los íconos AppList/StoreLogo conservan el cuadrado redondeado propio. La ficha Store usa un cuadrado de 300 px a fondo completo. Ningún recurso lleva borde ni sombra redundante.

**Integración pendiente:** el script actual `packaging/build-msix.ps1` deriva las imágenes del ICO existente. Una futura integración deberá incorporar estos PNG, preservar sus calificadores y regenerar `resources.pri`; ejecutar hoy ese script no integra automáticamente este paquete. El ICO se conectaría al recurso Win32 de `build.rs`. `src/icons.rs` genera actualmente la bandeja de otra identidad, por lo que esa parte también requiere una modificación posterior autorizada. Aquí no se editó ninguno de esos archivos.

## Fuentes comprobadas

Guías consultadas el 5 de octubre de 2026:

- [Microsoft Learn: Construct your Windows app's icon](https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-construction). Página actualizada el 27 de julio de 2026. Tamaños, escalas y variantes de tema.
- [Microsoft Learn: Add app screenshots, images, and trailers for MSIX apps](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/screenshots-and-images). Campo 1:1 App tile icon de 300 × 300 px y validación mediante Windows App Certification Kit.
- [Museo Larco: Inca Quipus](https://www.museolarco.org/en/exhibition/permanent-exhibition/online-exhibition/textiles-from-ancient-peru/inca-quipus/). Referencia sobre registro de información con cuerdas y nudos. La relación con la expansión de texto es una interpretación de diseño contemporánea.

## Regeneración

Con Node.js y `sharp` disponibles, ejecutar desde la carpeta del paquete:

```powershell
node fuentes/generar-assets.mjs C:/ruta/a/una/carpeta-nueva
```

La salida incluye SVG, tokens, PNG operativos, ICO e inventario. La presentación y la lámina se conservan como entregables de revisión. Usar una carpeta nueva para no reemplazar ajustes posteriores.

## Alcance

Esta entrega prepara recursos para integración y publicación futura. No constituye un MSIX certificado, no incluye una validación de Partner Center y no afirma que la app esté lista para Microsoft Store. Quedan pendientes la integración, pruebas en el shell real con distintos DPI y alto contraste, generación y comprobación del índice de recursos y Windows App Certification Kit. No se publicó nada ni se modificó el comportamiento de la aplicación.
