#!/usr/bin/env node
//
// Ejerce el wasm por la misma puerta que la página, sin navegador.
//
//   scripts/web-smoke.mjs
//
// Necesita `web/pkg/` compilado (ver docs/development.md). Carga
// `web/pkg/img2svg.js` y le pasa los bytes del `.wasm` a mano: el arranque por
// omisión se baja una URL, y dándole los bytes no hace falta ni servidor ni
// Chrome.
//
// Qué comprueba, y por qué cada cosa:
//
//   1. Que sale un SVG. Es el humo: si el wasm no arranca, aquí se ve.
//   2. Que están **todos los campos que copia `web/worker.js`**. Ese objeto es
//      el acoplamiento real entre la página y el wasm, y un getter renombrado
//      no da error ahí: da `undefined`, que llega a la página como un hueco.
//   3. Que las **claves de opciones se leen**. `read_config` las saca por
//      cadena con `Reflect::get` y una errata no falla, se queda con el valor
//      por omisión. El `.d.ts` versionado congela la forma declarada de la API;
//      esto es lo único que dice que además está conectada.
//
// El dibujo va aquí dentro, como en `tests/golden.rs`: el wasm se compila sin
// códecs de imagen —los decodifica el navegador— así que no podría abrir un PNG
// aunque se lo diéramos.

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const root = new URL("..", import.meta.url);
const pkg = new URL("web/pkg/", root);

/**
 * Campos que `web/worker.js` copia de cada resultado, uno por segmentación.
 *
 * Los `opcionales` valen `undefined` de pleno derecho —no hay damero, no hay
 * fondo que quitar—, así que de ésos sólo se puede exigir que el getter siga
 * existiendo. Es lo justo que hace falta: lo que se busca es el renombrado, y
 * un getter que ya no está deja de aparecer en el prototipo.
 */
const CAMPOS = {
  pixelart: {
    obligatorios: [
      "svg",
      "gridWidth",
      "gridHeight",
      "cellWidth",
      "cellHeight",
      "colors",
      "paths",
      "subpaths",
    ],
    opcionales: ["checkerCell", "checkerCoverage", "background"],
  },
  photo: {
    obligatorios: [
      "svg",
      "canvasWidth",
      "canvasHeight",
      "colors",
      "paths",
      "subpaths",
      "regions",
    ],
    opcionales: ["background"],
  },
};

let fallos = 0;

function comprueba(condicion, mensaje) {
  console.log(`${condicion ? "  ok  " : "FALLA "} ${mensaje}`);
  if (!condicion) fallos += 1;
}

/**
 * Rasteriza arte ASCII a `escala`x, igual que el `raster` de `tests/golden.rs`.
 * El sprite es asimétrico a propósito: un motivo que se repita hace que la
 * detección de rejilla vea una que no está.
 */
function raster(filas, escala, paleta) {
  const [cols, lineas] = [filas[0].length, filas.length];
  const [w, h] = [cols * escala, lineas * escala];
  const buf = new Uint8Array(w * h * 4);
  filas.forEach((fila, y) => {
    if (fila.length !== cols) throw new Error("todas las filas miden lo mismo");
    [...fila].forEach((ch, x) => {
      const px = paleta[ch];
      if (!px) return;
      for (let dy = 0; dy < escala; dy += 1) {
        const base = ((y * escala + dy) * w + x * escala) * 4;
        for (let dx = 0; dx < escala; dx += 1) buf.set(px, base + dx * 4);
      }
    });
  });
  return { width: w, height: h, data: buf };
}

const SPRITE = [
  "...###########..",
  "..#############.",
  "..##o#######o##.",
  "..###o#####o###.",
  "..#####ooo#####.",
  "..#############.",
  "..#.###########.",
  "...####ooo####..",
  "....##ooooo##...",
  "....ooo###ooo...",
  "...oo.......oo..",
];

const PALETA = {
  "#": [20, 30, 40, 255],
  o: [220, 60, 50, 255],
  "+": [250, 250, 250, 255],
};

/** Un degradado vertical con dos bloques planos encima, para el camino de foto. */
function foto() {
  const [w, h] = [64, 48];
  const buf = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const base = (y * w + x) * 4;
      const rampa = 40 + Math.round((y * 180) / h);
      let px = [30, 40, rampa, 255];
      if (y > 8 && y < 22 && x > 6 && x < 28) px = [197, 41, 41, 255];
      if (y > 26 && y < 40 && x > 34 && x < 58) px = [41, 156, 58, 255];
      buf.set(px, base);
    }
  }
  return { width: w, height: h, data: buf };
}

