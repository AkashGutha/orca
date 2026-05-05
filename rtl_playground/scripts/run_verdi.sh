#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
TOP="${TOP:-tb_tensor_unit}"
FSDB_FILE="${ROOT_DIR}/sim/${TOP}.fsdb"

if [[ -z "${TOP}" ]]; then
  echo "TOP must not be empty" >&2
  exit 2
fi

if [[ ! "${TOP}" =~ ^[A-Za-z_][A-Za-z0-9_$]*$ ]]; then
  echo "Unsupported TOP '${TOP}'" >&2
  exit 2
fi

if [[ ! -f "${FSDB_FILE}" ]]; then
  echo "Missing FSDB: ${FSDB_FILE}" >&2
  echo "Run 'make waves TOP=${TOP}' from rtl_playground first." >&2
  exit 1
fi

cd "${ROOT_DIR}"
exec verdi -sv -f filelist.f -ssf "sim/${TOP}.fsdb" -top "${TOP}"
