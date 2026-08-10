// El wasm vive en un worker (ver `worker.js`), así que aquí no se bloquea nada:
// mientras convierte, la página sigue respondiendo y la barra de progreso puede
// moverse de verdad.

const $ = (id) => document.getElementById(id);
const ui = {
  tabs: [...document.querySelectorAll(".tab")],
  panels: { pixelart: $("panel-pixelart"), photo: $("panel-photo") },
  footerNote: $("footerNote"),

  drop: $("drop"),
  file: $("file"),
  workspace: $("workspace"),
  preview: document.querySelector(".preview"),

  original: $("original"),
  originalSkeleton: $("originalSkeleton"),
  originalMeta: $("originalMeta"),
  result: $("result"),
  resultBox: $("resultBox"),
  resultSkeleton: $("resultSkeleton"),
  svgMeta: $("svgMeta"),

  progress: $("progress"),
  progressBar: $("progressBar"),
  progressLabel: $("progressLabel"),
  stats: $("stats"),
  error: $("error"),

  autoScale: $("autoScale"),
  autoScaleLabel: $("autoScaleLabel"),
  scale: $("scale"),
  tolerance: $("tolerance"),
  toleranceOut: $("toleranceOut"),
  alpha: $("alpha"),
  alphaOut: $("alphaOut"),
  autoPixel: $("autoPixel"),
  pixelSize: $("pixelSize"),
  useBackground: $("useBackground"),
  background: $("background"),
  removeChecker: $("removeChecker"),
  removeBackground: $("removeBackground"),
  mergeColors: $("mergeColors"),
  fit: $("fit"),
  fitTolerance: $("fitTolerance"),
  fitToleranceField: $("fitToleranceField"),
  fitToleranceOut: $("fitToleranceOut"),

  photoTolerance: $("photoTolerance"),
  photoToleranceOut: $("photoToleranceOut"),
  gradientStep: $("gradientStep"),
  gradientStepOut: $("gradientStepOut"),
  filterSpeckle: $("filterSpeckle"),
  filterSpeckleOut: $("filterSpeckleOut"),
  minThickness: $("minThickness"),
  minThicknessOut: $("minThicknessOut"),
  colorPrecision: $("colorPrecision"),
  colorPrecisionOut: $("colorPrecisionOut"),
  capColors: $("capColors"),
  maxColors: $("maxColors"),
  photoAlpha: $("photoAlpha"),
  photoAlphaOut: $("photoAlphaOut"),
  photoUseBackground: $("photoUseBackground"),
  photoBackground: $("photoBackground"),
  photoRemoveBackground: $("photoRemoveBackground"),
  photoFit: $("photoFit"),
  photoFitTolerance: $("photoFitTolerance"),
  photoFitToleranceField: $("photoFitToleranceField"),
  photoFitToleranceOut: $("photoFitToleranceOut"),

  download: $("download"),
  copy: $("copy"),
  reset: $("reset"),
  photoDownload: $("photoDownload"),
  photoCopy: $("photoCopy"),
  photoReset: $("photoReset"),
};

/** Metadatos de la imagen cargada; los píxeles se quedan en el worker. */
let source = null;
/** Último SVG generado, para descargar y copiar. */
let svg = "";
/** Id de la petición en vuelo: las respuestas viejas se descartan. */
let request = 0;

// Un modo es una segmentación: sus ajustes, el motor del worker que los lee y
// las cifras que tienen sentido contar de lo que devuelve. `report` está aquí y
// no en `render` porque son dos vocabularios distintos —rejilla y celda contra
// lienzo y regiones— y mezclarlos daría una línea con la mitad vacía.
const MODES = {
  pixelart: {
    options: pixelartOptions,
    report: pixelartReport,
    note:
      "Detecta la rejilla midiendo la periodicidad del gradiente, reduce la " +
      "imagen a sus píxeles reales y traza el contorno de cada región con " +
      '<code>fill-rule="evenodd"</code>.',
  },
  photo: {
    options: photoOptions,
    report: photoReport,
    note:
      "Agrupa los colores en una paleta por cercanía perceptual, etiqueta las " +
      "regiones conexas de cada entrada, funde las que no dibujan nada y saca " +
      "cada frontera una sola vez.",
  },
};

