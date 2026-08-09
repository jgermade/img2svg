// El wasm vive en un worker (ver `worker.js`), así que aquí no se bloquea nada:
// mientras convierte, la página sigue respondiendo y la barra de progreso puede
// moverse de verdad.

const $ = (id) => document.getElementById(id);
const ui = {
  tabs: [...document.querySelectorAll(".tab")],
  panels: { pixelart: $("panel-pixelart"), curves: $("panel-curves") },
  footerNote: $("footerNote"),

  drop: $("drop"),
  file: $("file"),
  workspace: $("workspace"),
  preview: document.querySelector(".preview"),
  panes: document.querySelector(".panes"),
  svgFigure: $("result").closest("figure"),

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

  download: $("download"),
  copy: $("copy"),
  reset: $("reset"),
  resetCurves: $("resetCurves"),
};

/** Metadatos de la imagen cargada; los píxeles se quedan en el worker. */
let source = null;
/** Último SVG generado, para descargar y copiar. */
let svg = "";
/** Id de la petición en vuelo: las respuestas viejas se descartan. */
let request = 0;

const MODES = {
  pixelart: {
    /** Sin `convert` no hay motor y el modo se enseña como pendiente. */
    convert: (options) => send("convert", { options }),
    options: pixelartOptions,
    note:
      "Detecta la rejilla midiendo la periodicidad del gradiente, reduce la " +
      "imagen a sus píxeles reales y traza el contorno de cada región con " +
      '<code>fill-rule="evenodd"</code>.',
  },
  curves: {
    convert: null,
    note:
      "Pendiente: segmentación por clustering para agrupar los colores de una " +
      "foto en regiones, y ajuste de Béziers sobre sus contornos.",
  },
};

let mode = location.hash.slice(1) in MODES ? location.hash.slice(1) : "pixelart";

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

function pixelartOptions() {
  const opts = {
    tolerance: Number(ui.tolerance.value),
    alphaThreshold: Number(ui.alpha.value),
    removeCheckerboard: ui.removeChecker.checked,
    removeBackground: ui.removeBackground.checked,
    mergeColors: ui.mergeColors.checked,
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

function convert() {
  const engine = MODES[mode];
  if (!source || !engine.convert) return;

  // Si ya hay un resultado, se atenúa en vez de sustituirlo por un esqueleto:
  // ver el anterior mientras llega el nuevo se lee mucho mejor al mover un
  // deslizador.
  if (svg) ui.resultBox.classList.add("stale");
  else ui.resultSkeleton.hidden = false;

  engine.convert(engine.options());
}

function render(out, ms) {
  svg = out.svg;
  ui.result.innerHTML = svg;
  ui.resultBox.classList.remove("stale");
  ui.resultSkeleton.hidden = true;

  const grid = `${out.gridWidth}×${out.gridHeight}`;
  const cell = `${out.cellWidth.toFixed(2)}×${out.cellHeight.toFixed(2)}`;
  ui.svgMeta.textContent = `${grid} px · ${size(svg.length)}`;
  ui.stats.textContent =
    (out.checkerCell
      ? `damero de ${out.checkerCell.toFixed(0)} px quitado ` +
        `(${(out.checkerCoverage * 100).toFixed(0)}% a transparente) · `
      : "") +
    (out.background ? `fondo ${out.background} quitado · ` : "") +
    `rejilla ${grid} · celda ${cell} px · ${out.colors} colores · ` +
    `${out.paths} paths · ${percent(svg.length, source.bytes)} del original · ` +
    `${Math.round(ms)} ms`;

  // Con la escala en automático, el campo manual refleja lo detectado para que
  // retocarlo a mano parta de ahí.
  if (ui.autoScale.checked) {
    const detected = Math.max(out.cellWidth, out.cellHeight);
    ui.scale.value = detected.toFixed(2);
    ui.autoScaleLabel.textContent = `automática (${detected.toFixed(2)} px)`;
  }

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

  // Sin motor no hay SVG que enseñar; el original se queda para que se vea que
  // la imagen sigue cargada, a lo ancho y sin las cifras del otro modo, que no
  // describen nada de lo que hay en pantalla.
  const hasEngine = Boolean(MODES[mode].convert);
  ui.svgFigure.hidden = !hasEngine;
  ui.panes.classList.toggle("single", !hasEngine);
  ui.preview.hidden = !source;
  ui.footerNote.innerHTML = MODES[mode].note;
  if (!hasEngine) ui.stats.textContent = "";

  if (location.hash.slice(1) !== mode) history.replaceState(null, "", `#${mode}`);
  if (convertNow && hasEngine && source && !svg) convert();
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
addEventListener("hashchange", () => setMode(location.hash.slice(1)));

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

ui.download.addEventListener("click", () => {
  if (!svg) return;
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = `${source.name}.svg`;
  link.click();
  URL.revokeObjectURL(url);
});

ui.copy.addEventListener("click", async () => {
  if (!svg) return;
  try {
    await navigator.clipboard.writeText(svg);
    ui.copy.textContent = "¡Copiado!";
    setTimeout(() => (ui.copy.textContent = "Copiar"), 1500);
  } catch {
    fail("El navegador ha bloqueado el acceso al portapapeles.");
  }
});

ui.reset.addEventListener("click", reset);
ui.resetCurves.addEventListener("click", reset);

setMode(mode, { convertNow: false });
