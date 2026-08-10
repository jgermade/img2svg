import { Tabs } from "../components/Tabs.jsx";
import { MODES } from "./modes.jsx";

const TABS = Object.entries(MODES).map(([id, { name, hint }]) => ({
  id,
  name,
  hint,
}));

export function Header({ mode, onSelect }) {
  return (
    <header class="top">
      <h1>
        <img class="logo" src="./img2svg.svg" alt="" /> img2svg
      </h1>
      <p>Imágenes a SVG, entero en tu navegador: la imagen no sale de tu equipo.</p>

      <Tabs
        label="Modo de conversión"
        tabs={TABS}
        active={mode}
        onSelect={onSelect}
      />
    </header>
  );
}
