import { useState } from "preact/hooks";

// Los dos paneles tienen sus propios botones —cada `<aside>` es el suyo— y
// hacen exactamente lo mismo sobre el último SVG generado. El «¡Copiado!» es de
// cada botón, así que el aviso vive aquí y no en el estado de la página.

export function Actions({ onDownload, onCopy, onReset }) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    if (!(await onCopy())) return;
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div class="actions">
      <button type="button" class="primary" onClick={onDownload}>
        Descargar SVG
      </button>
      <button type="button" onClick={copy}>
        {copied ? "¡Copiado!" : "Copiar"}
      </button>
      <button type="button" onClick={onReset}>
        Otra imagen
      </button>
    </div>
  );
}