/** `#curves` era el nombre de esta pestaña antes de que tuviera motor. */
const HASH_ALIASES = { curves: "photo" };

function modeFromHash() {
  const hash = location.hash.slice(1);
  return HASH_ALIASES[hash] || (hash in MODES ? hash : "pixelart");
}

let mode = modeFromHash();

/* ----------------------------------------------------------------- worker --- */

const worker = new Worker(new URL("./worker.js", import.meta.url), {
  type: "module",
});

function send(kind, payload, transfer = []) {
  const id = ++request;
  worker.postMessage({ id, kind, ...payload }, transfer);
  return id;
}

worker.onmessage = ({ data }) => {
  // Una respuesta de una petición ya superada no debe pintar nada.
  if (data.id !== request) return;

  if (data.kind === "stage") return stage(data.stage);
  if (data.kind === "error") {
    endProgress();
    return fail(data.message);
  }
  if (data.kind === "done") return render(data.result, data.ms);
};

worker.onerror = (e) => {
  endProgress();
  fail(`El worker ha fallado: ${e.message}`);
};

/* --------------------------------------------------------------- progreso --- */

const STAGES = {
  decode: { at: 15, label: "Decodificando la imagen…" },
  sample: { at: 35, label: "Leyendo los píxeles…" },
  wasm: { at: 55, label: "Cargando el motor…" },
  convert: { at: 75, label: "Convirtiendo…", pulse: true },
};

function stage(name) {
  const step = STAGES[name];
  if (!step) return;
  ui.progress.hidden = false;
  ui.progress.classList.toggle("pulse", Boolean(step.pulse));
  ui.progressBar.style.width = `${step.at}%`;
  ui.progressLabel.textContent = step.label;
  ui.progress.setAttribute("aria-valuenow", String(step.at));
}

function endProgress() {
  ui.progress.classList.remove("pulse");
  ui.progressBar.style.width = "100%";
  // Se deja ver el 100% un instante: desaparecer de golpe se lee como un fallo.
  setTimeout(() => {
    ui.progress.hidden = true;
    ui.progressBar.style.width = "0%";
  }, 180);
}

/* ------------------------------------------------------------------ carga --- */

async function load(blob, name) {
  hideError();
  // El espacio de trabajo aparece ya, con esqueletos: así se ve la forma de la
  // página desde el primer momento en vez de una zona de carga congelada.
  ui.drop.hidden = true;
  ui.workspace.hidden = false;
  ui.preview.hidden = false;
  ui.original.hidden = true;
  ui.originalSkeleton.hidden = false;
  ui.resultSkeleton.hidden = false;
  ui.result.innerHTML = "";
  ui.originalMeta.textContent = "";
  ui.svgMeta.textContent = "";
  ui.stats.textContent = "";
  svg = "";

  stage("decode");
  let bitmap;
  try {
    bitmap = await createImageBitmap(blob);
  } catch {
    endProgress();
    return fail("El navegador no ha podido decodificar esa imagen.");
  }

  stage("sample");
  // Un fotograma para que el esqueleto y la barra lleguen a pintarse antes del
  // `getImageData`, que en una imagen grande sí cuesta.
  await frame();

  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  ctx.drawImage(bitmap, 0, 0);
  const pixels = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
  bitmap.close();

  source = {
    name: name.replace(/\.[^.]+$/, "") || "imagen",
    width: pixels.width,
    height: pixels.height,
    bytes: blob.size,
  };

  ui.original.width = source.width;
  ui.original.height = source.height;
  ui.original.getContext("2d").putImageData(pixels, 0, 0);
  ui.original.hidden = false;
  ui.originalSkeleton.hidden = true;
  ui.originalMeta.textContent = `${source.width}×${source.height} · ${size(blob.size)}`;

  // Los píxeles se transfieren (no se copian) y se quedan en el worker: cada
  // cambio de ajuste manda sólo las opciones.
  const rgba = new Uint8Array(pixels.data);
  send("image", { width: source.width, height: source.height, rgba }, [rgba.buffer]);

  // Cada imagen trae su propia rejilla: se vuelve a detectar.
  ui.autoScale.checked = true;
  ui.scale.disabled = true;
  ui.autoScaleLabel.textContent = "automática";

  convert();
}