/**
 * Cifras del `d` de todos los paths. Es proporcional a los vértices dibujados,
 * que es lo que tiene que bajar cuando el ajustador de polígono entra de verdad.
 */
function cifras(svg) {
  let total = 0;
  for (const [, d] of svg.matchAll(/\sd="([^"]*)"/g)) {
    total += (d.match(/-?\d+(\.\d+)?/g) ?? []).length;
  }
  return total;
}

const init = await import(new URL("img2svg.js", pkg));
await init.default({
  module_or_path: await readFile(fileURLToPath(new URL("img2svg_bg.wasm", pkg))),
});

const sprite = raster(SPRITE, 3, PALETA);
const dibujo = foto();

const casos = [
  {
    nombre: "pixelart",
    convert: (opciones) =>
      init.convertRgba(sprite.width, sprite.height, sprite.data, opciones),
  },
  {
    nombre: "photo",
    convert: (opciones) =>
      init.convertPhoto(dibujo.width, dibujo.height, dibujo.data, opciones),
  },
];

for (const { nombre, convert } of casos) {
  console.log(`\n${nombre}`);

  const { obligatorios, opcionales } = CAMPOS[nombre];
  const base = convert({ fit: "pixel" });

  console.log(
    "  " +
      [...obligatorios, ...opcionales]
        .filter((campo) => campo !== "svg")
        .map((campo) => `${campo}=${base[campo]}`)
        .join("  "),
  );

  comprueba(
    typeof base.svg === "string" && base.svg.startsWith("<svg"),
    "devuelve un SVG",
  );
  for (const campo of [...obligatorios, ...opcionales]) {
    comprueba(campo in base, `worker.js lee ${campo}`);
  }
  for (const campo of obligatorios) {
    comprueba(base[campo] !== undefined, `${campo} trae valor`);
  }

  // La clave que la página manda de verdad, con una tolerancia lo bastante
  // grande como para que la diferencia no dependa del dibujo.
  const ajustado = convert({ fit: "polygon", fitTolerance: 8 });
  const [antes, despues] = [cifras(base.svg), cifras(ajustado.svg)];
  comprueba(
    despues < antes,
    `fitTolerance llega al ajustador (${antes} -> ${despues} cifras de path)`,
  );

  // Y lo que pasa con un nombre que no existe: cae en el de píxel, que es el
  // que siempre dibuja algo. Está documentado en `read_fit`, así que se fija.
  comprueba(
    convert({ fit: "ni idea" }).svg === base.svg,
    "un ajustador desconocido cae en el de píxel",
  );

  // El de curvas se pide por su nombre y tiene su propia tolerancia por
  // omisión, así que basta con nombrarlo: si la clave no llegara, saldría la
  // escalera y no habría ni una `c`.
  const curvo = convert({ fit: "spline", fitTolerance: 2 });
  comprueba(/c[-\d]/.test(curvo.svg), "fit: spline emite curvas");
  curvo.free();

  base.free();
  ajustado.free();
}

// El avance sólo lo cuenta el camino de foto, y es lo único de la API de JS que
// no se puede comprobar mirando lo que devuelve: hay que ver si llaman.
console.log("\nprogreso");
const avisos = [];
const conProgreso = init.convertPhoto(
  dibujo.width,
  dibujo.height,
  dibujo.data,
  { onProgress: (v) => avisos.push(v) },
);
conProgreso.free();

comprueba(avisos.length > 1, `onProgress se llama (${avisos.length} veces)`);
comprueba(
  avisos.every((v, i) => v >= 0 && v <= 1 && (i === 0 || v >= avisos[i - 1])),
  "los avisos van de 0 a 1 y no retroceden",
);
comprueba(avisos.at(-1) === 1, `el último aviso es 1 (es ${avisos.at(-1)})`);
comprueba(
  avisos.length <= 101,
  `no se avisa más de una vez por tanto por ciento (${avisos.length})`,
);

console.log(fallos === 0 ? "\ntodo en orden" : `\n${fallos} comprobaciones mal`);
process.exit(fallos === 0 ? 0 : 1);
