import { useEffect, useRef, useState } from "preact/hooks";
import { DropZone } from "../components/DropZone.jsx";
import * as converter from "../services/converter.js";
import { Header } from "./Header.jsx";
import { Footer } from "./Footer.jsx";
import { Preview } from "./Preview.jsx";
import { PixelartPanel } from "./PixelartPanel.jsx";
import { PhotoPanel } from "./PhotoPanel.jsx";
import { MODES, modeFromHash } from "./modes.jsx";

const PANELS = { pixelart: PixelartPanel, photo: PhotoPanel };

export function App() {
  const [mode, setMode] = useState(modeFromHash);
  const [settings, setSettings] = useState(() => ({
    pixelart: { ...MODES.pixelart.defaults },
    photo: { ...MODES.photo.defaults },
  }));

  const active = converter.active.value;
  const error = converter.error.value;

  /** Convierte con los ajustes de un modo. Un solo sitio que arma opciones. */
  function convert(which, values, { debounce = false } = {}) {
    converter.convert(which, MODES[which].options(values), { debounce });
  }

  // Un cambio de ajuste: se guarda y se convierte. Rebota o no según el tipo de
  // control, que es quien lo sabe.
  function change(patch, { continuous = false } = {}) {
    const values = { ...settings[mode], ...patch };
    setSettings((prev) => ({ ...prev, [mode]: values }));
    convert(mode, values, { debounce: continuous });
  }

  // Cambiar de modo reconvierte siempre: el SVG en pantalla es el de la otra
  // segmentación, y sus cifras describen algo que ya no se está viendo.
  function choose(next) {
    if (!(next in MODES)) return;
    setMode(next);
    if (location.hash.slice(1) !== next) {
      history.replaceState(null, "", `#${next}`);
    }
    convert(next, settings[next]);
  }

  async function open(file, name) {
    if (!(await converter.load(file, name ?? file.name))) return;
    // Cada imagen trae su propia rejilla: se vuelve a detectar.
    const pixelart = { ...settings.pixelart, autoScale: true, scale: "" };
    setSettings((prev) => ({ ...prev, pixelart }));
    convert(mode, mode === "pixelart" ? pixelart : settings[mode]);
  }

  // Los oyentes de documento y de hash se montan una vez, así que leen lo de
  // arriba por referencia y no por copia: si no, se quedarían con los ajustes
  // del primer render.
  const latest = useRef();
  latest.current = { open, choose };

  useEffect(() => {
    const onDragOver = (e) => e.preventDefault();
    const onDrop = (e) => {
      e.preventDefault();
      const file = e.dataTransfer?.files?.[0];
      if (file) latest.current.open(file);
    };
    const onPaste = (e) => {
      const item = [...(e.clipboardData?.items || [])].find((i) =>
        i.type.startsWith("image/"),
      );
      if (!item) return;
      const file = item.getAsFile();
      latest.current.open(file, file.name || "pegado.png");
    };
    const onHash = () => latest.current.choose(modeFromHash());

    document.addEventListener("dragover", onDragOver);
    document.addEventListener("drop", onDrop);
    document.addEventListener("paste", onPaste);
    addEventListener("hashchange", onHash);
    return () => {
      document.removeEventListener("dragover", onDragOver);
      document.removeEventListener("drop", onDrop);
      document.removeEventListener("paste", onPaste);
      removeEventListener("hashchange", onHash);
    };
  }, []);

  // Con la escala en automático, el campo manual refleja lo detectado para que
  // retocarlo a mano parta de ahí. Escribir un **resultado** en el campo no
  // vuelve a convertir: sólo lo haría un cambio del usuario.
  const result = converter.result.value;
  const engine = converter.engine.value;
  useEffect(() => {
    if (engine !== "pixelart" || !result || !settings.pixelart.autoScale) return;
    const detected = Math.max(result.cellWidth, result.cellHeight).toFixed(2);
    if (settings.pixelart.scale === detected) return;
    setSettings((prev) => ({
      ...prev,
      pixelart: { ...prev.pixelart, scale: detected },
    }));
  }, [result, engine]);

  const actions = {
    onDownload: converter.download,
    onCopy: converter.copy,
    onReset: converter.reset,
  };

  return (
    <>
      <Header mode={mode} onSelect={choose} />

      <main>
        <DropZone hidden={active} onFile={(file) => open(file)} />

        <div class="workspace" hidden={!active}>
          {active ? <Preview /> : null}

          {Object.entries(PANELS).map(([id, Panel]) => (
            <Panel
              key={id}
              hidden={id !== mode}
              values={settings[id]}
              onChange={change}
              actions={actions}
            />
          ))}
        </div>

        <p class="error" hidden={!error}>
          {error}
        </p>
      </main>

      <Footer mode={mode} />
    </>
  );
}
