`timescale 1ns/1ps

module tb_tensor_unit_dft_uvm;
  import uvm_pkg::*;
  import tensor_unit_dft_uvm_pkg::*;

  logic clk;

  initial begin
    clk = 1'b0;
    forever #5 clk = ~clk;
  end

  tensor_unit_dft_if #(
    .DATA_WIDTH(DATA_WIDTH),
    .MAT_DIM(MAT_DIM),
    .ACC_WIDTH(ACC_WIDTH),
    .CACHE_DEPTH(CACHE_DEPTH)
  ) tensor_if (
    .clk(clk)
  );

  tensor_unit #(
    .DATA_WIDTH(DATA_WIDTH),
    .MAT_DIM(MAT_DIM),
    .ACC_WIDTH(ACC_WIDTH),
    .ENABLE_CACHE(1'b1),
    .CACHE_DEPTH(CACHE_DEPTH)
  ) dut (
    .clk         (tensor_if.clk),
    .rst_n       (tensor_if.rst_n),
    .dft_mode    (tensor_if.dft_mode),
    .scan_enable (tensor_if.scan_enable),
    .scan_in     (tensor_if.scan_in),
    .in_valid    (tensor_if.in_valid),
    .in_ready    (tensor_if.in_ready),
    .a_data      (tensor_if.a_data),
    .b_data      (tensor_if.b_data),
    .out_valid   (tensor_if.out_valid),
    .out_ready   (tensor_if.out_ready),
    .c_data      (tensor_if.c_data),
    .cache_hit   (tensor_if.cache_hit),
    .cache_miss  (tensor_if.cache_miss),
    .scan_out    (tensor_if.scan_out)
  );

  initial begin
`ifdef FSDB_DUMP
    string fsdb_file;
    if (!$value$plusargs("FSDB_FILE=%s", fsdb_file)) begin
      fsdb_file = "sim/tb_tensor_unit_dft_uvm.fsdb";
    end
    $fsdbDumpfile(fsdb_file);
    $fsdbDumpvars(0, tb_tensor_unit_dft_uvm);
`endif
  end

  initial begin
    uvm_config_db#(tensor_unit_dft_vif)::set(null, "uvm_test_top.env.agent.*", "vif", tensor_if);
    run_test("tensor_unit_dft_test");
  end
endmodule
