`timescale 1ns/1ps

interface tensor_unit_dft_if #(
  parameter int DATA_WIDTH  = 16,
  parameter int MAT_DIM     = 2,
  parameter int ACC_WIDTH   = (2 * DATA_WIDTH) + ((MAT_DIM <= 1) ? 0 : $clog2(MAT_DIM)),
  parameter int CACHE_DEPTH = 4
) (
  input logic clk
);
  localparam int MAT_ELEMS = MAT_DIM * MAT_DIM;
  localparam int A_BITS    = MAT_ELEMS * DATA_WIDTH;
  localparam int C_BITS    = MAT_ELEMS * ACC_WIDTH;

  logic rst_n;

  logic dft_mode;
  logic scan_enable;
  logic scan_in;
  logic scan_out;

  logic in_valid;
  logic in_ready;
  logic [A_BITS-1:0] a_data;
  logic [A_BITS-1:0] b_data;

  logic out_valid;
  logic out_ready;
  logic [C_BITS-1:0] c_data;
  logic cache_hit;
  logic cache_miss;

  int unsigned scenario;
endinterface
