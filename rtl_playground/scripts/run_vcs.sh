#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SIM_DIR="${ROOT_DIR}/sim"
TOP="${TOP:-tb_tensor_unit}"

if [[ -z "${TOP}" ]]; then
  echo "TOP must not be empty" >&2
  exit 2
fi

if [[ ! "${TOP}" =~ ^[A-Za-z_][A-Za-z0-9_$]*$ ]]; then
  echo "Unsupported TOP '${TOP}'" >&2
  exit 2
fi

usage() {
  echo "Usage: $0 {build|sim|waves}" >&2
}

compile_normal() {
  mkdir -p "${SIM_DIR}"
  cd "${ROOT_DIR}"
  vcs -full64 -sverilog -timescale=1ns/1ps \
    -ntb_opts uvm \
    -debug_access+all -kdb \
    -top "${TOP}" \
    -Mdir="sim/${TOP}.csrc" \
    -l "sim/${TOP}.vcs_compile.log" \
    -f filelist.f \
    -o "sim/${TOP}.simv"
}

wait_for_simv_ready() {
  local simv_path="$1"

  for _ in {1..60}; do
    if [[ -x "${simv_path}" ]] && ! ldd "${simv_path}" 2>&1 | grep -q "not found"; then
      return 0
    fi
    sleep 1
  done

  echo "Simulator '${simv_path}' was not ready after VCS build" >&2
  ldd "${simv_path}" >&2 || true
  return 1
}

compile_fsdb() {
  local fsdb_flags=()
  if [[ -n "${VCS_FSDB_FLAGS:-}" ]]; then
    read -r -a fsdb_flags <<< "${VCS_FSDB_FLAGS}"
  fi

  mkdir -p "${SIM_DIR}"
  cd "${ROOT_DIR}"
  vcs -full64 -sverilog -timescale=1ns/1ps \
    -ntb_opts uvm \
    -debug_access+all -kdb \
    +define+FSDB_DUMP \
    "${fsdb_flags[@]}" \
    -top "${TOP}" \
    -Mdir="sim/${TOP}.csrc_fsdb" \
    -l "sim/${TOP}.vcs_compile_fsdb.log" \
    -f filelist.f \
    -o "sim/${TOP}.simv_fsdb"
}

run_normal() {
  cd "${ROOT_DIR}"
  wait_for_simv_ready "sim/${TOP}.simv"
  "./sim/${TOP}.simv" -l "sim/${TOP}.vcs_sim.log"
}

run_fsdb() {
  cd "${ROOT_DIR}"
  wait_for_simv_ready "sim/${TOP}.simv_fsdb"
  "./sim/${TOP}.simv_fsdb" -l "sim/${TOP}.vcs_sim_fsdb.log" "+FSDB_FILE=sim/${TOP}.fsdb"
}

MODE="${1:-sim}"

case "${MODE}" in
  build)
    compile_normal
    ;;
  sim)
    compile_normal
    run_normal
    ;;
  waves)
    compile_fsdb
    run_fsdb
    ;;
  *)
    usage
    exit 2
    ;;
esac
