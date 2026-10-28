# Khipu · Roadmap de producto y diseño

7 de octubre de 2026 · Propuesta sobre la base local 0.0.6

## Recomendación

Haría que Khipu creciera en tres direcciones: encontrar una expansión sin recordar su activador, adaptar una plantilla sin duplicarla y poder corregir cualquier cambio con tranquilidad. Su ventaja es resolver una tarea cotidiana con muy poca fricción. Cada nueva función debería fortalecer esa ventaja.

La propuesta principal de diseño es **Esencial**: conservar la biblioteca actual y mejorar su jerarquía, los nombres de las expansiones y la edición. La segunda, **Al instante**, evoluciona el buscador global hacia un pequeño espacio para encontrar, completar e insertar. Las dos pueden convivir; desarrollaría primero Esencial y validaría Al instante con usuarios frecuentes.

Este documento propone trabajo; no representa funciones implementadas. La revisión se basa en el código local, sus instrucciones, capturas existentes y documentación oficial. No ejecuté una auditoría de Narrador ni pruebas de compatibilidad de la app durante esta revisión. Las prioridades son criterio de producto, todavía no resultados de investigación con usuarios.

## Lo que ya tenemos y conviene conservar

Khipu ya incluye biblioteca con búsqueda, carpetas, inspector adaptable al ancho, acciones por lote, vista compacta, expansiones de texto y fecha, buscador global con atajo configurable, pausa, importación y exportación, respaldos automáticos, inicio con Windows, cuatro idiomas y una bienvenida con prueba real. También contempla alto contraste, movimiento reducido y el tema de identidad añadido en 0.0.6.

No propondría esas capacidades como novedades. La oportunidad está en hacerlas más útiles o en cubrir lo que aún requiere conocer el motor o manipular archivos.

## Orden de entrega

Esfuerzo relativo: **S** = cambio localizado; **M** = varios flujos o persistencia; **L** = integración con el motor y validación amplia. No son estimaciones de días. Antes de asignar calendario hay que resolver los puntos técnicos señalados.

| Etapa | Cambio | Beneficio esperado | Esfuerzo | Dependencia |
|---|---|---|---|---|
| 1 · Confianza y claridad | 1. Accesibilidad comprobada | Uso completo con teclado, Narrador y escalado | M | Verificar AccessKit y controles propios |
| 1 · Confianza y claridad | 2. Deshacer y recuperar desde la app | Corregir errores sin buscar archivos | M | Snapshot coherente de biblioteca y metadatos |
| 1 · Confianza y claridad | 3. Nombres opcionales, favoritos y búsqueda más flexible | Encontrar por intención, además de por activador | M | Conservar metadatos al editar/importar |
| 1 · Confianza y claridad | 4. Refinamiento Esencial | Crear y editar con menos incertidumbre | S–M | Probar prototipo; mantener flujos existentes |
| 2 · Plantillas útiles | 5. Campos rellenables | Reutilizar una plantilla en casos diferentes | L | Prueba técnica de formularios y foco |
| 2 · Plantillas útiles | 6. Fecha dentro del texto y posición del cursor | Terminar la expansión en el punto correcto | M | Modelo de plantilla y pruebas por aplicación |
| 2 · Plantillas útiles | 7. Crear desde texto copiado | Convertir trabajo repetido en una expansión | S–M | Lectura explícita del portapapeles |
| 3 · Uso diario y equipos | 8. Al instante: vista previa y completar desde el buscador | Insertar sin abrir la biblioteca | M–L | Nombres y campos; validar que siga siendo rápido |
| 3 · Uso diario y equipos | 9. Importación con revisión de cambios | Compartir y actualizar plantillas sin perder ajustes | M–L | Recuperación y formato de intercambio compatible |
| 3 · Uso diario y equipos | 10. Excepciones por aplicación | Controlar dónde debe actuar Khipu | M | Configuración por aplicación del motor |

La etapa 1 puede dividirse en entregas pequeñas. La accesibilidad debe acompañar a todas las siguientes, no convertirse en una revisión al final. El editor de campos es la novedad central de la etapa 2; el resto puede recortarse si compromete su calidad.

## 1. Accesibilidad comprobada

**Qué cambiaría.** Verificar el puente de accesibilidad de eframe/AccessKit y completar nombres, roles, estados y orden de foco de los controles personalizados. Revisar también el buscador global y la devolución del foco a la aplicación de origen.

**Por qué conviene.** Un diseño limpio pierde su valor si una persona no puede encontrar el botón, leer una selección o completar una edición con su tecnología de asistencia. Además, esta revisión suele revelar problemas de navegación que afectan a todos.

