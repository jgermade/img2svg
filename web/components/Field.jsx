// La forma que comparten los 21 ajustes: título, control(es) y la explicación
// de debajo. El texto lo pone quien lo usa; aquí sólo está la caja.

export function Field({ label, hint, hidden, children }) {
  return (
    <label class="field" hidden={hidden}>
      <span>{label}</span>
      {children}
      {hint ? <small>{hint}</small> : null}
    </label>
  );
}

/** La fila donde conviven una casilla y lo que habilita. */
export function Row({ children }) {
  return <div class="row">{children}</div>;
}

export function RowLabel({ children }) {
  return <span class="row-label">{children}</span>;
}
