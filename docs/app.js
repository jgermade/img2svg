import init, { convertRgba } from "./pkg/px2svg.js";

const $ = (id) => document.getElementById(id);
const ui = {
  drop: $("drop"),
  file: $("file"),
  workspace: $("workspace"),
  original: $("original"),
  originalMeta: $("originalMeta"),
  result: $("result"),
  svgMeta: $("svgMeta"),
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
};

/** Imagen cargada: RGBA sin comprimir, tal cual sale del canvas. */
let source = null;
/** Último SVG generado. */
let svg = "";
let running = false;
let queued = false;

await init();

/* ---------------------------------------------------------------- carga --- */

async function load(blob, name) {
  let bitmap;
  try {
    bitmap = await createImageBitmap(blob);
  } catch {
    return fail("El navegador no ha podido decodificar esa imagen.");
  }

  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  ctx.drawImage(bitmap, 0, 0);
  const pixels = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
  bitmap.close();

  source = {
    name: name.replace(/\.[^.]+$/, ""),
    width: pixels.width,
    height: pixels.height,
    rgba: new Uint8Array(pixels.data.buffer),
    bytes: blob.size,
  };

  ui.original.width = source.width;
  ui.original.height = source.height;
  ui.original.getContext("2d").putImageData(pixels, 0, 0);
  ui.originalMeta.textContent = `${source.width}×${source.height} · ${size(blob.size)}`;

  // Cada imagen trae su propia rejilla: se vuelve a detectar.
  ui.autoScale.checked = true;
  ui.scale.disabled = true;
  ui.drop.hidden = true;
  ui.workspace.hidden = false;
  hideError();
  convert();
}

/* ------------------------------------------------------------ conversión --- */

function options() {
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

async function convert() {
  if (!source) return;
  if (running) {
    queued = true;
    return;
  }
  running = true;
  ui.stats.classList.add("busy");
  // Un fotograma para que se pinte el estado antes de bloquear con el wasm.
  await new Promise((resolve) => requestAnimationFrame(resolve));

  const started = performance.now();
  let out;
  try {
    out = convertRgba(source.width, source.height, source.rgba, options());
  } catch (err) {
    running = false;
    ui.stats.classList.remove("busy");
    return fail(String(err.message || err));
  }

  svg = out.svg;
  const info = {
    grid: `${out.gridWidth}×${out.gridHeight}`,
    cell: `${out.cellWidth.toFixed(2)}×${out.cellHeight.toFixed(2)}`,
    colors: out.colors,
    paths: out.paths,
    checkerCell: out.checkerCell,
    checkerCoverage: out.checkerCoverage,
    background: out.background,
    cellRounded: Math.max(out.cellWidth, out.cellHeight),
  };
  out.free();

  ui.result.innerHTML = svg;
  ui.svgMeta.textContent = `${info.grid} px · ${size(svg.length)}`;
  ui.stats.textContent =
    (info.checkerCell
      ? `damero de ${info.checkerCell.toFixed(0)} px quitado ` +
        `(${(info.checkerCoverage * 100).toFixed(0)}% a transparente) · `
      : "") +
    (info.background ? `fondo ${info.background} quitado · ` : "") +
    `rejilla ${info.grid} · celda ${info.cell} px · ${info.colors} colores · ` +
    `${info.paths} paths · ${percent(svg.length, source.bytes)} del original · ` +
    `${Math.round(performance.now() - started)} ms`;
  ui.stats.classList.remove("busy");
  hideError();

  // Con la escala en automático, el campo manual refleja lo detectado para que
  // retocarlo a mano parta de ahí.
  if (ui.autoScale.checked) {
    ui.scale.value = info.cellRounded.toFixed(2);
    ui.autoScaleLabel.textContent = `automática (${info.cellRounded.toFixed(2)} px)`;
  }

  running = false;
  if (queued) {
    queued = false;
    convert();
  }
}

/* ------------------------------------------------------------- utilidades --- */

const size = (bytes) =>
  bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024
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

/* ---------------------------------------------------------------- eventos --- */

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

ui.reset.addEventListener("click", () => {
  source = null;
  svg = "";
  ui.workspace.hidden = true;
  ui.drop.hidden = false;
  ui.result.innerHTML = "";
  hideError();
});
