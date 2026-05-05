`ifndef RTL_PLAYGROUND_JPEG_FILTER_APPLIER_SV
`define RTL_PLAYGROUND_JPEG_FILTER_APPLIER_SV

module jpeg_filter_applier #(
  parameter int SAMPLE_WIDTH = 8,
  parameter int BLOCK_DIM    = 8,
  parameter int LANES        = 8,
  parameter int COEFF_WIDTH  = 8,
  parameter int ACC_WIDTH    = 24,
  parameter bit SATURATE     = 1'b1
) (
  input  logic clk,
  input  logic rst_n,

  input  logic in_valid,
  output logic in_ready,
  input  logic [BLOCK_DIM*BLOCK_DIM*SAMPLE_WIDTH-1:0] block_data,

  input  logic [1:0] filter_mode,

  output logic out_valid,
  input  logic out_ready,
  output logic [LANES*SAMPLE_WIDTH-1:0] sample_data,
  output logic [LANES-1:0] sample_keep,
  output logic out_last
);

  // Applies spatial filters to decoded 8x8 sample blocks only. Compressed JPEG
  // byte streams, entropy decoding, DCT/IDCT, quantization, and color-component
  // interpretation are outside this module's scope.
  //
  // Input block packing is row-major:
  //   block_data[(sample_index*SAMPLE_WIDTH) +: SAMPLE_WIDTH]
  // where sample_index = row*BLOCK_DIM + col.
  //
  // Output beats are also row-major:
  //   sample_data[(lane*SAMPLE_WIDTH) +: SAMPLE_WIDTH]
  // where global_index = beat_index*LANES + lane. sample_keep[lane] marks
  // lanes whose global_index is inside the block; invalid final lanes drive 0.
  //
  // Reset is active-low and synchronous to clk. The reset source is assumed to
  // be synchronous, or externally synchronized, to clk.
  localparam int BLOCK_SAMPLES        = BLOCK_DIM * BLOCK_DIM;
  localparam int BLOCK_BITS           = BLOCK_SAMPLES * SAMPLE_WIDTH;
  localparam int OUTPUT_BITS          = LANES * SAMPLE_WIDTH;
  localparam int BEAT_COUNT           = (BLOCK_SAMPLES + LANES - 1) / LANES;
  localparam int BEAT_INDEX_WIDTH     = (BEAT_COUNT <= 1) ? 1 : $clog2(BEAT_COUNT);
  localparam int MIN_KERNEL_ACC_WIDTH = SAMPLE_WIDTH + 5;
  localparam int MIN_ACC_WIDTH        = (MIN_KERNEL_ACC_WIDTH > COEFF_WIDTH) ?
                                        MIN_KERNEL_ACC_WIDTH : COEFF_WIDTH;

  localparam logic [1:0] FILTER_IDENTITY = 2'd0;
  localparam logic [1:0] FILTER_BLUR     = 2'd1;
  localparam logic [1:0] FILTER_SHARPEN  = 2'd2;
  localparam logic [1:0] FILTER_EDGE     = 2'd3;

  typedef enum logic [0:0] {
    STATE_IDLE = 1'b0,
    STATE_EMIT = 1'b1
  } state_t;

  state_t state_q;
  state_t state_d;

  logic [BLOCK_BITS-1:0]             block_data_q;
  logic [1:0]                        filter_mode_q;
  logic [BEAT_INDEX_WIDTH-1:0]       beat_index_q;

  logic                              input_fire;
  logic                              output_fire;
  logic                              final_beat;

  assign in_ready    = (state_q == STATE_IDLE);
  assign input_fire  = in_valid && in_ready;
  assign output_fire = out_valid && out_ready;
  assign final_beat  = (beat_index_q == (BEAT_COUNT - 1));

  // synthesis translate_off
  initial begin
    if (SAMPLE_WIDTH <= 0) begin
      $fatal(1, "SAMPLE_WIDTH must be > 0");
    end
    if (BLOCK_DIM != 8) begin
      $fatal(1, "BLOCK_DIM must be 8 for jpeg_filter_applier v1");
    end
    if (LANES <= 0) begin
      $fatal(1, "LANES must be > 0");
    end
    if (LANES > BLOCK_SAMPLES) begin
      $fatal(1, "LANES must be <= BLOCK_DIM*BLOCK_DIM");
    end
    if (COEFF_WIDTH < 5) begin
      $fatal(1, "COEFF_WIDTH must be >= 5 to represent the edge-detect +8 coefficient");
    end
    if (ACC_WIDTH < MIN_ACC_WIDTH) begin
      $fatal(1, "ACC_WIDTH must cover the signed 3x3 accumulation range and coefficient width");
    end
    if (!SATURATE) begin
      $display("jpeg_filter_applier: SATURATE=0 truncates normalized signed results to low SAMPLE_WIDTH bits");
    end
  end
  // synthesis translate_on

  function automatic int clamp_index(
    input int value,
    input int min_value,
    input int max_value
  );
    if (value < min_value) begin
      return min_value;
    end
    if (value > max_value) begin
      return max_value;
    end
    return value;
  endfunction

  function automatic logic [SAMPLE_WIDTH-1:0] get_block_sample(
    input logic [BLOCK_BITS-1:0] block_flat,
    input int                    row,
    input int                    col
  );
    int clamped_row;
    int clamped_col;
    int sample_index;

    clamped_row  = clamp_index(row, 0, BLOCK_DIM - 1);
    clamped_col  = clamp_index(col, 0, BLOCK_DIM - 1);
    sample_index = (clamped_row * BLOCK_DIM) + clamped_col;
    return block_flat[(sample_index*SAMPLE_WIDTH) +: SAMPLE_WIDTH];
  endfunction

  function automatic logic signed [COEFF_WIDTH-1:0] filter_coeff(
    input logic [1:0] mode,
    input int         tap_row,
    input int         tap_col
  );
    logic signed [COEFF_WIDTH-1:0] coeff;

    coeff = '0;
    unique case (mode)
      FILTER_BLUR: begin
        // Gaussian-like blur: 1 2 1 / 2 4 2 / 1 2 1, normalized by >>> 4.
        if ((tap_row == 1) && (tap_col == 1)) begin
          coeff = 4;
        end else if ((tap_row == 1) || (tap_col == 1)) begin
          coeff = 2;
        end else begin
          coeff = 1;
        end
      end
      FILTER_SHARPEN: begin
        // Sharpen: 0 -1 0 / -1 5 -1 / 0 -1 0.
        if ((tap_row == 1) && (tap_col == 1)) begin
          coeff = 5;
        end else if ((tap_row == 1) || (tap_col == 1)) begin
          coeff = -1;
        end
      end
      FILTER_EDGE: begin
        // Edge detect: -1 around the center, +8 at the center.
        if ((tap_row == 1) && (tap_col == 1)) begin
          coeff = 8;
        end else begin
          coeff = -1;
        end
      end
      default: begin
        coeff = '0;
      end
    endcase

    return coeff;
  endfunction

  function automatic logic [SAMPLE_WIDTH-1:0] clamp_to_sample(
    input logic signed [ACC_WIDTH-1:0] value
  );
    logic signed [ACC_WIDTH-1:0] sample_max_ext;

    // With SATURATE disabled, the normalized signed value wraps by truncating
    // to its low SAMPLE_WIDTH bits.
    sample_max_ext = $signed({{(ACC_WIDTH-SAMPLE_WIDTH){1'b0}}, {SAMPLE_WIDTH{1'b1}}});
    if (SATURATE) begin
      if (value < '0) begin
        return '0;
      end
      if (value > sample_max_ext) begin
        return {SAMPLE_WIDTH{1'b1}};
      end
    end
    return value[SAMPLE_WIDTH-1:0];
  endfunction

  function automatic logic [SAMPLE_WIDTH-1:0] calc_filtered_sample(
    input logic [BLOCK_BITS-1:0] block_flat,
    input logic [1:0]            mode,
    input int                    sample_index
  );
    logic signed [ACC_WIDTH-1:0] acc;
    logic signed [ACC_WIDTH-1:0] normalized;
    logic signed [ACC_WIDTH-1:0] sample_ext;
    logic signed [ACC_WIDTH-1:0] coeff_ext;
    logic signed [ACC_WIDTH-1:0] product;
    logic signed [COEFF_WIDTH-1:0] coeff;
    logic [SAMPLE_WIDTH-1:0]     sample;
    int                          row;
    int                          col;
    int                          tap_row;
    int                          tap_col;

    if (mode == FILTER_IDENTITY) begin
      return get_block_sample(block_flat, sample_index / BLOCK_DIM, sample_index % BLOCK_DIM);
    end

    row = sample_index / BLOCK_DIM;
    col = sample_index % BLOCK_DIM;
    acc = '0;

    for (tap_row = 0; tap_row < 3; tap_row++) begin
      for (tap_col = 0; tap_col < 3; tap_col++) begin
        coeff      = filter_coeff(mode, tap_row, tap_col);
        sample     = get_block_sample(block_flat, row + tap_row - 1, col + tap_col - 1);
        coeff_ext  = {{(ACC_WIDTH-COEFF_WIDTH){coeff[COEFF_WIDTH-1]}}, coeff};
        sample_ext = {{(ACC_WIDTH-SAMPLE_WIDTH){1'b0}}, sample};
        product    = sample_ext * coeff_ext;
        acc        = acc + product;
      end
    end

    unique case (mode)
      FILTER_BLUR: normalized = acc >>> 4;
      default: normalized = acc;
    endcase

    return clamp_to_sample(normalized);
  endfunction

  function automatic logic [OUTPUT_BITS-1:0] calc_output_beat(
    input logic [BLOCK_BITS-1:0]       block_flat,
    input logic [1:0]                  mode,
    input logic [BEAT_INDEX_WIDTH-1:0] beat_index
  );
    logic [OUTPUT_BITS-1:0] result;
    int                     lane;
    int                     beat_base;
    int                     global_index;

    result = '0;
    beat_base = beat_index;
    for (lane = 0; lane < LANES; lane++) begin
      global_index = (beat_base * LANES) + lane;
      if (global_index < BLOCK_SAMPLES) begin
        result[(lane*SAMPLE_WIDTH) +: SAMPLE_WIDTH] =
          calc_filtered_sample(block_flat, mode, global_index);
      end
    end

    return result;
  endfunction

  function automatic logic [LANES-1:0] calc_sample_keep(
    input logic [BEAT_INDEX_WIDTH-1:0] beat_index
  );
    logic [LANES-1:0] result;
    int               lane;
    int               beat_base;
    int               global_index;

    result = '0;
    beat_base = beat_index;
    for (lane = 0; lane < LANES; lane++) begin
      global_index = (beat_base * LANES) + lane;
      if (global_index < BLOCK_SAMPLES) begin
        result[lane] = 1'b1;
      end
    end

    return result;
  endfunction

  always_comb begin
    state_d = state_q;

    unique case (state_q)
      STATE_IDLE: begin
        if (input_fire) begin
          state_d = STATE_EMIT;
        end
      end
      STATE_EMIT: begin
        if (output_fire && final_beat) begin
          state_d = STATE_IDLE;
        end
      end
      default: begin
        state_d = STATE_IDLE;
      end
    endcase
  end

  always_ff @(posedge clk) begin
    if (!rst_n) begin
      state_q       <= STATE_IDLE;
      block_data_q  <= '0;
      filter_mode_q <= '0;
      beat_index_q  <= '0;
      out_valid     <= 1'b0;
      sample_data   <= '0;
      sample_keep   <= '0;
      out_last      <= 1'b0;
    end else begin
      state_q <= state_d;

      unique case (state_q)
        STATE_IDLE: begin
          out_valid <= 1'b0;
          out_last  <= 1'b0;

          if (input_fire) begin
            block_data_q  <= block_data;
            filter_mode_q <= filter_mode;
            beat_index_q  <= '0;
            out_valid     <= 1'b1;
            sample_data   <= calc_output_beat(block_data, filter_mode, '0);
            sample_keep   <= calc_sample_keep('0);
            out_last      <= (BEAT_COUNT == 1);
          end else begin
            sample_data <= '0;
            sample_keep <= '0;
          end
        end
        STATE_EMIT: begin
          if (output_fire) begin
            if (final_beat) begin
              beat_index_q <= '0;
              out_valid    <= 1'b0;
              sample_data  <= '0;
              sample_keep  <= '0;
              out_last     <= 1'b0;
            end else begin
              beat_index_q <= beat_index_q + 1'b1;
              sample_data  <= calc_output_beat(block_data_q, filter_mode_q, beat_index_q + 1'b1);
              sample_keep  <= calc_sample_keep(beat_index_q + 1'b1);
              out_last     <= ((beat_index_q + 1'b1) == (BEAT_COUNT - 1));
            end
          end
        end
        default: begin
          block_data_q  <= '0;
          filter_mode_q <= '0;
          beat_index_q <= '0;
          out_valid    <= 1'b0;
          sample_data  <= '0;
          sample_keep  <= '0;
          out_last     <= 1'b0;
        end
      endcase
    end
  end

endmodule

`endif
