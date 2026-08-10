// Los controles sueltos. Todos son controlados: el valor entra por `value` y
// sale por `onChange`, y ninguno guarda nada.
//
// Quién rebota y quién no es una propiedad del **tipo** de control, no del
// ajuste: un deslizador manda decenas de valores mientras se arrastra y una
// casilla manda uno. Por eso `continuous` viaja con el cambio, y el modo de
// arriba no tiene que acordarse ajuste por ajuste.

import { Field, Row, RowLabel } from "./Field.jsx";

export function Check({ checked, onChange }) {
  return (
    <input
      type="checkbox"
      checked={checked}
      onChange={(e) => onChange(e.currentTarget.checked, { continuous: false })}
    />
  );
}

export function NumberInput({ value, min, step, disabled, onChange }) {
  return (
    <input
      type="number"
      min={min}
      step={step}
      value={value}
      disabled={disabled}
      onInput={(e) => onChange(e.currentTarget.value, { continuous: true })}
    />
  );
}

export function ColorInput({ value, disabled, onChange }) {
  return (
    <input
      type="color"
      value={value}
      disabled={disabled}
      onInput={(e) => onChange(e.currentTarget.value, { continuous: true })}
    />
  );
}

/**
 * Deslizador con su cifra en vivo dentro del título. `suffix` es para la
 * desviación, que se lee «Desviación máxima 0.75 px».
 */
export function Range({
  label,
  value,
  min,
  max,
  step,
  hint,
  hidden,
  suffix,
  onChange,
}) {
  return (
    <Field
      label={
        <>
          {label} <b>{value}</b>
          {suffix ? ` ${suffix}` : null}
        </>
      }
      hint={hint}
      hidden={hidden}
    >
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onInput={(e) =>
          onChange(Number(e.currentTarget.value), { continuous: true })
        }
      />
    </Field>
  );
}

export function Select({ label, value, hint, options, onChange }) {
  return (
    <Field label={label} hint={hint}>
      <select
        value={value}
        onChange={(e) => onChange(e.currentTarget.value, { continuous: false })}
      >
        {options.map(({ value: v, label: text }) => (
          <option key={v} value={v}>
            {text}
          </option>
        ))}
      </select>
    </Field>
  );
}

/** Casilla con una etiqueta al lado, sin nada que habilitar. */
export function Toggle({ label, note, hint, checked, onChange }) {
  return (
    <Field label={label} hint={hint}>
      <Row>
        <Check checked={checked} onChange={onChange} />
        <RowLabel>{note}</RowLabel>
      </Row>
    </Field>
  );
}
