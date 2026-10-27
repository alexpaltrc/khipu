# Khipu web

Web de presentación basada en la propuesta **Con carácter**: violeta fijo, Percha anudada y composición tipográfica expresiva. Sitio estático con HTML, CSS y JavaScript nativos. Sin dependencias de ejecución, formularios, servicios externos de fuentes ni analítica.

## Revisión local

Con Node.js 20 o posterior:

```powershell
npm run dev
```

Abrir `http://127.0.0.1:4319`. Para cambiar el puerto, definir `PORT` antes de iniciar. El servidor solo escucha en el equipo local. También se puede servir la carpeta `public` con cualquier servidor estático.

## Contenido

- Portada, demostración interactiva con cuatro ejemplos, funciones verificadas contra el README de la app, sección de instalación y preguntas frecuentes.
- Enlaces reales a `https://github.com/alexpaltrc/espanso-manager` y sus issues. El repositorio conserva el nombre anterior.
- Página de privacidad del sitio y página 404.
- Íconos SVG de la identidad aprobada. No se alteran los recursos de la aplicación.
- Diseño adaptable, controles accesibles por teclado y respeto a movimiento reducido.

## Microsoft Store

`public/config.js` contiene la configuración pública. `storeUrl` está en `null` hasta contar con la URL oficial. La página muestra **Próximamente**. Cuando haya una ficha disponible, colocar su enlace HTTPS de `apps.microsoft.com` en `storeUrl`; la web mostrará el botón **Instalar desde Microsoft Store** y enlazará directamente a esa ficha. No usar la página general de Store como sustituto de la ficha del producto.

El código comprueba el protocolo HTTPS y el dominio antes de activar el enlace. No incluye claves ni credenciales. Los nombres, correos y respuestas de la demostración son ficticios. El ejemplo de fecha usa la fecha local del navegador.

## Cloudflare, preparado para después

`wrangler.jsonc` prepara Workers Static Assets para servir `public`. No tiene dominio, credenciales ni recursos de pago configurados. No se registró ningún sitio ni se hizo un despliegue.

Para una publicación futura, utilizar Wrangler con la cuenta de Cloudflare del usuario y asignar el dominio o subdominio después de revisar la web. Recomendación: fijar una versión de Wrangler y su lockfile al iniciar esa integración, y validar el despliegue antes de publicarlo. [Guía oficial](https://developers.cloudflare.com/workers/static-assets/get-started/).

La publicación deberá añadir la URL canónica y, si se decide, el sitemap, y revisar la política de privacidad según el alojamiento y servicios finalmente utilizados. No se inventa un dominio ni una URL de Store. La web no realiza solicitudes desde JavaScript: toda la demostración funciona localmente.

## Archivos

- `public/index.html`: página principal.
- `public/styles.css`: sistema visual y adaptación a pantallas.
- `public/app.js`: ejemplos, expansión y resolución de enlaces públicos.
- `public/config.js`: GitHub y futura ficha de Store.
- `public/privacidad.html`: privacidad del sitio.
- `public/404.html`: página no encontrada.
- `public/assets/`: SVG oficiales de Khipu.
- `public/_headers`: cabeceras para Cloudflare.
- `server.mjs`: servidor de revisión local.
- `wrangler.jsonc`: configuración para publicación posterior en Cloudflare.

No se modificaron código, binarios, paquetes ni configuración de Khipu. No se publicó la web ni se compró un dominio.

Para recuperar la vista previa en este equipo, ejecutar Abrir-vista-previa.ps1. Inicia el servidor en segundo plano si hace falta y abre la página. El texto secundario sobre el violeta se ajustó a #392F52 para alcanzar contraste AA de texto normal.
