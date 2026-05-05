`ifndef JPEG_FILTER_IF_SV
`define JPEG_FILTER_IF_SV

interface jpeg_filter_if #(
  parameter int SAMPLE_WIDTH = 8,
  parameter int BLOCK_DIM    = 8,
  parameter int LANES        = 8
) (
  input logic clk
);
  localparam int BLOCK_SAMPLES = BLOCK_DIM * BLOCK_DIM;
  localparam int BLOCK_BITS    = BLOCK_SAMPLES * SAMPLE_WIDTH;
  localparam int SAMPLE_BITS   = LANES * SAMPLE_WIDTH;

  logic rst_n;

  logic                    in_valid;
  logic                    in_ready;
  logic [BLOCK_BITS-1:0]   block_data;
  logic [1:0]              filter_mode;

  logic                    out_valid;
  logic                    out_ready;
  logic [SAMPLE_BITS-1:0]  sample_data;
  logic [LANES-1:0]        sample_keep;
  logic                    out_last;

  modport dut (
    input  clk,
    input  rst_n,
    input  in_valid,
    output in_ready,
    input  block_data,
    input  filter_mode,
    output out_valid,
    input  out_ready,
    output sample_data,
    output sample_keep,
    output out_last
  );

  modport tb (
    input  clk,
    output rst_n,
    output in_valid,
    input  in_ready,
    output block_data,
    output filter_mode,
    input  out_valid,
    output out_ready,
    input  sample_data,
    input  sample_keep,
    input  out_last
  );

  property p_outputs_stable_while_stalled;
    @(posedge clk) disable iff (!rst_n)
      out_valid && !out_ready |=> out_valid &&
        $stable(sample_data) && $stable(sample_keep) && $stable(out_last);
  endproperty

  property p_out_last_only_when_valid;
    @(posedge clk) disable iff (!rst_n)
      out_last |-> out_valid;
  endproperty

  property p_output_known_and_keep_nonzero;
    @(posedge clk) disable iff (!rst_n)
      out_valid |-> !$isunknown({sample_data, sample_keep, out_last}) &&
        (sample_keep != '0);
  endproperty

  property p_in_ready_low_while_emitting;
    @(posedge clk) disable iff (!rst_n)
      out_valid |-> !in_ready;
  endproperty

  property p_first_output_after_accept;
    @(posedge clk) disable iff (!rst_n)
      in_valid && in_ready |=> out_valid;
  endproperty

  property p_in_ready_returns_after_last;
    @(posedge clk) disable iff (!rst_n)
      out_valid && out_ready && out_last |=> in_ready;
  endproperty

  a_outputs_stable_while_stalled:
    assert property (p_outputs_stable_while_stalled)
    else $error("jpeg_filter_if LANES=%0d: outputs changed while out_valid && !out_ready", LANES);

  a_out_last_only_when_valid:
    assert property (p_out_last_only_when_valid)
    else $error("jpeg_filter_if LANES=%0d: out_last asserted without out_valid", LANES);

  a_output_known_and_keep_nonzero:
    assert property (p_output_known_and_keep_nonzero)
    else $error("jpeg_filter_if LANES=%0d: output beat has unknown data/control or zero sample_keep", LANES);

  a_in_ready_low_while_emitting:
    assert property (p_in_ready_low_while_emitting)
    else $error("jpeg_filter_if LANES=%0d: in_ready asserted while output block is active", LANES);

  a_first_output_after_accept:
    assert property (p_first_output_after_accept)
    else $error("jpeg_filter_if LANES=%0d: first output beat did not appear one cycle after input accept", LANES);

  a_in_ready_returns_after_last:
    assert property (p_in_ready_returns_after_last)
    else $error("jpeg_filter_if LANES=%0d: in_ready did not return after final output handshake", LANES);
endinterface

`endif
