import { useEffect, useRef } from "preact/hooks";
import { CanvasBox, Figure } from "../components/CanvasBox.jsx";
import { Progress } from "../components/Progress.jsx";
import * as converter from "../services/converter.js";
import { percent, size } from "../services/format.js";
import { MODES } from "./modes.jsx";

export function Preview() {
  const canvas = useRef(null);

  const image = converter.image.value;
  const source = converter.source.value;
  const svg = converter.svg.value;
  const result = converter.result.value;
  const engine = converter.engine.value;
  const progress = converter.progress.value;
  const pending = converter.pending.value;
  const decoding = converter.decoding.value;
  const ms = converter.elapsed.value;

  useEffect(() => {
    if (!image || !canvas.current) return;
    canvas.current.width = image.width;
    canvas.current.height = image.height;
    canvas.current.getContext("2d").putImageData(image, 0, 0);
  }, [image]);

  // Las cifras se leen con el vocabulario de la segmentación que las produjo,
  // no con el de la pestaña abierta.
  const report = result && engine ? MODES[engine].report(result) : null;

  return (
    <section class="preview">
      <div class="panes">
        <Figure
          caption="Original"
          meta={source ? `${source.width}×${source.height} · ${size(source.bytes)}` : ""}
        >
          <CanvasBox id="originalBox" skeleton={!image}>
            <canvas ref={canvas} hidden={!image} />
          </CanvasBox>
        </Figure>

        <Figure
          caption="SVG"
          meta={report ? `${report.meta} · ${size(svg.length)}` : ""}
        >
          {/*
            El SVG lo acaba de escribir nuestro propio wasm a partir de píxeles
            que no han salido del equipo: es contenido propio, no de terceros.
          */}
          <CanvasBox
            id="resultBox"
            stale={pending && Boolean(svg)}
            skeleton={pending && !svg}
          >
            <div class="result-svg" dangerouslySetInnerHTML={{ __html: svg }} />
          </CanvasBox>
        </Figure>
      </div>

      <Progress
        hidden={!progress}
        at={progress ? progress.at : 0}
        label={progress ? progress.label : ""}
        pulse={Boolean(progress && progress.pulse)}
      />

      <p class="stats">
        {report && source && !decoding
          ? `${report.stats} · ${percent(svg.length, source.bytes)} del original · ${Math.round(ms)} ms`
          : ""}
      </p>
    </section>
  );
}
