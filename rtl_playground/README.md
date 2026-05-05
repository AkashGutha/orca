# RTL Playground

This playground contains self-contained SystemVerilog RTL examples and self-checking verification environments. The default `make sim` target remains the tensor unit regression.

## Tensor unit

The tensor example implements a width-generic unsigned tensor primitive:

```text
C = A x B
```

`A` and `B` are row-major matrices of unsigned elements. `C` is a matrix of widened unsigned accumulation results. Each output element is:

```text
C[row][col] = sum(A[row][k] * B[k][col]) for k = 0..MAT_DIM-1
```

The primary default configuration is `DATA_WIDTH=16`, `MAT_DIM=2`, `ENABLE_CACHE=1`, and `CACHE_DEPTH=4`. For all-maximum 16-bit 2x2 inputs, each output element is `65535*65535 + 65535*65535 = 8589672450`, so the default accumulator width is 33 bits.

### Tensor interface

The primary RTL module is `rtl/tensor_unit.sv`:

```systemverilog
module tensor_unit #(
  parameter int DATA_WIDTH   = 16,
  parameter int MAT_DIM      = 2,
  parameter int ACC_WIDTH    = (2 * DATA_WIDTH) + $clog2(MAT_DIM),
  parameter bit ENABLE_CACHE = 1'b1,
  parameter int CACHE_DEPTH  = 4
) (
  input  logic clk,
  input  logic rst_n,
  input  logic dft_mode,
  input  logic scan_enable,
  input  logic scan_in,

  input  logic in_valid,
  output logic in_ready,

  input  logic [MAT_DIM*MAT_DIM*DATA_WIDTH-1:0] a_data,
  input  logic [MAT_DIM*MAT_DIM*DATA_WIDTH-1:0] b_data,

  output logic out_valid,
  input  logic out_ready,

  output logic [MAT_DIM*MAT_DIM*ACC_WIDTH-1:0] c_data,
  output logic cache_hit,
  output logic cache_miss,
  output logic scan_out
);
```

The unit uses a single-entry ready/valid pipeline. Input is accepted when `in_valid && in_ready`. Output is consumed when `out_valid && out_ready`. If `out_valid && !out_ready`, `out_valid`, `c_data`, `cache_hit`, and `cache_miss` are held stable. If `out_valid && out_ready && in_valid`, the old output is consumed and the new input is accepted in the same cycle.

Reset is synchronous active-low. During reset, `out_valid`, `c_data`, cache status, cache valid bits, and the replacement pointer are cleared.

### DFT behavioral scan

`tensor_unit` and the legacy `tensor_unit_8bit` wrapper expose `dft_mode`, `scan_enable`, `scan_in`, and `scan_out` for RTL-level behavioral scan readiness. This is only an RTL scan model for controllability/observability; physical scan-cell replacement, stitching, compression, ATPG, and timing constraints remain implementation-flow responsibilities. Functional-only instantiations must tie `dft_mode`, `scan_enable`, and `scan_in` low.

Reset is synchronous active-low and has highest sequential priority. With `dft_mode=0`, `scan_enable` and `scan_in` are ignored and `scan_out` is driven low, so functional ready/valid timing, outputs, and cache behavior are unchanged by noisy scan pins. With `dft_mode=1 && scan_enable=0`, all scanned state is held and functional input acceptance is suppressed. With `dft_mode=1 && scan_enable=1`, the scan chain destructively shifts; reset is required before supported functional reuse.

`scan_out` is `0` in reset and functional mode. In DFT hold mode it reflects the current scan-chain tail bit. In DFT shift mode it reflects the pre-shift tail bit for that clock.

The generic module scans every sequential state element, LSB first, all reset to zero:

```text
out_valid                                  1 bit
c_data                             C_FLAT_BITS bits
cache_hit                                  1 bit
cache_miss                                 1 bit
cache_valid_q                    CACHE_SLOTS bits
replace_ptr_q             CACHE_INDEX_WIDTH bits
cache_key_q[0..CACHE_SLOTS-1] CACHE_SLOTS * CACHE_KEY_WIDTH bits
cache_data_q[0..CACHE_SLOTS-1] CACHE_SLOTS * C_FLAT_BITS bits
```

`SCAN_STATE_WIDTH = 1 + C_FLAT_BITS + 1 + 1 + CACHE_SLOTS + CACHE_INDEX_WIDTH + (CACHE_SLOTS * CACHE_KEY_WIDTH) + (CACHE_SLOTS * C_FLAT_BITS)`. The `tensor_unit_8bit` wrapper has no additional flops; it passes the DFT pins through to `tensor_unit`. The regression covers default cached, legacy 8-bit, cache-disabled, cache-depth-1, cache-depth-2, and `MAT_DIM=1` DFT configurations.

### Matrix packing

Matrices are packed row-major:

```text
index = row*MAT_DIM + col
bits  = matrix[(index*WIDTH) +: WIDTH]
```

Input elements use `DATA_WIDTH` bits. Output elements use `ACC_WIDTH` bits.

Arithmetic is unsigned, widened, and non-saturating. The default output width formula is:

```text
ACC_WIDTH = (2 * DATA_WIDTH) + $clog2(MAT_DIM)
```

### Cache behavior

