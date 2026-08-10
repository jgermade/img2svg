#!/usr/bin/env bash
#
# Baja a `examples/` las tres imágenes reales sobre las que corre
# `tests/corpus.rs`.
#
# Pesan 9 MB, así que no están versionadas: viven adjuntas a una release propia
# del repositorio, `corpus-v1`. Esa release no es una versión del programa —no
# se toca al publicar una v0.x, ni sale como «latest»—, sólo un sitio estable
# del que bajarlas. Se fija por etiqueta a propósito: el día que el corpus
# cambie habrá que subir una etiqueta nueva y cambiarla aquí, en el mismo commit
# que regenere las instantáneas que dependen de esas imágenes.
#
#   scripts/corpus.sh            baja el corpus si falta o no cuadra
#   scripts/corpus.sh --force    lo vuelve a bajar aunque esté
#
# CORPUS_REPO y CORPUS_URL permiten apuntar a otro sitio (un fork, un fichero
# local) sin tocar el script.

set -euo pipefail

tag="corpus-v1"
repo="${CORPUS_REPO:-jgermade/img2svg}"
url="${CORPUS_URL:-https://github.com/$repo/releases/download/$tag/corpus.tar.gz}"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest="$root/examples"
sums="$root/scripts/corpus.sha256"

# `sha256sum` viene en Linux y `shasum` en macOS, y ninguno de los dos está en
# los dos sitios.
verify() {
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$dest" && sha256sum -c "$sums")
  else
    (cd "$dest" && shasum -a 256 -c "$sums")
  fi
}

if [ "${1:-}" != "--force" ] && verify >/dev/null 2>&1; then
  echo "corpus ya presente en examples/"
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "bajando el corpus de $url"
curl -fsSL --retry 3 -o "$tmp/corpus.tar.gz" "$url"

mkdir -p "$dest"
tar xzf "$tmp/corpus.tar.gz" -C "$dest"

# Se comprueban los PNG extraídos, no el .tar.gz: el archivo se reempaqueta con
# fechas nuevas y su huella cambiaría sin que cambiara ninguna imagen. Las de
# los PNG son las que las instantáneas dan por buenas.
if ! verify; then
  echo "" >&2
  echo "las imágenes bajadas no son las que espera scripts/corpus.sha256." >&2
  echo "Si el corpus ha cambiado a propósito, hay que subir una release nueva," >&2
  echo "cambiar \$tag aquí y regenerar: UPDATE_GOLDEN=1 cargo test" >&2
  exit 1
fi

echo "corpus listo en examples/"
