// Las dos cajas de la vista previa: el damero de fondo, lo que se enseñe
// dentro, y el esqueleto encima mientras no hay nada que enseñar.

export function CanvasBox({ id, stale, skeleton, children }) {
  return (
    <div class={stale ? "canvas-box checker stale" : "canvas-box checker"} id={id}>
      {children}
      <div class="skeleton" hidden={!skeleton} aria-hidden="true" />
    </div>
  );
}

export function Figure({ caption, meta, children }) {
  return (
    <figure>
      <figcaption>
        {caption} <span>{meta}</span>
      </figcaption>
      {children}
    </figure>
  );
}
