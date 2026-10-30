# Khipu en la Microsoft Store

*Actualizada el 2026-10-29 para la actualización a la 0.0.8. Las pantallas son las que se comprobaron el 2026-10-07 (skill `subir-a-microsoft-store`); si alguna no coincide, mándame la captura.*

*Las imágenes y capturas que esta guía y las fichas (`ficha-<idioma>.md`) mencionan no están en el
repo: viven en `Documentos\Vibecode Archivos\Khipu Archivos\marca\store\`.*

Khipu está en la Store, **pública**, desde que se aprobó el envío 2 (0.0.6): Store ID
`9NXXV15RJ94T`, `https://apps.microsoft.com/detail/9NXXV15RJ94T`. Todo lo que se hace en Partner
Center lo haces tú; aquí va casilla por casilla.

## Ahora: actualización a la 0.0.8

Un envío nuevo con el paquete nuevo y sus novedades. La ficha, las imágenes y el video se quedan
como están.

### Empezar

- [ ] Partner Center › Apps y juegos › **Khipu** › **«Iniciar actualización»** (o «Update»). Copia
      el envío publicado: todo llega ya rellenado.

### «Paquetes»

- [ ] «Browse your files» (o arrastra el archivo) y sube `Documentos\Vibecode Archivos\Khipu Archivos\paquetes\Khipu 0.0.8.msix` (11 MB).
- [ ] En la tabla debe salir «Khipu 0.0.8.msix · v0.0.8.0, X64» con «Windows 10/11 Desktop»
      marcado.
- [ ] Partner Center tacha «Khipu 0.0.7.msix» y avisa de que lo quitará al guardar. Así tiene que
      ser: no pulses «Don’t remove this package». Quien tiene la 0.0.7 recibe la 0.0.8.
- [ ] «Roll out update gradually…» y «Make this update mandatory»: sin marcar.
- [ ] «Guardar».

### «Descripciones de Store», en cada uno de los cuatro idiomas

- [ ] «Novedades de esta versión»: copia la sección **«Novedades de esta versión (0.0.8)»** del
      final de la `ficha.md` de ese idioma (`es-MX`, `en-US`, `fil-PH`, `hi-IN`). Todas miden
      menos de 1500 caracteres.
- [ ] Lo demás (descripción, capturas, características): **déjalo como viene**.
- [ ] «Guardar» en cada idioma.

### «Opciones de envío»

- [ ] «Publicar este envío tan pronto como supere la certificación»: déjalo como viene, marcado.
- [ ] Si vuelve a salir la caja de `runFullTrust` («¿Por qué necesita…?*»), copia la justificación
      de «Textos para copiar» y comprueba que entró la última frase («…never stored or sent.»).
- [ ] «Guardar».

### Lo demás

- [ ] «Precios y disponibilidad», «Propiedades», «Clasificación por edades» y las notas para el
      revisor: no cambian.

### Enviar

- [ ] «Enviar para certificación». Mientras se revisa, la 0.0.7 sigue publicada. Tarda hasta 3
      días hábiles.
- [ ] Al publicarse, en `https://apps.microsoft.com/detail/9NXXV15RJ94T` debe salir la versión
      0.0.8 y sus novedades. Quien ya la tiene instalada la recibe solo.

## El envío 2 (0.0.6, lanzamiento público)

Enviado el 2026-10-07: paquete 0.0.6, ficha arreglada, visibilidad pública y publicación automática al
certificarse. **Las imágenes de la ficha y el video de presentación de este envío los hizo ChatGPT
6.0 Astra**, no salen de esta carpeta: las capturas, el hero art y el box art de aquí son los del
primer envío y ya no son los publicados.

## Cómo quedó puesto el primer envío

Para saber qué hay en cada sección si un día hay que revisarla. Los nombres son los de la pantalla.

- **«Precios y disponibilidad»:** todos los mercados; «Audiencia pública»; «Hacer que este
  producto esté disponible pero no reconocible en Microsoft Store, y:» › «Solo vínculo directo»;
  Divisa «USD - Estados Unidos», Precio de venta minorista «$ 0».
