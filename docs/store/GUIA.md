# Khipu en la Microsoft Store

*Actualizada el 2026-11-22 para la actualización a la 0.0.10. Las pantallas son las que se comprobaron el 2026-10-07 (skill `subir-a-microsoft-store`); si alguna no coincide, mándame la captura.*

*Las imágenes y capturas que esta guía y las fichas (`ficha-<idioma>.md`) mencionan no están en el
repo: viven en `Documentos\Vibecode Archivos\Khipu Archivos\marca\store\`. Las capturas vigentes
(cuatro por idioma, con su `leyendas.txt`) están en `marca\campana-store\Store\Capturas\<idioma>\`.*

Khipu está en la Store, **pública**, desde que se aprobó el envío 2 (0.0.6): Store ID
`9NXXV15RJ94T`, `https://apps.microsoft.com/detail/9NXXV15RJ94T`. Todo lo que se hace en Partner
Center lo haces tú; aquí va casilla por casilla.

## Ahora: actualización a la 0.0.10

Un envío nuevo con el paquete nuevo, sus novedades, y las cuatro capturas nuevas por idioma con
sus leyendas (se renovaron el 2026-11-21 y aún no se han subido). La descripción, las
características y el video se quedan como están.

### Empezar

- [ ] Partner Center › Apps y juegos › **Khipu** › **«Iniciar actualización»** (o «Update»). Copia
      el envío publicado: todo llega ya rellenado.

### «Paquetes»

- [ ] «Browse your files» (o arrastra el archivo) y sube `Documentos\Vibecode Archivos\Khipu Archivos\paquetes\Khipu 0.0.10.msix`.
- [ ] En la tabla debe salir «Khipu 0.0.10.msix · v0.0.10.0, X64» con «Windows 10/11 Desktop»
      marcado.
- [ ] Partner Center tacha «Khipu 0.0.9.msix» y avisa de que lo quitará al guardar. Así tiene que
      ser: no pulses «Don’t remove this package». Quien tiene la 0.0.9 recibe la 0.0.10.
- [ ] «Roll out update gradually…» y «Make this update mandatory»: sin marcar.
- [ ] «Guardar».

### «Descripciones de Store», en cada uno de los cuatro idiomas

- [ ] «Novedades de esta versión»: copia la sección **«Novedades de esta versión (0.0.10)»** del
      final de la `ficha.md` de ese idioma (`es-MX`, `en-US`, `fil-PH`, `hi-IN`). Todas miden
      menos de 1500 caracteres.
- [ ] «Capturas de pantalla»: quita las siete que vienen y sube las cuatro de
      `marca\campana-store\Store\Capturas\<idioma>\`, en orden (01 a 04).
- [ ] La leyenda de cada captura: copia la línea con su número de la sección **«Leyendas de las
      capturas»** de la misma ficha.
- [ ] Lo demás (descripción, características, descripción corta, palabras clave): **déjalo como
      viene**.
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

- [ ] «Enviar para certificación». Mientras se revisa, la 0.0.9 sigue publicada. Tarda hasta 3
      días hábiles.
- [ ] Al publicarse, en `https://apps.microsoft.com/detail/9NXXV15RJ94T` debe salir la versión
      0.0.10 y sus novedades. Quien ya la tiene instalada la recibe solo.

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
- **Rechazo por la norma 10.1** (las capturas no reflejan la app): las cuatro capturas son
  ilustraciones de la app, no la interfaz capturada tal cual. Si lo objetan, las capturas reales de
  antes están en `Capturas-opciones\01-anteriores\` y las limpias en `fuentes\` (`cap-*.png`).

## Por qué está así

- **Texto dentro de las capturas.** Microsoft pide capturas de la app y deja la explicación a la
  leyenda, pero las apps grandes (Microsoft 365, Notion, Spotify) ponen un titular encima y la
  Store lo acepta. Desde 2026-11-21 son cuatro ilustraciones con cordones de khipu (portada,
  formulario, buscador, carpetas y «No expandir en»), con los textos y botones reales de la app.
- **Cuatro y no siete**: cada imagen cuenta una sola cosa; lo demás se descubre usando la app.
- **Lo importante va en los dos tercios de arriba**, que es lo que enseñan las miniaturas.
- **Datos de demostración, nunca los tuyos**: Dafne Ruiz y Alex Ortega en los cuatro idiomas, con
  correos `@example.com`.
- **Alt+Espacio en la captura 3**, el atajo de fábrica.
- **Hero art**: el pájaro del icono sobre su propio cordón de khipu, a la derecha y por encima
  del cuarto inferior, porque la Store pone ahí el nombre y los botones.

## Pendiente

1. **Revisión nativa de filipino e hindi**: los textos de `fil-PH` y `hi-IN`, y los titulares de
   sus capturas, no los ha leído ninguna persona hablante nativa. Cada `leyendas.txt` de esos dos
   idiomas lleva debajo de cada leyenda lo que dice en español.
2. **Pública**: hecho con el envío de la 0.0.6.
3. **winget**: no hay que enviar nada. Winget la encuentra por la fuente `msstore`. Con la ficha
   oculta, puede que no salga en sus búsquedas (sin confirmar).