When `ENABLE_CACHE=1`, accepted inputs are looked up by the exact packed key `{a_data,b_data}`. A hit returns the cached packed `c_data` result and asserts `cache_hit=1, cache_miss=0`. A miss computes the result, asserts `cache_hit=0, cache_miss=1`, and fills one entry using round-robin replacement. Hits do not advance the replacement pointer. Cache status is valid only with `out_valid` and remains stable under output backpressure.

When `ENABLE_CACHE=0`, the unit performs no cache lookup or fill and both status outputs remain `0`.

The legacy source-compatible wrapper remains available as `rtl/tensor_unit_8bit.sv`.

### Tensor verification

The provided self-checking regression is `tb/tb_tensor_unit.sv`, top `tb_tensor_unit`. It instantiates default 16-bit cached, cache-disabled, cache-depth-2, cache-depth-1, `MAT_DIM=1`, and legacy 8-bit fixtures. The regression uses an independent reference matrix multiply model and independent reference cache model.

## JPEG decoded-block filter applier

The JPEG example verifies `rtl/jpeg_filter_applier.sv` with `tb/tb_jpeg_filter_applier.sv`, top `tb_jpeg_filter_applier`. The scope is a decoded 8x8 sample block only; compressed JPEG byte streams, entropy decoding, DCT/IDCT, quantization, and color-component handling are outside this playground.

Input blocks are packed row-major:

```text
sample_index = row*8 + col
bits         = block_data[(sample_index*SAMPLE_WIDTH) +: SAMPLE_WIDTH]
```

Outputs are streamed row-major over `LANES` parallel lanes:

```text
global_index = beat_index*LANES + lane
bits         = sample_data[(lane*SAMPLE_WIDTH) +: SAMPLE_WIDTH]
```

`LANES` is legal from 1 through 64. The output beat count is `ceil(64/LANES)`, so `LANES=1` emits 64 beats, `LANES=5` emits 13 beats, `LANES=8` emits 8 beats, and `LANES=64` emits 1 beat. `sample_keep[lane]` marks valid lanes; for `LANES=5`, the final beat has `sample_keep == 5'b01111` and invalid tail sample data is checked as zero. `out_last` is only meaningful with `out_valid` and must be consumed on the final `out_valid && out_ready` beat. Output data, keep, and last are asserted stable during `out_valid && !out_ready`.

Implemented filter modes are:

```text
0: identity
1: Gaussian-like blur, 1 2 1 / 2 4 2 / 1 2 1, normalized by >>> 4
2: sharpen, 0 -1 0 / -1 5 -1 / 0 -1 0
3: edge detect, -1 -1 -1 / -1 8 -1 / -1 -1 -1
```

The UVM reference model uses unsigned 8-bit input samples, signed kernel accumulation, clamped-edge boundary sampling, and saturating clamp to the sample range. The JPEG test instantiates four DUT/interface pairs in one run for `LANES=1,5,8,64`, drives directed identity/blur/sharpen/edge scenarios, injects deterministic stalls on first, middle, and final output beats, checks reset recovery, and reports non-vacuous KPI lines from scoreboard counters.

## Running

Synopsys VCS and Verdi must be available on `PATH`.

```sh
cd rtl_playground
make clean
make sim
make sim TOP=tb_tensor_unit
make sim TOP=tb_jpeg_filter_applier
make waves TOP=tb_jpeg_filter_applier
make verdi TOP=tb_jpeg_filter_applier
```

Expected tensor simulation pass message:

```text
PASS: all tensor_unit tests passed
```

Expected JPEG simulation pass messages include:

```text
JPEG_FILTER_KPI LANES=1 EXPECTED_BEATS=64 OBSERVED_BEATS=64 CHECKED_BLOCKS=N PASS
JPEG_FILTER_KPI LANES=5 EXPECTED_BEATS=13 OBSERVED_BEATS=13 CHECKED_BLOCKS=N PASS
JPEG_FILTER_KPI LANES=8 EXPECTED_BEATS=8 OBSERVED_BEATS=8 CHECKED_BLOCKS=N PASS
JPEG_FILTER_KPI LANES=64 EXPECTED_BEATS=1 OBSERVED_BEATS=1 CHECKED_BLOCKS=N PASS
JPEG_FILTER_APPLIER_TEST_PASS
```

Generated build, simulation, log, and waveform artifacts are written under `rtl_playground/sim/` where possible.

Common generated files include:

```text
sim/<TOP>.simv
sim/<TOP>.simv_fsdb
sim/<TOP>.csrc/
sim/<TOP>.csrc_fsdb/
sim/<TOP>.vcs_compile.log
sim/<TOP>.vcs_sim.log
sim/<TOP>.vcs_compile_fsdb.log
sim/<TOP>.vcs_sim_fsdb.log
sim/<TOP>.fsdb
```

Remove generated artifacts while preserving `sim/.gitkeep` with:

```sh
make clean
```

FSDB dumping is compile-time guarded with `FSDB_DUMP`. Some sites require additional Novas/Verdi PLI flags for FSDB system tasks; pass them with `VCS_FSDB_FLAGS`, for example:

```sh
VCS_FSDB_FLAGS="-P /path/to/novas.tab /path/to/pli.a" make waves
```