**Primera entrega.** Crear, buscar, editar, pausar e insertar usando teclado; Narrador anuncia nombre y estado de los controles; el foco es visible; alto contraste y escalado no ocultan acciones. Probar 100 %, 150 % y 200 %, texto ampliado y cadenas largas en los cuatro idiomas.

**Base técnica.** El manifiesto actual desactiva las funciones predeterminadas de eframe y no enumera `accesskit` entre las habilitadas. Hay controles con `widget_info`, así que parte de la semántica ya existe. Esto justifica verificar la integración; por sí solo no demuestra cómo se comporta el binario con Narrador. Microsoft recomienda semántica accesible, teclado y pruebas con tecnologías de asistencia. [Guía de accesibilidad de Windows](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessibility-overview).

## 2. Deshacer y recuperar desde Khipu

**Qué cambiaría.** Ampliar el Deshacer actual a eliminar, editar y cambios masivos. Añadir “Recuperar…” dentro de Ajustes, con fecha del respaldo, resumen de cambios y restauración selectiva cuando sea viable.

**Por qué conviene.** La confianza permite organizar una biblioteca sin miedo a perder trabajo. Es especialmente útil cuando un usuario cambia el prefijo de muchas expansiones o importa una colección del equipo.

**Primera entrega.** Después de eliminar: “Expansión eliminada · Deshacer”. Para recuperación: elegir un respaldo, ver qué se recuperará y confirmar. Crear una copia del estado actual antes de restaurar. Restaurar también carpetas y estado de activación, no solo textos.

**Base técnica.** Ya existen 20 respaldos rotativos de `base.yml` y `_disabled.yml`; `UndoAction` actualmente cubre movimientos de carpeta. Las carpetas se almacenan en `settings.json`, separado de esos respaldos. Antes de ofrecer recuperación completa hay que guardar conjuntamente el estado lógico necesario y validar que un fallo no deje archivos de momentos distintos. No restaurar indiscriminadamente preferencias de ventana o equipo al recuperar textos.

**Aceptación.** Recuperar un elemento borrado y una modificación masiva conservando texto, activador, carpeta y estado. Los errores de lectura o escritura deben mantener disponible el último estado válido.

## 3. Encontrar por intención

**Qué cambiaría.** Un nombre opcional como “Seguimiento de propuesta”, además de `:followup`. Favoritos y búsqueda que ignore acentos y acepte varias palabras en distinto orden. Orden estable: coincidencia exacta del activador, coincidencias relevantes de nombre/activador y contenido; favoritos ayudan cuando no hay consulta.

**Por qué conviene.** Es más fácil reconocer una intención que memorizar una abreviatura. Conforme crece la biblioteca, el nombre también permite diferenciar textos parecidos.

**Primera entrega.** Nombre opcional en el editor, acción de favorito en el inspector y filtro de favoritos. Si no existe nombre, el activador conserva su protagonismo. La búsqueda actual ya consulta activador y texto: se amplía esa búsqueda, no se reemplaza por un sistema nuevo.

**Base técnica.** `SimpleMatch` ya tiene `label: Option<String>`, pero el editor revisado genera `label: None`. Reutilizar esa capacidad después de asegurar que guardar, duplicar, importar y exportar preserven el nombre. Los metadatos hoy se asocian al activador: nuevas propiedades deben sobrevivir a sus cambios. Evaluar un identificador estable solo si la sincronización de metadatos lo necesita; no introducir una base de datos por anticipación.

**Aceptación.** Encontrar “Reunión de seguimiento” con `reunion seguimiento`; buscar por el texto que contiene; renombrar un activador sin perder su favorito ni su carpeta. Medir fluidez con bibliotecas de 2.000 y 10.000 elementos antes de prometer una escala soportada.

## 4. Esencial: refinar la biblioteca

**Qué cambiaría.** Una acción principal con nombre, “Nueva expansión”, y una jerarquía de fila más reconocible: nombre, activador y extracto. El inspector existente conserva su lugar; evaluar edición dentro de ese contexto para cambios breves.

**Por qué conviene.** El símbolo `+` es compacto, pero una etiqueta explica mejor el primer paso. Los nombres permiten escanear la lista y el editor contextual evita perder la selección o la búsqueda.

**Primera entrega.** Ajustar jerarquía y acción principal, sin rehacer toda la navegación. Conservar el selector de carpetas y la vista compacta. En una ventana estrecha, el detalle ocupa la vista con “Volver”, como ya contempla la app. La maqueta apila las zonas a tamaños de conversación muy estrechos para poder comparar ambas.

