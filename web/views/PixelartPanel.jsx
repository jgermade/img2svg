import { Field, Row, RowLabel } from "../components/Field.jsx";
import {
  Check,
  ColorInput,
  NumberInput,
  Range,
  Select,
  Toggle,
} from "../components/inputs.jsx";
import { Advanced } from "../components/Advanced.jsx";
import { Actions } from "../components/Actions.jsx";
import { FIT_OPTIONS, FIT_TOLERANCE, fitPatch } from "./modes.jsx";

export function PixelartPanel({ hidden, values: v, onChange, actions }) {
  const set = (key) => (value, opts) => onChange({ [key]: value }, opts);

  return (
    <aside
      class="controls"
      id="panel-pixelart"
      role="tabpanel"
      aria-labelledby="tab-pixelart"
      hidden={hidden}
    >
      <Field
        label="Escala de la rejilla"
        hint="Píxeles reales que ocupa cada píxel del dibujo."
      >
        <Row>
          <Check checked={v.autoScale} onChange={set("autoScale")} />
          <RowLabel>
            {v.autoScale && v.scale !== ""
              ? `automática (${v.scale} px)`
              : "automática"}
          </RowLabel>
        </Row>
        <NumberInput
          min="1"
          step="0.01"
          value={v.scale}
          disabled={v.autoScale}
          onChange={set("scale")}
        />
      </Field>

      <Range
        label="Tolerancia de color"
        value={v.tolerance}
        min="0"
        max="48"
        step="1"
        hint="Funde los tonos casi idénticos del ruido de compresión."
        onChange={set("tolerance")}
      />

      <Toggle
        label="Quitar cuadrícula de transparencia"
        note="damero blanco/gris"
        checked={v.removeChecker}
        onChange={set("removeChecker")}
        hint="Devuelve a transparente el damero que se queda pegado al capturar la pantalla de un editor."
      />

      <Advanced>
        <Range
          label="Umbral de alfa"
          value={v.alpha}
          min="0"
          max="255"
          step="1"
          hint="Por debajo, el píxel se considera transparente."
          onChange={set("alpha")}
        />

        <Field label="Tamaño de píxel" hint="Unidades SVG por píxel del dibujo.">
          <Row>
            <Check checked={v.autoPixel} onChange={set("autoPixel")} />
            <RowLabel>tamaño original</RowLabel>
          </Row>
          <NumberInput
            min="1"
            step="1"
            value={v.pixelSize}
            disabled={v.autoPixel}
            onChange={set("pixelSize")}
          />
        </Field>

        <Field
          label="Fondo"
          hint="Sin marcar, el SVG queda con fondo transparente."
        >
          <Row>
            <Check checked={v.useBackground} onChange={set("useBackground")} />
            <ColorInput
              value={v.background}
              disabled={!v.useBackground}
              onChange={set("background")}
            />
          </Row>
        </Field>

        <Toggle
          label="Quitar el fondo"
          note="y recortar al dibujo"
          checked={v.removeBackground}
          onChange={set("removeBackground")}
          hint="Vacía el color liso que rodea al dibujo y ajusta el lienzo a lo que queda. El mismo color encerrado dentro se conserva."
        />

        <Toggle
          label="Un path por color"
          note="en vez de por bloque"
          checked={v.mergeColors}
          onChange={set("mergeColors")}
          hint="Ocupa menos, pero cada figura del SVG pasa a ser todo lo que comparte color, esté donde esté."
        />

        <Select
          label="Contorno"
          value={v.fit}
          options={FIT_OPTIONS}
          onChange={(fit, opts) => onChange(fitPatch(fit), opts)}
          hint={
            <>
              En pixel art la escalera <em>es</em> el dibujo, así que lo normal
              es dejarla. El polígono endereza las diagonales, que aquí son
              escalones a propósito, y las curvas redondean el sprite entero.
            </>
          }
        />

        <Range
          label="Desviación máxima"
          suffix="px"
          value={v.fitTolerance}
          min="0.25"
          max="3"
          step="0.05"
          hidden={!(v.fit in FIT_TOLERANCE)}
          hint="Cuánto puede apartarse la línea del contorno original."
          onChange={set("fitTolerance")}
        />
      </Advanced>

      <Actions {...actions} />
    </aside>
  );
}
