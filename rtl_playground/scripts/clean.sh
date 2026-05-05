#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SIM_DIR="${ROOT_DIR}/sim"

mkdir -p "${SIM_DIR}"
find "${SIM_DIR}" -mindepth 1 ! -name .gitkeep -exec rm -rf {} +
rm -rf "${ROOT_DIR}/ucli.key" \
       "${ROOT_DIR}/DVEfiles" \
       "${ROOT_DIR}/novas.conf" \
       "${ROOT_DIR}/novas.rc" \
       "${ROOT_DIR}/novas_dump.log" \
       "${ROOT_DIR}/verdiLog"