**Decisión pendiente.** La edición contextual se mantiene solo si simplifica el flujo frente al editor actual. Debe haber Guardar y Cancelar claros; navegar a otra expansión no puede descartar cambios silenciosamente.

## 5. Plantillas con campos: la función de mayor valor nuevo

**Qué añadiría.** Dentro del editor, “Insertar campo”: texto corto, texto de varias líneas o elección entre opciones. Un mismo campo repetido se pide una sola vez. Botón “Probar” para revisar el resultado antes de usarlo.

**Ejemplo.** `:followup` pide Nombre y Proyecto; al completar Ana y Atlas, genera un correo de seguimiento personalizado. La plantilla no necesita una copia para cada cliente. Una respuesta `:ooo` puede pedir fecha de regreso y contacto alternativo.

**Por qué conviene.** Los textos repetidos suelen ser casi iguales. Resolver esas pequeñas variaciones amplía el uso real de Khipu sin convertirlo en un editor complejo.

**Primera entrega.** Campos de texto y opciones sencillas; valores requeridos claros; Cancelar no inserta nada. La vista previa utiliza datos de ejemplo y no ejecuta comandos.

**Prueba técnica obligatoria.** Espanso ya ofrece formularios, pero Khipu necesita reconocerlos, editarlos y conservarlos. Antes de elegir la interfaz final hay que comprobar el motor incluido, el flujo al escribir el activador, el buscador, cancelación y recuperación del foco. Una interfaz de formulario propia idéntica a la maqueta requiere integración adicional; no aparece automáticamente al guardar YAML. [Formularios de Espanso](https://espanso.org/docs/matches/forms/).

**Aceptación.** Crear, guardar, volver a editar, exportar e importar una plantilla sin perder campos; completarla por teclado; probar Outlook, Teams, Word, un navegador y Bloc de notas, registrando las combinaciones de versiones probadas.

## 6. Fechas integradas y punto de continuación

**Qué añadiría.** Reutilizar el constructor de fechas existente dentro de una plantilla de texto y ofrecer “Dejar el cursor aquí”. Mostrar el resultado mediante una acción de prueba, evitando mantener dos paneles con el mismo texto constantemente.

**Por qué conviene.** Un usuario puede insertar una nota con la fecha actual, una confirmación de reunión o una estructura en la que necesita seguir escribiendo en medio.

**Primera entrega.** Fecha/hora actuales dentro del texto y una única posición de cursor. La fecha elegida por el usuario es un campo, no un cálculo implícito. Fechas relativas, días hábiles y calendarios de empresa quedan para una necesidad posterior comprobada.

**Límite real.** El motor ya soporta variables de fecha y marcas de cursor. Las marcas pueden fallar en editores con autoindentación; no prometer que funcionarán igual en cualquier campo. [Variables básicas y marcas de cursor](https://espanso.org/docs/matches/basics/).

## 7. De texto copiado a expansión

**Qué añadiría.** “Crear desde texto copiado” en el menú de nueva expansión. Leer el texto solo al pulsar esa acción, abrir el editor y pedir el activador. Un atajo adicional sería opcional, después de validar la demanda.

**Por qué conviene.** La mejor ocasión para crear una expansión es justo después de escribir o reutilizar algo. Reduce el esfuerzo inicial sin pedir al usuario que organice toda su biblioteca de antemano.

**Primera entrega.** Solo texto plano, vista previa antes de guardar y un mensaje útil si el portapapeles no contiene texto. Sin vigilancia continua del portapapeles ni historial de contenidos.

**Mejora de bienvenida asociada.** La bienvenida ya prueba una expansión real. Añadir después una invitación opcional a crear una propia, con tres ejemplos pequeños —firma, enlace y respuesta frecuente—. Mantener Omitir siempre visible; no llenar automáticamente la biblioteca.

## 8. Al instante: evolucionar el buscador global

**Qué cambiaría.** Mostrar nombres y favoritos al abrir, una vista previa legible del resultado seleccionado y, si tiene campos, pasar a completarlos en ese mismo espacio. Las expansiones simples siguen insertándose directamente con Enter.

**Por qué conviene.** Gran parte del uso diario puede resolverse sin abrir la ventana principal. La biblioteca queda como lugar para organizar y mantener.

**Primera entrega.** Vista previa opcional, favoritos y continuidad del teclado. Después, conectar los campos. Conservar el atajo que haya elegido cada usuario; la maqueta no propone sustituirlo.

**Riesgo de producto.** Una ventana más rica también puede estorbar. Comparar el buscador actual con el prototipo en tareas concretas: encontrar una firma, distinguir dos respuestas parecidas y completar una plantilla. Si la vista previa añade pasos a las expansiones simples, hacerla opcional o descartarla.

**Riesgo técnico.** Medir apertura, navegación y devolución del foco. El formulario no debe pegar en una ventana que el usuario abrió después por accidente. El ejemplo interactivo solo simula el resultado, sin leer ni escribir otras aplicaciones.

## 9. Compartir y actualizar con control

**Qué cambiaría.** Mantener el intercambio mediante archivo y añadir revisión de conflictos: Conservar el mío, Reemplazar o Importar con otro activador. Mostrar el antes y después y recordar qué elementos se omitieron.

**Por qué conviene.** Un equipo puede distribuir respuestas consistentes y actualizarlas sin que cada persona pierda sus adaptaciones. No requiere cuentas ni un servicio central desde la primera entrega.

**Primera entrega.** Comparación por activador y elección explícita por conflicto, con respaldo previo. Mantener intactas las reglas avanzadas no admitidas y explicar cuáles no pueden importarse o editarse.

**Base técnica.** La importación actual omite activadores existentes; eso protege los datos, pero no permite actualizar fácilmente una plantilla compartida. Una biblioteca de equipo con versiones, permisos o sincronización es una fase distinta y necesita demanda real.

**Aceptación.** Importar dos veces el mismo archivo no duplica datos. Actualizar una plantilla conserva o cambia sus propiedades según la decisión mostrada. Un archivo parcialmente inválido no causa un reemplazo silencioso.

## 10. Excepciones por aplicación

**Qué añadiría.** En Ajustes, “No expandir en estas aplicaciones”, con un selector de ejecutable y una lista corta editable. Más adelante, conjuntos de expansiones limitados a una aplicación si los usuarios lo necesitan.

**Por qué conviene.** Hay campos y herramientas donde una abreviatura puede tener otro significado. Una excepción persistente evita pausar y reactivar todo Khipu repetidamente.

**Primera entrega.** Exclusión por programa, con una explicación de su alcance. Excluir un navegador afecta a ese navegador; no se presentará como bloqueo fiable por sitio web. Mantener la pausa global actual.

**Base técnica.** Espanso soporta configuración por aplicación y `enable: false`. Antes de escribirla, comprobar precedencias y preservar configuraciones manuales existentes; no sobrescribirlas. [Configuración por aplicación](https://espanso.org/docs/configuration/app-specific-configurations/).

## Dirección visual

### Esencial · recomendación principal

La biblioteca sigue siendo familiar: búsqueda y selector de carpetas arriba, expansiones en filas y detalle cuando se selecciona una. La principal diferencia es la jerarquía de información y la cercanía de las acciones.

Conservaría el pájaro y el khipu originales, Segoe UI Variable para la interfaz y una monoespaciada para activadores. El violeta identifica una selección, una acción importante o un campo; los fondos permanecen tranquilos. El tema de identidad ya existente mantiene su carácter y su condición de opción descubierta. No cambiaría silenciosamente el tema que usa cada persona.

Evitaría una barra lateral permanente: la app ya la retiró por el espacio que consumía. Tampoco convertiría cada expansión en una tarjeta grande. El diseño debe seguir sirviendo cuando hay cien textos, no solo cuando hay seis.

### Al instante · segunda propuesta

Una ventana compacta centrada en la tarea: buscar, reconocer el texto y usarlo. Al elegir una plantilla con campos, la misma superficie cambia al formulario y permite volver sin perder la búsqueda. Los favoritos sustituyen a un listado arbitrario cuando no se ha escrito nada.

Tiene más potencial para quien usa Khipu muchas veces al día; también exige más cuidado con foco, compatibilidad y velocidad. La validaría después de los nombres opcionales, sin reemplazar de golpe el buscador existente.

### Movimiento y lenguaje de Windows 11

Mantendría microtransiciones breves para orientar: cambio de selección, apertura del detalle y confirmación de guardado. Como punto de partida a probar: 120–180 ms, desplazamientos de 4–8 px y desaceleración suave. No aplicar el zoom del video promocional a cada acción cotidiana. Respetar la preferencia de movimiento reducido, que la app ya contempla, y evitar animaciones continuas que mantengan renderizando la ventana inactiva.

Priorizaría teclado, foco, contraste, escalado, diálogos del sistema y comportamiento correcto de la ventana, incluidos los controles nativos que permiten las funciones de Windows. El hecho de usar Rust + egui no impide una experiencia coherente, pero tampoco concede automáticamente el comportamiento de controles WinUI.

Mica sería una exploración técnica opcional y acotada, no un requisito del rediseño. Microsoft lo plantea como material de fondo de ventanas y exige condiciones de respaldo a color sólido. En este renderizador hay que verificar compatibilidad, rendimiento y estados del sistema antes de ofrecerlo. No reescribiría la aplicación en WinUI 3 solo para conseguir ese efecto. [Material Mica](https://learn.microsoft.com/en-us/windows/apps/design/style/mica).

### Contraste como criterio de aceptación

Objetivos para los pares reales de cada tema: texto normal al menos 4,5:1; texto grande y controles/indicadores pertinentes al menos 3:1, además de foco visible y estados distinguibles sin depender únicamente del color. Medir texto sobre su superficie efectiva y revisar los estados seleccionado, error y alto contraste. Los controles deshabilitados tienen excepciones en WCAG, aunque conviene que sigan siendo reconocibles. Una paleta bonita aislada no equivale a una interfaz accesible. La propuesta no certifica conformidad de la app. Referencias: [contraste de texto, WCAG 2.2](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) y [contraste de componentes](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html).

## Cómo decidir si las mejoras funcionan

Probar con cinco compañeros: algunos nuevos y otros que ya usan Khipu. Es una evaluación cualitativa pequeña, útil para detectar dificultades; no una medición representativa de toda la audiencia.

1. Crear una expansión propia y usarla en otra aplicación.
2. Encontrar una expansión cuyo activador no recuerdan.
3. Cambiar y recuperar un texto eliminado.
4. Completar una plantilla con dos campos.
5. Actualizar una plantilla compartida sin perder una personal.

Registrar si completan la tarea sin ayuda, dónde dudan, errores y tiempo observado. Comparar con la app actual en tareas equivalentes; alternar el orden de las variantes para reducir el efecto de aprendizaje. Hacer una pasada dedicada con teclado, Narrador, alto contraste y escalado. No inventar minutos ahorrados ni porcentajes de productividad.

## Lo que dejaría fuera de este ciclo

No priorizaría cuentas, nube propia, generación por IA, estadísticas de productividad, rachas, un editor de texto enriquecido o una reescritura del framework. Pueden tener valor en otro contexto, pero ahora añaden mantenimiento y decisiones antes de resolver los problemas centrales. Una sincronización futura necesita diseñar conflictos y recuperación; compartir archivos es un primer paso más acotado.

## Referencias de la revisión local

Base consultada: commit local `bebb488`, Cargo `0.0.6`. No se modificó el código de la aplicación para elaborar esta propuesta.

| Evidencia | Dónde revisarla |
|---|---|
| Plataforma y funciones de eframe | [Cargo.toml](C:/Users/aleks/Documents/Vibecode/Khipu/espanso-portable/espanso-manager-src/Cargo.toml) |
| Búsqueda por activador y texto; límites del buscador | [src/ui/search_view.rs](C:/Users/aleks/Documents/Vibecode/Khipu/espanso-portable/espanso-manager-src/src/ui/search_view.rs), función `hits` |
| Inspector existente y adaptación al ancho | [src/ui/list_view.rs](C:/Users/aleks/Documents/Vibecode/Khipu/espanso-portable/espanso-manager-src/src/ui/list_view.rs), `inspector` |
| Editor con tipos Text y Date | [src/ui/edit_form.rs](C:/Users/aleks/Documents/Vibecode/Khipu/espanso-portable/espanso-manager-src/src/ui/edit_form.rs), `Kind` |
| Nombre opcional ya presente en el modelo | [src/yaml/model.rs](C:/Users/aleks/Documents/Vibecode/Khipu/espanso-portable/espanso-manager-src/src/yaml/model.rs), `SimpleMatch` |
| Respaldos de dos YAML y retención de 20 | [src/yaml/io.rs](C:/Users/aleks/Documents/Vibecode/Khipu/espanso-portable/espanso-manager-src/src/yaml/io.rs), `backup` |
| Metadatos de carpetas y temas | [src/settings.rs](C:/Users/aleks/Documents/Vibecode/Khipu/espanso-portable/espanso-manager-src/src/settings.rs), `Settings` |
| Deshacer actual | [src/app.rs](C:/Users/aleks/Documents/Vibecode/Khipu/espanso-portable/espanso-manager-src/src/app.rs), `UndoAction` |
| Intercambio y omisión de activadores existentes | [src/transfer.rs](C:/Users/aleks/Documents/Vibecode/Khipu/espanso-portable/espanso-manager-src/src/transfer.rs), `merge` |

Los mockups que acompañan esta propuesta usan datos de ejemplo. Sirven para comparar jerarquía y flujos; no son capturas de una versión ya desarrollada.
