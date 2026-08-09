// El wasm se ejecuta aquí y no en el hilo principal. Convertir una imagen
// grande son cientos de milisegundos de cálculo seguido: en el hilo principal
// congelaría la página entera, y con ella cualquier barra de progreso, justo
// durante el rato en que hace falta enseñar que algo está pasando.
//
// Los píxeles se envían una sola vez, al cargar la imagen, y se quedan aquí.
// Cada cambio de ajuste manda sólo las opciones, que son cuatro números.

import init, { convertRgba, convertPhoto } from "./pkg/img2svg.js";

// Cada segmentación tiene su función y su resultado: no comparten casi ninguna
// cifra, así que en vez de un tipo con la mitad de los campos vacíos hay dos, y
// aquí se copia el que toca. Lo que se manda al hilo principal es un objeto
// llano, porque el del wasm deja de ser válido en cuanto se libera.
const ENGINES = {
  pixelart: {
    convert: convertRgba,
    read: (out) => ({
      svg: out.svg,
      gridWidth: out.gridWidth,
      gridHeight: out.gridHeight,
      cellWidth: out.cellWidth,
      cellHeight: out.cellHeight,
      colors: out.colors,
      paths: out.paths,
      subpaths: out.subpaths,
      checkerCell: out.checkerCell,
      checkerCoverage: out.checkerCoverage,
      background: out.background,
    }),
  },
  photo: {
    convert: convertPhoto,
    read: (out) => ({
      svg: out.svg,
      canvasWidth: out.canvasWidth,
      canvasHeight: out.canvasHeight,
      colors: out.colors,
      paths: out.paths,
      subpaths: out.subpaths,
      regions: out.regions,
      background: out.background,
    }),
  },
};

/** Imagen en curso: `{ width, height, rgba }`. */
let image = null;
/** Promesa de arranque del wasm, una sola vez por worker. */
let booting = null;

const reply = (id, kind, data) => self.postMessage({ id, kind, ...data });

/** Arranca el wasm, avisando de que toca esperar si es la primera vez. */
async function boot(id) {
  if (!booting) {
    reply(id, "stage", { stage: "wasm" });
    booting = init();
  }
  await booting;
}

self.onmessage = async ({ data }) => {
  const { id, kind } = data;

  if (kind === "image") {
    image = { width: data.width, height: data.height, rgba: data.rgba };
    reply(id, "ready", {});
    return;
  }

  if (kind !== "convert") return;
  if (!image) return reply(id, "error", { message: "No hay imagen cargada." });

  const engine = ENGINES[data.engine];
  if (!engine) return reply(id, "error", { message: `Modo desconocido: ${data.engine}` });

  try {
    await boot(id);
    reply(id, "stage", { stage: "convert" });

    const started = performance.now();
    const out = engine.convert(image.width, image.height, image.rgba, data.options);

    // Se copia todo lo que hace falta antes de `free()`: el objeto vive en la
    // memoria del wasm y deja de ser válido en cuanto se libera.
    const result = engine.read(out);
    out.free();

    reply(id, "done", { result, ms: performance.now() - started });
  } catch (err) {
    reply(id, "error", { message: String(err?.message || err) });
  }
};