/* ------------------------------------------------------------- conversión --- */

// El ajuste es el eje que **no** depende de la segmentación, así que los dos
// modos mandan exactamente las mismas dos claves y las leen los dos lectores.
function fitOptions(select, tolerance) {
  return select.value === "polygon"
    ? { fit: "polygon", fitTolerance: Number(tolerance.value) }
    : { fit: "pixel" };
}

function pixelartOptions() {
  const opts = {
    tolerance: Number(ui.tolerance.value),
    alphaThreshold: Number(ui.alpha.value),
    removeCheckerboard: ui.removeChecker.checked,
    removeBackground: ui.removeBackground.checked,
    mergeColors: ui.mergeColors.checked,
    ...fitOptions(ui.fit, ui.fitTolerance),
  };
  if (!ui.autoScale.checked && Number(ui.scale.value) >= 1) {
    opts.scale = Number(ui.scale.value);
  }
  if (!ui.autoPixel.checked && Number(ui.pixelSize.value) >= 1) {
    opts.pixelSize = Number(ui.pixelSize.value);
  }
  if (ui.useBackground.checked) {
    opts.background = ui.background.value;
  }
  return opts;
}

function photoOptions() {
  const opts = {
    tolerance: Number(ui.photoTolerance.value),
    gradientStep: Number(ui.gradientStep.value),
    filterSpeckle: Number(ui.filterSpeckle.value),
    minThickness: Number(ui.minThickness.value),
    colorPrecision: Number(ui.colorPrecision.value),
    alphaThreshold: Number(ui.photoAlpha.value),
    removeBackground: ui.photoRemoveBackground.checked,
    ...fitOptions(ui.photoFit, ui.photoFitTolerance),
  };
  if (ui.capColors.checked && Number(ui.maxColors.value) >= 2) {
    opts.maxColors = Number(ui.maxColors.value);
  }
  if (ui.photoUseBackground.checked) {
    opts.background = ui.photoBackground.value;
  }
  return opts;
}

function convert() {
  if (!source) return;

  // Si ya hay un resultado, se atenúa en vez de sustituirlo por un esqueleto:
  // ver el anterior mientras llega el nuevo se lee mucho mejor al mover un
  // deslizador.
  if (svg) ui.resultBox.classList.add("stale");
  else ui.resultSkeleton.hidden = false;

  send("convert", { engine: mode, options: MODES[mode].options() });
}

/** `{ meta, stats }`: lo que va bajo el SVG y la línea de cifras. */
function pixelartReport(out) {
  const grid = `${out.gridWidth}×${out.gridHeight}`;
  const cell = `${out.cellWidth.toFixed(2)}×${out.cellHeight.toFixed(2)}`;

  // Con la escala en automático, el campo manual refleja lo detectado para que
  // retocarlo a mano parta de ahí.
  if (ui.autoScale.checked) {
    const detected = Math.max(out.cellWidth, out.cellHeight);
    ui.scale.value = detected.toFixed(2);
    ui.autoScaleLabel.textContent = `automática (${detected.toFixed(2)} px)`;
  }

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
}

function photoReport(out) {
  const canvas = `${out.canvasWidth}×${out.canvasHeight}`;
  return {
    meta: `${canvas} px`,
    stats:
      (out.background ? `fondo ${out.background} quitado · ` : "") +
      `lienzo ${canvas} · ${out.colors} colores · ` +
      `${out.regions} regiones · ${out.paths} paths`,
  };
}

