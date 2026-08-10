// `tablist` de verdad: flechas entre pestañas, que es lo que se espera de uno,
// y el foco viaja con la selección.

export function Tabs({ label, tabs, active, onSelect }) {
  function onKeyDown(e, i) {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = tabs[(i + step + tabs.length) % tabs.length];
    // El botón siguiente ya está en el DOM: se busca por su id, que es el mismo
    // que anuncia `aria-controls` del panel.
    document.getElementById(`tab-${next.id}`)?.focus();
    onSelect(next.id);
  }

  return (
    <nav class="tabs" role="tablist" aria-label={label}>
      {tabs.map((tab, i) => (
        <button
          key={tab.id}
          id={`tab-${tab.id}`}
          class={tab.id === active ? "tab on" : "tab"}
          role="tab"
          type="button"
          aria-selected={String(tab.id === active)}
          aria-controls={`panel-${tab.id}`}
          onClick={() => onSelect(tab.id)}
          onKeyDown={(e) => onKeyDown(e, i)}
        >
          <span class="tab-name">{tab.name}</span>
          <span class="tab-hint">{tab.hint}</span>
        </button>
      ))}
    </nav>
  );
}