- **«Propiedades»:** categoría «Productividad»; privacidad «Sí» con la URL
  `https://github.com/alexpaltrc/khipu/blob/main/PRIVACY.md` (norma 10.5.1); sitio web
  `https://github.com/alexpaltrc/khipu`; soporte `https://github.com/alexpaltrc/khipu/issues`;
  desmarcadas las copias de OneDrive (la política promete que nada sale del equipo) y la grabación
  de clips (es para juegos); requisito mínimo: «Teclado».
- **«Clasificación por edades»:** «Todos los demás tipos de aplicaciones», «No» a todo.
- **«Paquetes»:** `paquetes\Khipu 0.0.5.msix`, «Windows 10/11 Desktop».
- **«Descripciones de Store»:** cuatro idiomas, con los campos cruzados que se arreglan arriba.
- **«Opciones de envío»:** publicar al certificarse. La caja de `runFullTrust` no apareció (sí en
  el envío de la 0.0.6, al subir el paquete nuevo).
- **Notas para el revisor:** en «Información adicional sobre pruebas» (fuera del Envío), ya
  escritas; se conservan entre envíos. Se guardan con «Save description», arriba.

## Textos para copiar

Van en inglés porque los leen los certificadores.

**Notas para el revisor** (ya puestas; aquí por si hay que reescribirlas) (376 caracteres)

> No account or sign-in is needed. To test: open Khipu from Start, click + to create an
> expansion (for example trigger :hello, text "Hello world"), save, then type :hello in Notepad
> and it expands. Press Alt+Space in any app to open the search window. If another app already
> owns Alt+Space, Khipu uses Shift+Alt+Space and says so in its Tips screen. The app works fully
> offline.

**Justificación de runFullTrust** (la pide «Opciones de envío» al subir un paquete nuevo) (454 de 500 caracteres)

> Khipu is a text expander (Win32 desktop app, Desktop Bridge). It runs espansod.exe, the
> open-source espanso engine bundled in the package, which reads keyboard input system-wide to
> detect the user's triggers and types the expanded text into the focused app. It also registers
> a global hotkey (Alt+Space) and lives in the notification area. None of this works in an
> AppContainer. No networking code: keystrokes are processed locally, never stored or sent.

## Si algo no deja avanzar

- **«Enviar para certificación» sigue gris:** alguna sección dice «Incompleto». Mándame la
  captura de esa sección.
- **«Descripción*» en rojo en un idioma:** la borraste y no se pegó el texto nuevo. Copia otra vez
  la sección «Descripción» de su ficha.
- **Rechazo por la norma 10.1** (las capturas no reflejan la app): las capturas llevan un titular
  encima de la interfaz real. Si lo objetan, las capturas limpias están en `fuentes\` (`cap-*.png`).

## Por qué está así

- **Texto dentro de las capturas.** Microsoft pide capturas de la app y deja la explicación a la
  leyenda, pero las apps grandes (Microsoft 365, Notion, Spotify) ponen un titular encima y la
  Store lo acepta. La interfaz es siempre la real, capturada del binario sin retoques.
- **Lo importante va en los dos tercios de arriba**, que es lo que enseñan las miniaturas.
- **Datos de demostración, nunca los tuyos**: una persona ficticia por idioma, con correos
  `@example.com`.
- **Alt+Espacio en la diapositiva 2**, el atajo de fábrica. En la de Ajustes (7) sale
  Mayús+Alt+Espacio porque tu Khipu ya tenía Alt+Espacio y la instancia de prueba cayó al segundo,
  como hace la app. Es un estado real, aunque no el más común.
- **Sin la pantalla de bienvenida**: su segunda página exige teclear.
- **Hero art**: el pájaro del icono sobre su propio cordón de khipu, a la derecha y por encima
  del cuarto inferior, porque la Store pone ahí el nombre y los botones.

## Pendiente

1. **Revisión nativa de filipino e hindi**: los textos de `fil-PH` y `hi-IN`, y los titulares de
   sus capturas, no los ha leído ninguna persona hablante nativa.
2. **Pública**: hecho con el envío de la 0.0.6.
3. **winget**: no hay que enviar nada. Winget la encuentra por la fuente `msstore`. Con la ficha
   oculta, puede que no salga en sus búsquedas (sin confirmar).