function render(out, ms) {
  svg = out.svg;
  ui.result.innerHTML = svg;
  ui.resultBox.classList.remove("stale");
  ui.resultSkeleton.hidden = true;

  const { meta, stats } = MODES[mode].report(out);
  ui.svgMeta.textContent = `${meta} · ${size(svg.length)}`;
  ui.stats.textContent =
    `${stats} · ${percent(svg.length, source.bytes)} del original · ` +
    `${Math.round(ms)} ms`;

  hideError();
  endProgress();
}

/* ------------------------------------------------------------------ modos --- */

function setMode(next, { convertNow = true } = {}) {
  if (!(next in MODES)) return;
  mode = next;

  for (const tab of ui.tabs) {
    const on = tab.dataset.mode === mode;
    tab.setAttribute("aria-selected", String(on));
    tab.classList.toggle("on", on);
  }
  for (const [name, panel] of Object.entries(ui.panels)) {
    panel.hidden = name !== mode;
  }

  ui.preview.hidden = !source;
  ui.footerNote.innerHTML = MODES[mode].note;

  if (location.hash.slice(1) !== mode) history.replaceState(null, "", `#${mode}`);

  // Cambiar de modo reconvierte siempre: el SVG en pantalla es el de la otra
  // segmentación, y sus cifras describen algo que ya no se está viendo.
  if (convertNow && source) convert();
}

/* ------------------------------------------------------------- utilidades --- */

const frame = () => new Promise((r) => requestAnimationFrame(() => r()));

const size = (bytes) =>
  bytes < 1024
    ? `${bytes} B`
    : bytes < 1024 * 1024
      ? `${(bytes / 1024).toFixed(1)} KB`
      : `${(bytes / 1024 / 1024).toFixed(1)} MB`;

const percent = (a, b) => (b ? `${((a / b) * 100).toFixed(1)}%` : "—");

function fail(message) {
  ui.error.textContent = message;
  ui.error.hidden = false;
}

function hideError() {
  ui.error.hidden = true;
}

let timer;
function schedule() {
  clearTimeout(timer);
  timer = setTimeout(convert, 120);
}

function reset() {
  source = null;
  svg = "";
  ui.workspace.hidden = true;
  ui.drop.hidden = false;
  ui.result.innerHTML = "";
  ui.resultBox.classList.remove("stale");
  endProgress();
  hideError();
}

/* ---------------------------------------------------------------- eventos --- */

for (const tab of ui.tabs) {
  tab.addEventListener("click", () => setMode(tab.dataset.mode));
}
// Flechas entre pestañas, que es lo que espera un `tablist`.
for (const [i, tab] of ui.tabs.entries()) {
  tab.addEventListener("keydown", (e) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = ui.tabs[(i + step + ui.tabs.length) % ui.tabs.length];
    next.focus();
    setMode(next.dataset.mode);
  });
}
addEventListener("hashchange", () => setMode(modeFromHash()));

ui.drop.addEventListener("click", () => ui.file.click());
ui.drop.addEventListener("keydown", (e) => {
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    ui.file.click();
  }
});
ui.file.addEventListener("change", () => {
  const file = ui.file.files[0];
  if (file) load(file, file.name);
  ui.file.value = "";
});

for (const type of ["dragenter", "dragover"]) {
  ui.drop.addEventListener(type, (e) => {
    e.preventDefault();
    ui.drop.classList.add("dragging");
  });
}
for (const type of ["dragleave", "drop"]) {
  ui.drop.addEventListener(type, () => ui.drop.classList.remove("dragging"));
}
document.addEventListener("dragover", (e) => e.preventDefault());
document.addEventListener("drop", (e) => {
  e.preventDefault();
  const file = e.dataTransfer?.files?.[0];
  if (file) load(file, file.name);
});
document.addEventListener("paste", (e) => {
  const item = [...(e.clipboardData?.items || [])].find((i) =>
    i.type.startsWith("image/")
  );
  if (item) {
    const file = item.getAsFile();
    load(file, file.name || "pegado.png");
  }
});

