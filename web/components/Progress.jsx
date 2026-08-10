export function Progress({ at, label, pulse, hidden }) {
  return (
    <div
      class={pulse ? "progress pulse" : "progress"}
      role="progressbar"
      aria-label="Progreso de la conversión"
      aria-valuenow={String(at)}
      hidden={hidden}
    >
      <div class="progress-track">
        <div class="progress-bar" style={{ width: `${at}%` }} />
      </div>
      <p class="progress-label">{label}</p>
    </div>
  );
}
