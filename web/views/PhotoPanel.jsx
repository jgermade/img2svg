import { Field, Row } from "../components/Field.jsx";
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
import { FIT_OPTIONS, PHOTO_FIT_TOLERANCE, fitPatch } from "./modes.jsx";

export function PhotoPanel({ hidden, values: v, onChange, actions }) {
  const set = (key) => (value, opts) => onChange({ [key]: value }, opts);

  return (
    <aside
      class="controls"
      id="panel-photo"
      role="tabpanel"
      aria-labelledby="tab-photo"
      hidden={hidden}
    >
      <Range
        label="Tolerancia de color"
        value={v.tolerance}
        min="0.01"
        max="0.2"
        step="0.005"
        hint="Distancia máxima entre un color y el de la región que lo pinta, en una escala perceptual donde de negro a blanco hay 1."
        onChange={set("tolerance")}
      />

      <Range
        label="Regularizar la paleta"
        suffix="pasadas"
        value={v.smoothing}
        min="0"
        max="6"
        step="1"
        hint="La paleta decide color a color, sin mirar alrededor, así que el grano de una foto rompe en motas lo que se ve liso. Esto lo deshace pesando el parecido de color contra el acuerdo con los vecinos: un píxel de grano se funde y un trazo fino no. Cada pasada raspa una corona; a 0, sin regularizar."
        onChange={set("smoothing")}
      />

      <Range
        label="Escalón de degradado"
        value={v.gradientStep}
        min="0"
        max="0.2"
        step="0.005"
        hint="Ensancha las bandas de un cielo fundiendo tonos que sólo se distinguen en luz. En un dibujo con volumen aplana el sombreado."
        onChange={set("gradientStep")}
      />

      <Select
        label="Contorno"
        value={v.fit}
        options={FIT_OPTIONS}
        onChange={(fit, opts) => onChange(fitPatch(fit, PHOTO_FIT_TOLERANCE), opts)}
        hint="El polígono junta en un tramo recto los escalones que no dibujan nada: el mismo dibujo con bastante menos SVG. Las curvas no comprimen —salen algo más grandes—, pero el contorno sigue siendo liso por mucho que se amplíe."
      />

      <Range
        label="Desviación máxima"
        suffix="px"
        value={v.fitTolerance}
        min="0.25"
        max="3"
        step="0.05"
        hidden={!(v.fit in PHOTO_FIT_TOLERANCE)}
        hint="Cuánto puede apartarse la línea del contorno original. A 0.71 una escalera de 45º colapsa en su diagonal, y con ella el borde de cualquier curva pequeña —una lente de gafas sale octogonal—, por eso arranca justo por debajo. En una imagen grande, donde los rasgos miden cientos de píxeles, subirla a 0.75 comprime bastante y no se pierde nada."
        onChange={set("fitTolerance")}
      />

      <Toggle
        label="Borde subpíxel"
        note="fuera de la retícula"
        checked={v.subpixel}
        onChange={set("subpixel")}
        hint="El contorno sale de recorrer grietas entre píxeles, así que sus vértices caen en la retícula entera: una lente de gafas de dieciséis píxeles no puede ser redonda. El color de los píxeles del borde dice por dónde corta de verdad, y con eso se recolocan. Cuesta bytes —fuera de la retícula un tramo recto necesita dos números con decimales en vez de uno— y lo que compra es sitio, así que en una imagen grande, donde los rasgos ya miden cientos de píxeles, no hay nada que ganar."
      />

      <Toggle
        label="Quitar el fondo"
        note="y recortar al dibujo"
        checked={v.removeBackground}
        onChange={set("removeBackground")}
        hint="Vacía lo que toca el borde de la imagen. El mismo color encerrado dentro se conserva."
      />

      <Advanced>
        <Range
          label="Mínimo para tener color propio"
          suffix="%"
          value={v.minColorShare}
          min="0"
          max="1"
          step="0.05"
          hint="Lo que un color tiene que valer para llevarse una entrada de la paleta. La agrupación va por frecuencia, pero la frecuencia sólo ordena y nunca frena, así que el ringing de un JPEG alrededor de un trazo negro deja una entrada por escalón. No se mide por recuento —que no distingue el ringing de un lunar del mismo tamaño— sino por el error que la entrada ahorra, así que lo que está lejos de todo conserva el suyo por poco que pinte."
          onChange={set("minColorShare")}
        />

        <Range
          label="Filtro de motas"
          value={v.filterSpeckle}
          min="0"
          max="32"
          step="1"
          hint="Área hasta la que una región se funde con su vecina. Sin esto una foto deja decenas de miles de paths de cuatro píxeles."
          onChange={set("filterSpeckle")}
        />

        <Range
          label="Grosor mínimo"
          value={v.minThickness}
          min="0"
          max="3"
          step="0.25"
          hint={
            <>
              Quita las bandas de un píxel que bordean cada frontera de color. A
              1 se lleva <b>todo</b> lo que mida un píxel de ancho, incluida una
              línea fina de dibujo: para línea fina, ponlo a 0.
            </>
          }
          onChange={set("minThickness")}
        />

        <Range
          label="Precisión de color"
          value={v.colorPrecision}
          min="2"
          max="8"
          step="1"
          hint="Bits por canal antes de agrupar; baja el ruido del último bit."
          onChange={set("colorPrecision")}
        />

        <Field
          label="Máximo de colores"
          hint="Con tope, los colores que sobran van al más cercano aunque quede lejos. Menos colores no es menos regiones: suele ser más."
        >
          <Row>
            <Check checked={v.capColors} onChange={set("capColors")} />
            <NumberInput
              min="2"
              step="1"
              value={v.maxColors}
              disabled={!v.capColors}
              onChange={set("maxColors")}
            />
          </Row>
        </Field>

        <Range
          label="Umbral de alfa"
          value={v.alpha}
          min="0"
          max="255"
          step="1"
          hint="Por debajo, el píxel se considera transparente."
          onChange={set("alpha")}
        />

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
      </Advanced>

      <Actions {...actions} />
    </aside>
  );
}
