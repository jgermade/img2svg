import { defineConfig } from "vite";
import preact from "@preact/preset-vite";

// `base` relativo a propósito: el sitio se publica en GitHub Pages bajo
// `/img2svg/`, no en la raíz de un dominio. Con el `/` por omisión todo
// funcionaría en local y daría 404 en producción, que es el fallo que no se ve
// hasta la release.
//
// La raíz de Vite es el directorio de este fichero (`web/`), así que `publicDir`
// y las rutas de abajo cuelgan de ahí.
export default defineConfig({
  base: "./",
  publicDir: "static",
  plugins: [preact()],
  build: {
    // El wasm son ~200 KB: inlinearlo en base64 lo haría crecer un tercio y
    // dejaría de cachearse aparte.
    assetsInlineLimit: 4096,
  },
});
