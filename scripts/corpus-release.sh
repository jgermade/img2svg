#!/usr/bin/env bash
#
# Publica el corpus como release propia del repositorio, que es de donde lo baja
# `scripts/corpus.sh`.
#
#   scripts/corpus-release.sh corpus-v2
#
# Empaqueta él mismo las imágenes que nombra `scripts/corpus.sha256`. Esa lista
# es la que manda: es la que verifica el script de bajada y la que esperan los
# tests, así que preguntar qué fichero subir sólo abriría la puerta a publicar
# otra cosa. Y las verifica antes de empaquetar, porque un adjunto que no case
# con las huellas del repositorio deja a todo el mundo con un `corpus.sh` que
# baja y rechaza.
#
# Se hace a mano y muy de vez en cuando —sólo cuando cambian las imágenes—, pero
# la parte de la API tiene detalles que no se recuerdan de un año para otro: los
# adjuntos van a otro host y por id de release, no por etiqueta, y `make_latest`
# es la cadena "false" y no un booleano.
#
# El token sale de GITHUB_TOKEN o GH_TOKEN, y si no están se pide por teclado
# para que no acabe en el historial ni en la lista de procesos. Basta un PAT
# fine-grained con Contents: Read and write sobre este repositorio; una deploy
# key no sirve, es SSH y esto es la API.

set -euo pipefail

tag="${1:-}"
repo="${CORPUS_REPO:-jgermade/img2svg}"

if [ -z "$tag" ]; then
  echo "uso: scripts/corpus-release.sh <etiqueta>" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest="$root/examples"
sums="$root/scripts/corpus.sha256"

# --- Las imágenes son las que dice el repositorio -----------------------------

# La segunda columna de cada línea de huellas, que es el nombre del fichero.
files=()
while IFS= read -r line; do
  case "$line" in '' | '#'*) continue ;; esac
  files+=("${line#*  }")
done < "$sums"

[ ${#files[@]} -gt 0 ] || { echo "$sums no nombra ninguna imagen" >&2; exit 1; }

for file in "${files[@]}"; do
  [ -f "$dest/$file" ] || { echo "falta examples/$file" >&2; exit 1; }
done

# `sha256sum` viene en Linux y `shasum` en macOS, y ninguno de los dos está en
# los dos sitios.
verify() {
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$dest" && sha256sum -c "$sums")
  else
    (cd "$dest" && shasum -a 256 -c "$sums")
  fi
}

if ! verify; then
  echo "" >&2
  echo "las imágenes de examples/ no son las de scripts/corpus.sha256." >&2
  echo "Si el corpus ha cambiado a propósito, primero las huellas nuevas y las" >&2
  echo "instantáneas regeneradas, y luego se publica:" >&2
  echo "  (cd examples && shasum -a 256 ${files[*]}) > scripts/corpus.sha256" >&2
  echo "  UPDATE_GOLDEN=1 cargo test" >&2
  exit 1
fi

# --- Empaquetar ---------------------------------------------------------------

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
archive="$tmp/corpus.tar.gz"

# `COPYFILE_DISABLE` es de macOS: sin él `tar` mete un `._fichero` con los
# atributos extendidos junto a cada imagen, y el corpus llegaría con el doble de
# ficheros de los que nombran las huellas.
COPYFILE_DISABLE=1 tar czf "$archive" -C "$dest" "${files[@]}"
echo "empaquetadas ${#files[@]} imágenes ($(du -h "$archive" | cut -f1))"

# --- El token -----------------------------------------------------------------

# Después de lo local, para no pedir un secreto y fallar luego por un fichero.
token="${GITHUB_TOKEN:-${GH_TOKEN:-}}"
if [ -z "$token" ]; then
  read -rsp "token de GitHub (Contents: write en $repo): " token
  echo
fi
[ -n "$token" ] || { echo "hace falta un token" >&2; exit 1; }

api=(
  -H "Accept: application/vnd.github+json"
  -H "X-GitHub-Api-Version: 2022-11-28"
)

# --- La etiqueta está libre ---------------------------------------------------

# Preguntando antes se distingue «ya publicaste esto» de un 422 genérico, y de
# paso el token se estrena en una llamada que no crea nada.
code=$(curl -sS -o /dev/null -w '%{http_code}' \
  -H "Authorization: Bearer $token" "${api[@]}" \
  "https://api.github.com/repos/$repo/releases/tags/$tag")
case "$code" in
  404) ;;
  200)
    echo "la release $tag ya existe en $repo: usa una etiqueta nueva" >&2
    exit 1
    ;;
  401 | 403)
    echo "la API responde $code: el token no vale para $repo" >&2
    exit 1
    ;;
  *)
    echo "la API responde $code al preguntar por $tag" >&2
    exit 1
    ;;
esac

# --- Crear la release ---------------------------------------------------------

# `make_latest: false` la deja fuera del «Latest release»: esa insignia es de la
# última versión del programa, y un corpus no es una versión del programa.
response=$(curl -sS --fail-with-body -X POST \
  -H "Authorization: Bearer $token" "${api[@]}" \
  "https://api.github.com/repos/$repo/releases" \
  -d "$(cat <<JSON
{
  "tag_name": "$tag",
  "target_commitish": "main",
  "name": "Corpus de pruebas ($tag)",
  "body": "Las imágenes reales sobre las que corre \`tests/corpus.rs\`. No es una versión del programa: es el sitio del que \`scripts/corpus.sh\` las baja. Huellas en \`scripts/corpus.sha256\`.",
  "make_latest": "false"
}
JSON
)")

# Con `sed` sobre el JSON se acaba subiendo el adjunto a la release equivocada:
# en esa respuesta hay más de un campo llamado "id".
id=$(printf '%s' "$response" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')

# --- Adjuntar el archivo ------------------------------------------------------

# Otro host, y por id: `uploads.github.com` no entiende de etiquetas.
url=$(curl -sS --fail-with-body -X POST \
  -H "Authorization: Bearer $token" "${api[@]}" \
  -H "Content-Type: application/gzip" \
  --data-binary "@$archive" \
  "https://uploads.github.com/repos/$repo/releases/$id/assets?name=corpus.tar.gz" \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["browser_download_url"])')

echo
echo "publicado: $url"
echo "queda cambiar \$tag en scripts/corpus.sh a \"$tag\" si no lo está ya."
