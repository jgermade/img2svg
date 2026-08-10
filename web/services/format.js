/** Cifras que se enseñan bajo el SVG. Nada aquí toca el DOM. */

export const size = (bytes) =>
  bytes < 1024
    ? `${bytes} B`
    : bytes < 1024 * 1024
      ? `${(bytes / 1024).toFixed(1)} KB`
      : `${(bytes / 1024 / 1024).toFixed(1)} MB`;

export const percent = (a, b) => (b ? `${((a / b) * 100).toFixed(1)}%` : "—");
