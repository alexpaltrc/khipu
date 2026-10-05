# Validación y límites

Fecha: 5 de octubre de 2026.

- 91 PNG inventariados más PNG de respaldo de aplicación: dimensiones y decodificación comprobadas contra inventario.
- 28 PNG de bandeja: alfa idéntico entre activo y pausado en cada tema y tamaño, fondo transparente y píxeles interiores opacos.
- Regeneración con el exportador incluido: 93 archivos PNG/ICO idénticos byte a byte.
- ICO: 9 entradas PNG RGBA de 32 bits, decodificadas en los tamaños 16, 20, 24, 32, 40, 48, 64, 128, 256 px.
- Contraste sobre las superficies de muestra: oscuro activo 13.57:1, oscuro pausado 5.12:1, claro activo 13.21:1, claro pausado 3.85:1. Estos valores corresponden al color sólido y no garantizan el contraste de cada píxel antialiasado.
- PPTX: 10 diapositivas, integridad y geometría sin hallazgos, tipografía Segoe UI y reimportación con Artifact Tool comprobadas. Texto editable y descripciones de imágenes incluidas.
- Revisión visual de cada diapositiva y de la lámina final. Las diapositivas históricas usan recortes de los bocetos proporcionados.
- PDF: 10 páginas de consulta estática renderizadas con PDFium. Comparación con los PNG de presentación, error medio máximo 0.000/255 por canal. PDF compuesto por imágenes, sin etiquetado de accesibilidad. El PPTX es la versión editable.

No se abrió el PPTX en PowerPoint ni se aplicaron los recursos a la aplicación. No se generó un MSIX ni resources.pri, no se ejecutó Windows App Certification Kit ni se verificó la resolución de recursos en un paquete instalado. No hay certificado ni aprobación de Microsoft Store. No se probaron bandeja real, alto contraste ni monitores con distinto DPI.

La atenuación exacta del estado pausado es una propuesta del diseñador. Tooltip, texto de estado y acción Reanudar son recomendaciones para una integración futura. Este paquete no cambia tema, acento, comportamiento, recursos o manifiesto de la app.
