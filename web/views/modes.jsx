// Un modo es una segmentación: sus ajustes, el motor del worker que los lee y
// las cifras que tienen sentido contar de lo que devuelve. `report` está aquí y
// no en la vista previa porque son dos vocabularios distintos —rejilla y celda
// contra lienzo y regiones— y mezclarlos daría una línea con la mitad vacía.

// Desviación de partida de cada ajustador que la lee. La de curvas es otra a
// propósito: el contorno del que se parte es una escalera, y por debajo de 1 px
// la curva se dedica a perseguir los peldaños en vez de la forma.
export const FIT_TOLERANCE = { polygon: 0.75, spline: 1.5 };

export const FIT_OPTIONS = [
  { value: "pixel", label: "Escalera de píxel" },
  { value: "polygon", label: "Polígono simplificado" },
  { value: "spline", label: "Curvas" },
];

// El ajuste es el eje que **no** depende de la segmentación, así que los dos
// modos mandan exactamente las mismas dos claves y las leen los dos lectores.
function fitOptions({ fit, fitTolerance }) {
  return fit in FIT_TOLERANCE ? { fit, fitTolerance } : { fit: "pixel" };
}

// Al cambiar de ajustador se vuelve a su desviación: son dos suelos distintos, y
// arrastrar la del polígono al spline es justo el valor con el que el spline
// sale mal. Lo usan los dos paneles, así que la regla vive una sola vez.
export function fitPatch(fit) {
  const preset = FIT_TOLERANCE[fit];
  return preset === undefined ? { fit } : { fit, fitTolerance: preset };
}

export const MODES = {
  pixelart: {
    name: "Pixel art",
    hint: "rejilla y píxeles cuadrados",
    note: (
      <>
        Detecta la rejilla midiendo la periodicidad del gradiente, reduce la
        imagen a sus píxeles reales y traza el contorno de cada región con{" "}
        <code>fill-rule="evenodd"</code>.
      </>
    ),
    defaults: {
      autoScale: true,
      scale: "",
      tolerance: 12,
      removeChecker: true,
      alpha: 128,
      autoPixel: true,
      pixelSize: "1",
      useBackground: false,
      background: "#ffffff",
      removeBackground: false,
      mergeColors: false,
      fit: "pixel",
      fitTolerance: 0.75,
    },
    options(v) {
      const opts = {
        tolerance: v.tolerance,
        alphaThreshold: v.alpha,
        removeCheckerboard: v.removeChecker,
        removeBackground: v.removeBackground,
        mergeColors: v.mergeColors,
        ...fitOptions(v),
      };
      if (!v.autoScale && Number(v.scale) >= 1) opts.scale = Number(v.scale);
      if (!v.autoPixel && Number(v.pixelSize) >= 1) {
        opts.pixelSize = Number(v.pixelSize);
      }
      if (v.useBackground) opts.background = v.background;
      return opts;
    },
    /** `{ meta, stats }`: lo que va bajo el SVG y la línea de cifras. */
    report(out) {
      const grid = `${out.gridWidth}×${out.gridHeight}`;
      const cell = `${out.cellWidth.toFixed(2)}×${out.cellHeight.toFixed(2)}`;
      return {
        meta: `${grid} px`,
        stats:
          (out.checkerCell
            ? `damero de ${out.checkerCell.toFixed(0)} px quitado ` +
              `(${(out.checkerCoverage * 100).toFixed(0)}% a transparente) · `
            : "") +
          (out.background ? `fondo ${out.background} quitado · ` : "") +
          `rejilla ${grid} · celda ${cell} px · ${out.colors} colores · ` +
          `${out.paths} paths`,
      };
    },
  },

  photo: {
    name: "Foto",
    hint: "sin rejilla, por colores",
    note: (
      <>
        Agrupa los colores en una paleta por cercanía perceptual, etiqueta las
        regiones conexas de cada entrada, funde las que no dibujan nada y saca
        cada frontera una sola vez.
      </>
    ),
    defaults: {
      tolerance: 0.045,
      smoothing: 2,
      // En tanto por ciento, que es como lo enseña el deslizador; la opción del
      // wasm va en tanto por uno.
      minColorShare: 0.2,
      gradientStep: 0,
      // A diferencia de pixelart, aquí no hay ninguna escalera que preservar
      // —sólo la de la retícula—, y enderezarla quita entre un 23% y un 32% del
      // fichero sin que se note en el dibujo.
      fit: "polygon",
      fitTolerance: 0.75,
      removeBackground: false,
      filterSpeckle: 4,
      minThickness: 1,
      colorPrecision: 5,
      capColors: false,
      maxColors: "16",
      alpha: 128,
      useBackground: false,
      background: "#ffffff",
    },
    options(v) {
      const opts = {
        tolerance: v.tolerance,
        smoothing: v.smoothing,
        minColorShare: v.minColorShare / 100,
        gradientStep: v.gradientStep,
        filterSpeckle: v.filterSpeckle,
        minThickness: v.minThickness,
        colorPrecision: v.colorPrecision,
        alphaThreshold: v.alpha,
        removeBackground: v.removeBackground,
        ...fitOptions(v),
      };
      if (v.capColors && Number(v.maxColors) >= 2) {
        opts.maxColors = Number(v.maxColors);
      }
      if (v.useBackground) opts.background = v.background;
      return opts;
    },
    report(out) {
      const canvas = `${out.canvasWidth}×${out.canvasHeight}`;
      return {
        meta: `${canvas} px`,
        stats:
          (out.background ? `fondo ${out.background} quitado · ` : "") +
          `lienzo ${canvas} · ${out.colors} colores · ` +
          `${out.regions} regiones · ${out.paths} paths`,
      };
    },
  },
};

/** `#curves` era el nombre de esta pestaña antes de que tuviera motor. */
const HASH_ALIASES = { curves: "photo" };

export function modeFromHash() {
  const hash = location.hash.slice(1);
  return HASH_ALIASES[hash] || (hash in MODES ? hash : "pixelart");
}
