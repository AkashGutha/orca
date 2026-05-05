`ifndef RTL_PLAYGROUND_TENSOR_UNIT_SV
`include "rtl/tensor_unit.sv"
`endif

module tensor_unit_8bit #(
  parameter int DATA_WIDTH   = 8,
  parameter int MAT_DIM      = 2,
  parameter int ACC_WIDTH    = (2 * DATA_WIDTH) + ((MAT_DIM <= 1) ? 0 : $clog2(MAT_DIM)),
  parameter bit ENABLE_CACHE = 1'b0,
  parameter int CACHE_DEPTH  = 4
) (
  input  logic clk,
  input  logic rst_n,
  // Passed through to the generic tensor_unit RTL behavioral scan chain.
  // This wrapper has no sequential state of its own.
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
  output logic scan_out
);

  tensor_unit #(
    .DATA_WIDTH   (DATA_WIDTH),
    .MAT_DIM      (MAT_DIM),
    .ACC_WIDTH    (ACC_WIDTH),
    .ENABLE_CACHE (ENABLE_CACHE),
    .CACHE_DEPTH  (CACHE_DEPTH)
  ) u_tensor_unit (
    .clk        (clk),
    .rst_n      (rst_n),
    .dft_mode   (dft_mode),
    .scan_enable(scan_enable),
    .scan_in    (scan_in),
    .in_valid   (in_valid),
    .in_ready   (in_ready),
    .a_data     (a_data),
    .b_data     (b_data),
    .out_valid  (out_valid),
    .out_ready  (out_ready),
    .c_data     (c_data),
    .cache_hit  (),
    .cache_miss (),
    .scan_out   (scan_out)
  );

endmodule
