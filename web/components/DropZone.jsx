import { useRef, useState } from "preact/hooks";

// Arrastrar sobre la zona la resalta. Soltar en cualquier parte del documento
// también carga: eso lo monta la vista, porque vale igual con el espacio de
// trabajo abierto y esta zona escondida.

export function DropZone({ onFile, hidden }) {
  const input = useRef(null);
  const [dragging, setDragging] = useState(false);

  const open = () => input.current?.click();

  return (
    <section
      id="drop"
      class={dragging ? "drop dragging" : "drop"}
      tabindex="0"
      aria-label="Cargar imagen"
      hidden={hidden}
      onClick={open}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          open();
        }
      }}
      onDragEnter={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragOver={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={() => setDragging(false)}
    >
      <svg class="drop-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 16V4m0 0L8 8m4-4 4 4M4 16v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" />
      </svg>
      <p class="drop-title">
        Arrastra una imagen, pega con <kbd>Ctrl</kbd>+<kbd>V</kbd> o haz clic
      </p>
      <p class="drop-hint">
        PNG, JPEG, GIF, WebP, AVIF… lo que decodifique el navegador
      </p>
      <input
        ref={input}
        id="file"
        type="file"
        accept="image/*"
        hidden
        onChange={(e) => {
          const file = e.currentTarget.files[0];
          if (file) onFile(file);
          e.currentTarget.value = "";
        }}
      />
    </section>
  );
}