ui.autoScale.addEventListener("change", () => {
  ui.scale.disabled = ui.autoScale.checked;
  if (ui.autoScale.checked) ui.autoScaleLabel.textContent = "automática";
  convert();
});
ui.scale.addEventListener("input", schedule);
ui.tolerance.addEventListener("input", () => {
  ui.toleranceOut.textContent = ui.tolerance.value;
  schedule();
});
ui.alpha.addEventListener("input", () => {
  ui.alphaOut.textContent = ui.alpha.value;
  schedule();
});
ui.autoPixel.addEventListener("change", () => {
  ui.pixelSize.disabled = ui.autoPixel.checked;
  convert();
});
ui.pixelSize.addEventListener("input", schedule);
ui.useBackground.addEventListener("change", () => {
  ui.background.disabled = !ui.useBackground.checked;
  convert();
});
ui.background.addEventListener("input", schedule);
ui.removeChecker.addEventListener("change", convert);
ui.removeBackground.addEventListener("change", convert);
ui.mergeColors.addEventListener("change", convert);

// La desviación sólo la lee el polígono, así que con la escalera se esconde en
// vez de quedarse ahí sin hacer nada.
function syncFit(select, field) {
  field.hidden = select.value !== "polygon";
}

for (const [select, field, tolerance, out] of [
  [ui.fit, ui.fitToleranceField, ui.fitTolerance, ui.fitToleranceOut],
  [
    ui.photoFit,
    ui.photoFitToleranceField,
    ui.photoFitTolerance,
    ui.photoFitToleranceOut,
  ],
]) {
  syncFit(select, field);
  select.addEventListener("change", () => {
    syncFit(select, field);
    convert();
  });
  tolerance.addEventListener("input", () => {
    out.textContent = tolerance.value;
    schedule();
  });
}

// Los dos paneles tienen sus propios botones —cada `<aside>` es el suyo— y
// hacen exactamente lo mismo sobre el último SVG generado.
function download() {
  if (!svg) return;
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = `${source.name}.svg`;
  link.click();
  URL.revokeObjectURL(url);
}

async function copy(button) {
  if (!svg) return;
  try {
    await navigator.clipboard.writeText(svg);
    button.textContent = "¡Copiado!";
    setTimeout(() => (button.textContent = "Copiar"), 1500);
  } catch {
    fail("El navegador ha bloqueado el acceso al portapapeles.");
  }
}

ui.photoTolerance.addEventListener("input", () => {
  ui.photoToleranceOut.textContent = ui.photoTolerance.value;
  schedule();
});
ui.gradientStep.addEventListener("input", () => {
  ui.gradientStepOut.textContent = ui.gradientStep.value;
  schedule();
});
ui.filterSpeckle.addEventListener("input", () => {
  ui.filterSpeckleOut.textContent = ui.filterSpeckle.value;
  schedule();
});
ui.minThickness.addEventListener("input", () => {
  ui.minThicknessOut.textContent = ui.minThickness.value;
  schedule();
});
ui.colorPrecision.addEventListener("input", () => {
  ui.colorPrecisionOut.textContent = ui.colorPrecision.value;
  schedule();
});
ui.photoAlpha.addEventListener("input", () => {
  ui.photoAlphaOut.textContent = ui.photoAlpha.value;
  schedule();
});
ui.capColors.addEventListener("change", () => {
  ui.maxColors.disabled = !ui.capColors.checked;
  convert();
});
ui.maxColors.addEventListener("input", schedule);
ui.photoUseBackground.addEventListener("change", () => {
  ui.photoBackground.disabled = !ui.photoUseBackground.checked;
  convert();
});
ui.photoBackground.addEventListener("input", schedule);
ui.photoRemoveBackground.addEventListener("change", convert);

for (const button of [ui.download, ui.photoDownload]) {
  button.addEventListener("click", download);
}
for (const button of [ui.copy, ui.photoCopy]) {
  button.addEventListener("click", () => copy(button));
}
for (const button of [ui.reset, ui.photoReset]) {
  button.addEventListener("click", reset);
}

setMode(mode, { convertNow: false });
