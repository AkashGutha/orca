`ifndef RTL_PLAYGROUND_TENSOR_UNIT_SV
`define RTL_PLAYGROUND_TENSOR_UNIT_SV

module tensor_unit #(
  parameter int DATA_WIDTH   = 16,
  parameter int MAT_DIM      = 2,
  parameter int ACC_WIDTH    = (2 * DATA_WIDTH) + ((MAT_DIM <= 1) ? 0 : $clog2(MAT_DIM)),
  parameter bit ENABLE_CACHE = 1'b1,
  parameter int CACHE_DEPTH  = 4
) (
  input  logic clk,
  input  logic rst_n,
  // RTL behavioral scan control. This is scan-readiness modeling only; physical
  // scan insertion/stitching is still owned by the implementation DFT flow.
  // Functional-only instantiations must tie dft_mode, scan_enable, and scan_in low.
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

  // Single-cycle ready/valid tensor matmul. Inputs and accumulation are unsigned.
  // Outputs are held stable while out_valid is asserted and out_ready is low.
  // The optional cache is a local exact-match result memoization cache keyed by
  // the full packed operand pair {a_data, b_data}; reset is the only invalidation.
  // Cache hits and misses have the same registered external latency.
  //
  // DFT contract:
  // - rst_n low has priority over DFT hold/shift and clears all sequential state.
  // - dft_mode low is functional mode; scan_enable and scan_in are ignored.
  // - dft_mode high with scan_enable low holds all sequential state and suppresses
  //   functional accepts.
  // - dft_mode high with scan_enable high destructively shifts the scan chain.
  //   Reset is required before returning to supported functional use.
  localparam int MAT_ELEMS         = MAT_DIM * MAT_DIM;
  localparam int A_FLAT_BITS       = MAT_ELEMS * DATA_WIDTH;
  localparam int C_FLAT_BITS       = MAT_ELEMS * ACC_WIDTH;
  localparam int CACHE_KEY_WIDTH   = 2 * A_FLAT_BITS;
  localparam int CACHE_SLOTS       = (CACHE_DEPTH <= 0) ? 1 : CACHE_DEPTH;
  localparam int CACHE_INDEX_WIDTH = (CACHE_SLOTS <= 1) ? 1 : $clog2(CACHE_SLOTS);
  localparam int MAT_DIM_ACC_BITS  = (MAT_DIM <= 1) ? 0 : $clog2(MAT_DIM);
  localparam int MIN_ACC_WIDTH     = (2 * DATA_WIDTH) + MAT_DIM_ACC_BITS;
  localparam bit CACHE_ACTIVE      = ENABLE_CACHE && (CACHE_DEPTH > 0);
  localparam int SCAN_OUT_VALID_LSB    = 0;
  localparam int SCAN_C_DATA_LSB       = SCAN_OUT_VALID_LSB + 1;
  localparam int SCAN_CACHE_HIT_LSB    = SCAN_C_DATA_LSB + C_FLAT_BITS;
  localparam int SCAN_CACHE_MISS_LSB   = SCAN_CACHE_HIT_LSB + 1;
  localparam int SCAN_CACHE_VALID_LSB  = SCAN_CACHE_MISS_LSB + 1;
  localparam int SCAN_REPLACE_PTR_LSB  = SCAN_CACHE_VALID_LSB + CACHE_SLOTS;
  localparam int SCAN_CACHE_KEY_LSB    = SCAN_REPLACE_PTR_LSB + CACHE_INDEX_WIDTH;
  localparam int SCAN_CACHE_DATA_LSB   = SCAN_CACHE_KEY_LSB + (CACHE_SLOTS * CACHE_KEY_WIDTH);
  localparam int SCAN_STATE_WIDTH      = SCAN_CACHE_DATA_LSB + (CACHE_SLOTS * C_FLAT_BITS);

  logic [CACHE_KEY_WIDTH-1:0] cache_key_q  [CACHE_SLOTS];
  logic [C_FLAT_BITS-1:0]     cache_data_q [CACHE_SLOTS];
  logic [CACHE_SLOTS-1:0]     cache_valid_q;
  logic [CACHE_INDEX_WIDTH-1:0] replace_ptr_q;

  logic [CACHE_KEY_WIDTH-1:0] lookup_key;
  logic [C_FLAT_BITS-1:0]     lookup_data;
  logic [C_FLAT_BITS-1:0]     matmul_data;
  logic                       lookup_hit;
  logic [SCAN_STATE_WIDTH-1:0] scan_state;
  logic [SCAN_STATE_WIDTH-1:0] scan_state_shifted;

  // One-deep registered output stage: accept a new input when the output stage
  // is empty or when its current transaction is consumed in the same cycle.
  // DFT mode owns all sequential state, so functional accepts are suppressed.
  assign in_ready    = dft_mode ? 1'b0 : (!out_valid || out_ready);
  assign lookup_key  = {a_data, b_data};
  assign scan_out    = (rst_n && dft_mode) ? scan_state[0] : 1'b0;

  // synthesis translate_off
  initial begin
    if (DATA_WIDTH <= 0) begin
      $fatal(1, "DATA_WIDTH must be > 0");
    end
    if (MAT_DIM <= 0) begin
      $fatal(1, "MAT_DIM must be > 0");
    end
    if (ENABLE_CACHE && (CACHE_DEPTH <= 0)) begin
      $fatal(1, "CACHE_DEPTH must be > 0 when ENABLE_CACHE is set");
    end
    if (ACC_WIDTH < MIN_ACC_WIDTH) begin
      $fatal(1, "ACC_WIDTH must be at least (2 * DATA_WIDTH) + $clog2(MAT_DIM)");
    end
  end
  // synthesis translate_on

  function automatic logic [C_FLAT_BITS-1:0] calc_matmul(
    input logic [A_FLAT_BITS-1:0] a_flat,
    input logic [A_FLAT_BITS-1:0] b_flat
  );
    logic [C_FLAT_BITS-1:0] result;
    logic [ACC_WIDTH-1:0] acc;
    logic [ACC_WIDTH-1:0] a_ext;
    logic [ACC_WIDTH-1:0] b_ext;
    logic [ACC_WIDTH-1:0] product;
    int row;
    int col;
    int k;
    int a_index;
    int b_index;
    int c_index;

    result = '0;
    for (row = 0; row < MAT_DIM; row++) begin
      for (col = 0; col < MAT_DIM; col++) begin
        acc = '0;
        for (k = 0; k < MAT_DIM; k++) begin
          a_index = (row * MAT_DIM) + k;
          b_index = (k * MAT_DIM) + col;
          a_ext = {{(ACC_WIDTH-DATA_WIDTH){1'b0}}, a_flat[(a_index*DATA_WIDTH) +: DATA_WIDTH]};
          b_ext = {{(ACC_WIDTH-DATA_WIDTH){1'b0}}, b_flat[(b_index*DATA_WIDTH) +: DATA_WIDTH]};
          product = a_ext * b_ext;
          acc = acc + product;
        end
        c_index = (row * MAT_DIM) + col;
        result[(c_index*ACC_WIDTH) +: ACC_WIDTH] = acc;
      end
    end

    return result;
  endfunction

  always_comb begin
    lookup_hit  = 1'b0;
    lookup_data = '0;

    if (CACHE_ACTIVE) begin
      for (int entry = 0; entry < CACHE_SLOTS; entry++) begin
        if (cache_valid_q[entry] && (cache_key_q[entry] == lookup_key) && !lookup_hit) begin
          lookup_hit  = 1'b1;
          lookup_data = cache_data_q[entry];
        end
      end
    end
  end

  always_comb begin
    matmul_data = '0;
    if (!CACHE_ACTIVE || !lookup_hit) begin
      matmul_data = calc_matmul(a_data, b_data);
    end
  end

  // RTL-level behavioral scan chain for all sequential state.
  //
  // Scan inventory, ordered low-to-high and shifted out LSB first. All scanned
  // state resets to zero:
  // - out_valid:                                  1 bit
  // - c_data:                             C_FLAT_BITS bits
  // - cache_hit:                                  1 bit
  // - cache_miss:                                 1 bit
  // - cache_valid_q:                    CACHE_SLOTS bits
  // - replace_ptr_q:             CACHE_INDEX_WIDTH bits
  // - cache_key_q[0..CACHE_SLOTS-1]: CACHE_SLOTS * CACHE_KEY_WIDTH bits
  // - cache_data_q[0..CACHE_SLOTS-1]:  CACHE_SLOTS * C_FLAT_BITS bits
  // Total SCAN_STATE_WIDTH =
  //   1 + C_FLAT_BITS + 1 + 1 + CACHE_SLOTS + CACHE_INDEX_WIDTH
  //   + (CACHE_SLOTS * CACHE_KEY_WIDTH) + (CACHE_SLOTS * C_FLAT_BITS).
  // There are no other sequential state elements in this module.
  // In DFT mode, scan_out is the pre-shift bit 0 tail and scan_in enters bit
  // SCAN_STATE_WIDTH-1. In reset or functional mode, scan_out is driven low.
  always_comb begin
    scan_state = '0;
    scan_state[SCAN_OUT_VALID_LSB] = out_valid;
    scan_state[SCAN_C_DATA_LSB +: C_FLAT_BITS] = c_data;
    scan_state[SCAN_CACHE_HIT_LSB] = cache_hit;
    scan_state[SCAN_CACHE_MISS_LSB] = cache_miss;
    scan_state[SCAN_CACHE_VALID_LSB +: CACHE_SLOTS] = cache_valid_q;
    scan_state[SCAN_REPLACE_PTR_LSB +: CACHE_INDEX_WIDTH] = replace_ptr_q;

    for (int entry = 0; entry < CACHE_SLOTS; entry++) begin
      scan_state[(SCAN_CACHE_KEY_LSB + (entry * CACHE_KEY_WIDTH)) +: CACHE_KEY_WIDTH] =
          cache_key_q[entry];
      scan_state[(SCAN_CACHE_DATA_LSB + (entry * C_FLAT_BITS)) +: C_FLAT_BITS] =
          cache_data_q[entry];
    end

    scan_state_shifted = {scan_in, scan_state[SCAN_STATE_WIDTH-1:1]};
  end

  // Active-low synchronous reset clears output state and invalidates the cache.
  // Reset has priority over DFT hold and scan shift. In DFT mode with
  // scan_enable low, all sequential state holds and in_ready remains low.
  always_ff @(posedge clk) begin
    if (!rst_n) begin
      out_valid     <= 1'b0;
      c_data        <= '0;
      cache_hit     <= 1'b0;
      cache_miss    <= 1'b0;
      cache_valid_q <= '0;
      replace_ptr_q <= '0;
      for (int entry = 0; entry < CACHE_SLOTS; entry++) begin
        cache_key_q[entry]  <= '0;
        cache_data_q[entry] <= '0;
      end
    end else if (dft_mode) begin
      if (scan_enable) begin
        out_valid     <= scan_state_shifted[SCAN_OUT_VALID_LSB];
        c_data        <= scan_state_shifted[SCAN_C_DATA_LSB +: C_FLAT_BITS];
        cache_hit     <= scan_state_shifted[SCAN_CACHE_HIT_LSB];
        cache_miss    <= scan_state_shifted[SCAN_CACHE_MISS_LSB];
        cache_valid_q <= scan_state_shifted[SCAN_CACHE_VALID_LSB +: CACHE_SLOTS];
        replace_ptr_q <= scan_state_shifted[SCAN_REPLACE_PTR_LSB +: CACHE_INDEX_WIDTH];
        for (int entry = 0; entry < CACHE_SLOTS; entry++) begin
          cache_key_q[entry] <=
              scan_state_shifted[(SCAN_CACHE_KEY_LSB + (entry * CACHE_KEY_WIDTH)) +: CACHE_KEY_WIDTH];
          cache_data_q[entry] <=
              scan_state_shifted[(SCAN_CACHE_DATA_LSB + (entry * C_FLAT_BITS)) +: C_FLAT_BITS];
        end
      end
    end else if (in_ready) begin
      out_valid <= in_valid;

      if (in_valid) begin
        if (CACHE_ACTIVE && lookup_hit) begin
          c_data     <= lookup_data;
          cache_hit  <= 1'b1;
          cache_miss <= 1'b0;
        end else begin
          c_data     <= matmul_data;
          cache_hit  <= 1'b0;
          cache_miss <= CACHE_ACTIVE;

          if (CACHE_ACTIVE) begin
            cache_valid_q[replace_ptr_q] <= 1'b1;
            cache_key_q[replace_ptr_q]   <= lookup_key;
            cache_data_q[replace_ptr_q]  <= matmul_data;
            if (replace_ptr_q == (CACHE_SLOTS - 1)) begin
              replace_ptr_q <= '0;
            end else begin
              replace_ptr_q <= replace_ptr_q + 1'b1;
            end
          end
        end
      end else begin
        c_data     <= '0;
        cache_hit  <= 1'b0;
        cache_miss <= 1'b0;
      end
    end
  end

endmodule

`endif
