`timescale 1ns/1ps

module tb_jpeg_filter_applier;
  import uvm_pkg::*;
  import jpeg_filter_uvm_pkg::*;

  bit clk;
  string fsdb_file;

  jpeg_filter_if #(.SAMPLE_WIDTH(8), .BLOCK_DIM(8), .LANES(1))  vif_lanes1  (.clk(clk));
  jpeg_filter_if #(.SAMPLE_WIDTH(8), .BLOCK_DIM(8), .LANES(5))  vif_lanes5  (.clk(clk));
  jpeg_filter_if #(.SAMPLE_WIDTH(8), .BLOCK_DIM(8), .LANES(8))  vif_lanes8  (.clk(clk));
  jpeg_filter_if #(.SAMPLE_WIDTH(8), .BLOCK_DIM(8), .LANES(64)) vif_lanes64 (.clk(clk));

  initial begin
    clk = 1'b0;
    forever #5 clk = ~clk;
  end

`ifdef FSDB_DUMP
  initial begin
    if (!$value$plusargs("FSDB_FILE=%s", fsdb_file)) begin
      fsdb_file = "sim/tb_jpeg_filter_applier.fsdb";
    end
    $fsdbDumpfile(fsdb_file);
    $fsdbDumpvars(0, tb_jpeg_filter_applier);
  end
`endif

  initial begin
    vif_lanes1.rst_n = 1'b0;
    vif_lanes5.rst_n = 1'b0;
    vif_lanes8.rst_n = 1'b0;
    vif_lanes64.rst_n = 1'b0;

    vif_lanes1.in_valid = 1'b0;
    vif_lanes5.in_valid = 1'b0;
    vif_lanes8.in_valid = 1'b0;
    vif_lanes64.in_valid = 1'b0;

    vif_lanes1.block_data = '0;
    vif_lanes5.block_data = '0;
    vif_lanes8.block_data = '0;
    vif_lanes64.block_data = '0;

    vif_lanes1.filter_mode = '0;
    vif_lanes5.filter_mode = '0;
    vif_lanes8.filter_mode = '0;
    vif_lanes64.filter_mode = '0;

    vif_lanes1.out_ready = 1'b1;
    vif_lanes5.out_ready = 1'b1;
    vif_lanes8.out_ready = 1'b1;
    vif_lanes64.out_ready = 1'b1;
  end

  jpeg_filter_applier #(
    .SAMPLE_WIDTH (8),
    .BLOCK_DIM    (8),
    .LANES        (1)
  ) dut_lanes1 (
    .clk         (vif_lanes1.clk),
    .rst_n       (vif_lanes1.rst_n),
    .in_valid    (vif_lanes1.in_valid),
    .in_ready    (vif_lanes1.in_ready),
    .block_data  (vif_lanes1.block_data),
    .filter_mode (vif_lanes1.filter_mode),
    .out_valid   (vif_lanes1.out_valid),
    .out_ready   (vif_lanes1.out_ready),
    .sample_data (vif_lanes1.sample_data),
    .sample_keep (vif_lanes1.sample_keep),
    .out_last    (vif_lanes1.out_last)
  );

  jpeg_filter_applier #(
    .SAMPLE_WIDTH (8),
    .BLOCK_DIM    (8),
    .LANES        (5)
  ) dut_lanes5 (
    .clk         (vif_lanes5.clk),
    .rst_n       (vif_lanes5.rst_n),
    .in_valid    (vif_lanes5.in_valid),
    .in_ready    (vif_lanes5.in_ready),
    .block_data  (vif_lanes5.block_data),
    .filter_mode (vif_lanes5.filter_mode),
    .out_valid   (vif_lanes5.out_valid),
    .out_ready   (vif_lanes5.out_ready),
    .sample_data (vif_lanes5.sample_data),
    .sample_keep (vif_lanes5.sample_keep),
    .out_last    (vif_lanes5.out_last)
  );

  jpeg_filter_applier #(
    .SAMPLE_WIDTH (8),
    .BLOCK_DIM    (8),
    .LANES        (8)
  ) dut_lanes8 (
    .clk         (vif_lanes8.clk),
    .rst_n       (vif_lanes8.rst_n),
    .in_valid    (vif_lanes8.in_valid),
    .in_ready    (vif_lanes8.in_ready),
    .block_data  (vif_lanes8.block_data),
    .filter_mode (vif_lanes8.filter_mode),
    .out_valid   (vif_lanes8.out_valid),
    .out_ready   (vif_lanes8.out_ready),
    .sample_data (vif_lanes8.sample_data),
    .sample_keep (vif_lanes8.sample_keep),
    .out_last    (vif_lanes8.out_last)
  );

  jpeg_filter_applier #(
    .SAMPLE_WIDTH (8),
    .BLOCK_DIM    (8),
    .LANES        (64)
  ) dut_lanes64 (
    .clk         (vif_lanes64.clk),
    .rst_n       (vif_lanes64.rst_n),
    .in_valid    (vif_lanes64.in_valid),
    .in_ready    (vif_lanes64.in_ready),
    .block_data  (vif_lanes64.block_data),
    .filter_mode (vif_lanes64.filter_mode),
    .out_valid   (vif_lanes64.out_valid),
    .out_ready   (vif_lanes64.out_ready),
    .sample_data (vif_lanes64.sample_data),
    .sample_keep (vif_lanes64.sample_keep),
    .out_last    (vif_lanes64.out_last)
  );

  initial begin
    uvm_config_db #(virtual jpeg_filter_if #(8, 8, 1))::set(null, "uvm_test_top", "vif_lanes1", vif_lanes1);
    uvm_config_db #(virtual jpeg_filter_if #(8, 8, 5))::set(null, "uvm_test_top", "vif_lanes5", vif_lanes5);
    uvm_config_db #(virtual jpeg_filter_if #(8, 8, 8))::set(null, "uvm_test_top", "vif_lanes8", vif_lanes8);
    uvm_config_db #(virtual jpeg_filter_if #(8, 8, 64))::set(null, "uvm_test_top", "vif_lanes64", vif_lanes64);
    run_test("jpeg_filter_all_lanes_test");
  end

  initial begin : watchdog
    #500000;
    $fatal(1, "tb_jpeg_filter_applier watchdog timeout");
  end
endmodule
