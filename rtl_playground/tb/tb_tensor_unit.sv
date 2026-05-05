`timescale 1ns/1ps

`ifndef TENSOR_UNIT_HAS_DFT_PORTS
`define TENSOR_UNIT_HAS_DFT_PORTS
`endif

package tensor_unit_tb_pkg;
  localparam int MODE_DEFAULT  = 0;
  localparam int MODE_NO_CACHE = 1;
  localparam int MODE_DEPTH2   = 2;
  localparam int MODE_DEPTH1   = 3;
  localparam int MODE_DIM1     = 4;
  localparam int FIXTURE_BITS  = 12;

  localparam int FIXTURE_DEFAULT16_CACHE    = 0;
  localparam int FIXTURE_CACHE_DISABLED     = 1;
  localparam int FIXTURE_CACHE_DEPTH2       = 2;
  localparam int FIXTURE_CACHE_DEPTH1       = 3;
  localparam int FIXTURE_MAT_DIM1           = 4;
  localparam int FIXTURE_LEGACY8            = 5;
  localparam int FIXTURE_GENERIC_DFT        = 6;
  localparam int FIXTURE_LEGACY8_DFT        = 7;
  localparam int FIXTURE_CACHE_DISABLED_DFT = 8;
  localparam int FIXTURE_CACHE_DEPTH1_DFT   = 9;
  localparam int FIXTURE_CACHE_DEPTH2_DFT   = 10;
  localparam int FIXTURE_MAT_DIM1_DFT       = 11;

  int unsigned error_count;
  int unsigned fixture_done_count;
  bit [FIXTURE_BITS-1:0] fixture_done_seen;

  task automatic record_error(input string scope, input string message);
    begin
      error_count++;
      $display("ERROR[%s]: %s", scope, message);
    end
  endtask

  function automatic int fixture_index(input string scope);
    begin
      if (scope == "default16_cache") begin
        return FIXTURE_DEFAULT16_CACHE;
      end else if (scope == "cache_disabled") begin
        return FIXTURE_CACHE_DISABLED;
      end else if (scope == "cache_depth2") begin
        return FIXTURE_CACHE_DEPTH2;
      end else if (scope == "cache_depth1") begin
        return FIXTURE_CACHE_DEPTH1;
      end else if (scope == "mat_dim1") begin
        return FIXTURE_MAT_DIM1;
      end else if (scope == "legacy8") begin
        return FIXTURE_LEGACY8;
      end else if (scope == "generic_dft") begin
        return FIXTURE_GENERIC_DFT;
      end else if (scope == "legacy8_dft") begin
        return FIXTURE_LEGACY8_DFT;
      end else if (scope == "cache_disabled_dft") begin
        return FIXTURE_CACHE_DISABLED_DFT;
      end else if (scope == "cache_depth1_dft") begin
        return FIXTURE_CACHE_DEPTH1_DFT;
      end else if (scope == "cache_depth2_dft") begin
        return FIXTURE_CACHE_DEPTH2_DFT;
      end else if (scope == "mat_dim1_dft") begin
        return FIXTURE_MAT_DIM1_DFT;
      end
      return -1;
    end
  endfunction

  function automatic string fixture_name(input int fixture_id);
    begin
      case (fixture_id)
        FIXTURE_DEFAULT16_CACHE:    return "default16_cache";
        FIXTURE_CACHE_DISABLED:     return "cache_disabled";
        FIXTURE_CACHE_DEPTH2:       return "cache_depth2";
        FIXTURE_CACHE_DEPTH1:       return "cache_depth1";
        FIXTURE_MAT_DIM1:           return "mat_dim1";
        FIXTURE_LEGACY8:            return "legacy8";
        FIXTURE_GENERIC_DFT:        return "generic_dft";
        FIXTURE_LEGACY8_DFT:        return "legacy8_dft";
        FIXTURE_CACHE_DISABLED_DFT: return "cache_disabled_dft";
        FIXTURE_CACHE_DEPTH1_DFT:   return "cache_depth1_dft";
        FIXTURE_CACHE_DEPTH2_DFT:   return "cache_depth2_dft";
        FIXTURE_MAT_DIM1_DFT:       return "mat_dim1_dft";
        default:                    return "<unknown>";
      endcase
    end
  endfunction

  task automatic mark_fixture_done(input string scope);
    int fixture_id;
    begin
      fixture_id = fixture_index(scope);
      if (fixture_id < 0) begin
        record_error("tb", {"unknown fixture completed: ", scope});
      end else if (fixture_done_seen[fixture_id]) begin
        record_error("tb", {"duplicate fixture completion: ", scope});
      end else begin
        fixture_done_seen[fixture_id] = 1'b1;
      end
      fixture_done_count++;
      $display("INFO[%s]: fixture complete", scope);
    end
  endtask

  task automatic check_fixture_execution(
    input bit [FIXTURE_BITS-1:0] expected_seen,
    input int unsigned expected_count
  );
    begin
      if (fixture_done_count != expected_count) begin
        record_error("tb", $sformatf("expected %0d fixture completions, observed %0d",
                                     expected_count, fixture_done_count));
      end
      for (int fixture_id = 0; fixture_id < FIXTURE_BITS; fixture_id++) begin
        if (expected_seen[fixture_id] && !fixture_done_seen[fixture_id]) begin
          record_error("tb", {"fixture did not execute: ", fixture_name(fixture_id)});
        end
        if (!expected_seen[fixture_id] && fixture_done_seen[fixture_id]) begin
          record_error("tb", {"unexpected fixture executed: ", fixture_name(fixture_id)});
        end
      end
    end
  endtask

  task automatic log_check(input string label);
    begin
      $display("CHECK: %s", label);
    end
  endtask
endpackage

module tensor_unit_scoreboard #(
  parameter int DATA_WIDTH         = 16,
  parameter int MAT_DIM            = 2,
  parameter int ACC_WIDTH          = (2 * DATA_WIDTH) + $clog2(MAT_DIM),
  parameter bit ENABLE_CACHE       = 1'b1,
  parameter int CACHE_DEPTH        = 4,
  parameter bit CHECK_CACHE_STATUS = 1'b1,
  parameter string NAME            = "scoreboard"
) (
  input logic clk,
  input logic rst_n,

  input logic in_valid,
  input logic in_ready,
  input logic [MAT_DIM*MAT_DIM*DATA_WIDTH-1:0] a_data,
  input logic [MAT_DIM*MAT_DIM*DATA_WIDTH-1:0] b_data,

  input logic out_valid,
  input logic out_ready,
  input logic [MAT_DIM*MAT_DIM*ACC_WIDTH-1:0] c_data,
  input logic cache_hit,
  input logic cache_miss
);
  import tensor_unit_tb_pkg::*;

  localparam int MAT_ELEMS = MAT_DIM * MAT_DIM;
  localparam int A_BITS    = MAT_ELEMS * DATA_WIDTH;
  localparam int C_BITS    = MAT_ELEMS * ACC_WIDTH;
  localparam int KEY_BITS  = 2 * A_BITS;
  localparam int CACHE_INDEX_WIDTH = (CACHE_DEPTH <= 1) ? 1 : $clog2(CACHE_DEPTH);

  typedef struct packed {
    logic [C_BITS-1:0] c_data;
    logic hit;
    logic miss;
  } expected_t;

  expected_t expected_q[$];

  logic [KEY_BITS-1:0] cache_key [CACHE_DEPTH];
  logic [C_BITS-1:0]   cache_value [CACHE_DEPTH];
  logic                cache_valid [CACHE_DEPTH];
  logic [CACHE_INDEX_WIDTH-1:0] replacement_ptr;

  bit pending_valid;
  expected_t pending_expected;

  bit prev_valid;
  bit prev_ready;
  logic [C_BITS-1:0] prev_c_data;
  logic prev_cache_hit;
  logic prev_cache_miss;

  covergroup tensor_unit_cg @(posedge clk);
    option.per_instance = 1;
    cp_accept: coverpoint (rst_n && in_valid && in_ready) {
      bins no_accept = {0};
      bins accept    = {1};
    }
    cp_output_stall: coverpoint (rst_n && out_valid && !out_ready) {
      bins no_stall = {0};
      bins stall    = {1};
    }
    cp_output_consume: coverpoint (rst_n && out_valid && out_ready) {
      bins no_consume = {0};
      bins consume    = {1};
    }
    cp_cache_hit: coverpoint cache_hit iff (CHECK_CACHE_STATUS && rst_n && out_valid) {
      bins miss_path = {0};
      bins hit_path  = {1};
    }
    cp_cache_miss: coverpoint cache_miss iff (CHECK_CACHE_STATUS && rst_n && out_valid) {
      bins no_miss = {0};
      bins miss    = {1};
    }
    cp_ready: coverpoint in_ready iff (rst_n) {
      bins not_ready = {0};
      bins ready     = {1};
    }
  endgroup

  tensor_unit_cg cov = new();

  property p_ready_equation;
    @(posedge clk) disable iff (!rst_n)
      in_ready == (!out_valid || out_ready);
  endproperty

  assert property (p_ready_equation)
    else record_error(NAME, "in_ready did not match !out_valid || out_ready");

  function automatic logic [C_BITS-1:0] ref_matmul(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat
  );
    logic [C_BITS-1:0] result;
    logic [ACC_WIDTH-1:0] acc;
    logic [ACC_WIDTH-1:0] a_ext;
    logic [ACC_WIDTH-1:0] b_ext;
    int row;
    int col;
    int k;
    int a_index;
    int b_index;
    int c_index;
    begin
      result = '0;
      for (row = 0; row < MAT_DIM; row++) begin
        for (col = 0; col < MAT_DIM; col++) begin
          acc = '0;
          for (k = 0; k < MAT_DIM; k++) begin
            a_index = (row * MAT_DIM) + k;
            b_index = (k * MAT_DIM) + col;
            a_ext = '0;
            b_ext = '0;
            a_ext[DATA_WIDTH-1:0] = a_flat[(a_index*DATA_WIDTH) +: DATA_WIDTH];
            b_ext[DATA_WIDTH-1:0] = b_flat[(b_index*DATA_WIDTH) +: DATA_WIDTH];
            acc = acc + (a_ext * b_ext);
          end
          c_index = (row * MAT_DIM) + col;
          result[(c_index*ACC_WIDTH) +: ACC_WIDTH] = acc;
        end
      end
      return result;
    end
  endfunction

  task automatic reset_model;
    int idx;
    begin
      expected_q.delete();
      pending_valid = 1'b0;
      replacement_ptr = '0;
      for (idx = 0; idx < CACHE_DEPTH; idx++) begin
        cache_valid[idx] = 1'b0;
        cache_key[idx]   = '0;
        cache_value[idx] = '0;
      end
    end
  endtask

  task automatic model_accept(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat,
    output expected_t expected
  );
    logic [KEY_BITS-1:0] key;
    logic [C_BITS-1:0] computed;
    bit found;
    int hit_index;
    int idx;
    begin
      key = {a_flat, b_flat};
      computed = ref_matmul(a_flat, b_flat);
      found = 1'b0;
      hit_index = 0;

      expected.c_data = computed;
      expected.hit    = 1'b0;
      expected.miss   = ENABLE_CACHE;

      if (ENABLE_CACHE) begin
        for (idx = 0; idx < CACHE_DEPTH; idx++) begin
          if (cache_valid[idx] && (cache_key[idx] == key)) begin
            found = 1'b1;
            hit_index = idx;
          end
        end

        if (found) begin
          expected.c_data = cache_value[hit_index];
          expected.hit    = 1'b1;
          expected.miss   = 1'b0;
        end else begin
          expected.hit    = 1'b0;
          expected.miss   = 1'b1;
          cache_valid[replacement_ptr] = 1'b1;
          cache_key[replacement_ptr]   = key;
          cache_value[replacement_ptr] = computed;
          if (replacement_ptr == CACHE_DEPTH-1) begin
            replacement_ptr = '0;
          end else begin
            replacement_ptr = replacement_ptr + 1'b1;
          end
        end
      end
    end
  endtask

  function automatic int pending_count();
    return expected_q.size() + (pending_valid ? 1 : 0);
  endfunction

  task automatic check_empty(input string check_name);
    begin
      if (expected_q.size() != 0) begin
        record_error(NAME, $sformatf("%s: scoreboard still has %0d queued outputs",
                                     check_name, expected_q.size()));
      end
      if (pending_valid) begin
        record_error(NAME, $sformatf("%s: scoreboard still has a pending latency check", check_name));
      end
    end
  endtask

  task automatic compare_expected(input expected_t expected, input logic [C_BITS-1:0] actual_c);
    begin
      if (actual_c !== expected.c_data) begin
        record_error(NAME, $sformatf("c_data mismatch expected=0x%0h actual=0x%0h",
                                     expected.c_data, actual_c));
      end
      if (CHECK_CACHE_STATUS) begin
        if (cache_hit !== expected.hit) begin
          record_error(NAME, $sformatf("cache_hit mismatch expected=%0b actual=%0b",
                                       expected.hit, cache_hit));
        end
        if (cache_miss !== expected.miss) begin
          record_error(NAME, $sformatf("cache_miss mismatch expected=%0b actual=%0b",
                                       expected.miss, cache_miss));
        end
        if (cache_hit && cache_miss) begin
          record_error(NAME, "cache_hit and cache_miss were both asserted");
        end
        if (!ENABLE_CACHE && ((cache_hit !== 1'b0) || (cache_miss !== 1'b0))) begin
          record_error(NAME, "cache status asserted while ENABLE_CACHE=0");
        end
      end
    end
  endtask

  initial begin
    expected_t accepted_expected;
    expected_t consumed_expected;
    bit sampled_rst_n;
    bit sampled_accept;
    bit sampled_consume;
    logic [A_BITS-1:0] sampled_a;
    logic [A_BITS-1:0] sampled_b;

    reset_model();
    prev_valid = 1'b0;
    prev_ready = 1'b1;
    prev_c_data = '0;
    prev_cache_hit = 1'b0;
    prev_cache_miss = 1'b0;

    forever begin
      @(posedge clk);
      sampled_rst_n = rst_n;
      sampled_accept = rst_n && in_valid && in_ready;
      sampled_consume = rst_n && out_valid && out_ready;
      sampled_a = a_data;
      sampled_b = b_data;

      if (sampled_rst_n && pending_valid) begin
        if (out_valid !== 1'b1) begin
          record_error(NAME, "accepted input did not produce out_valid in the next cycle");
        end else begin
          compare_expected(pending_expected, c_data);
        end
        pending_valid = 1'b0;
      end

      if (sampled_consume) begin
        if (expected_q.size() == 0) begin
          record_error(NAME, "observed output with an empty expected queue");
        end else begin
          consumed_expected = expected_q.pop_front();
          compare_expected(consumed_expected, c_data);
        end
      end

      if (!sampled_rst_n) begin
        reset_model();
      end else if (sampled_accept) begin
        model_accept(sampled_a, sampled_b, accepted_expected);
        expected_q.push_back(accepted_expected);
        pending_expected = accepted_expected;
        pending_valid = 1'b1;
      end

      #1;
      if (!rst_n) begin
        if (out_valid !== 1'b0) begin
          record_error(NAME, "reset did not clear out_valid");
        end
        if (c_data !== '0) begin
          record_error(NAME, "reset did not clear c_data");
        end
        if (CHECK_CACHE_STATUS && ((cache_hit !== 1'b0) || (cache_miss !== 1'b0))) begin
          record_error(NAME, "reset did not clear cache status");
        end
      end else begin
        if (prev_valid && !prev_ready && !out_ready) begin
          if (out_valid !== 1'b1) begin
            record_error(NAME, "out_valid changed while output was backpressured");
          end
          if (c_data !== prev_c_data) begin
            record_error(NAME, "c_data changed while output was backpressured");
          end
          if (CHECK_CACHE_STATUS) begin
            if (cache_hit !== prev_cache_hit) begin
              record_error(NAME, "cache_hit changed while output was backpressured");
            end
            if (cache_miss !== prev_cache_miss) begin
              record_error(NAME, "cache_miss changed while output was backpressured");
            end
          end
        end

        if (CHECK_CACHE_STATUS && !out_valid && ((cache_hit !== 1'b0) || (cache_miss !== 1'b0))) begin
          record_error(NAME, "cache status asserted while out_valid was low");
        end
      end

      prev_valid = out_valid;
      prev_ready = out_ready;
      prev_c_data = c_data;
      prev_cache_hit = cache_hit;
      prev_cache_miss = cache_miss;
    end
  end
endmodule

module tensor_unit_generic_fixture #(
  parameter int MODE = tensor_unit_tb_pkg::MODE_DEFAULT,
  parameter string NAME = "generic",
  parameter bit USE_DUT_DEFAULTS = 1'b0,
  parameter int DATA_WIDTH = 16,
  parameter int MAT_DIM = 2,
  parameter int ACC_WIDTH = (2 * DATA_WIDTH) + $clog2(MAT_DIM),
  parameter bit ENABLE_CACHE = 1'b1,
  parameter int CACHE_DEPTH = 4
);
  import tensor_unit_tb_pkg::*;

  localparam int MAT_ELEMS = MAT_DIM * MAT_DIM;
  localparam int A_BITS = MAT_ELEMS * DATA_WIDTH;
  localparam int C_BITS = MAT_ELEMS * ACC_WIDTH;

  logic clk;
  logic rst_n;
  logic in_valid;
  logic in_ready;
  logic [A_BITS-1:0] a_data;
  logic [A_BITS-1:0] b_data;
  logic out_valid;
  logic out_ready;
  logic [C_BITS-1:0] c_data;
  logic cache_hit;
  logic cache_miss;
`ifdef TENSOR_UNIT_HAS_DFT_PORTS
  logic dft_mode;
  logic scan_enable;
  logic scan_in;
  logic scan_out;
`endif

  generate
    if (USE_DUT_DEFAULTS) begin : gen_default_dut
      tensor_unit dut (
        .clk        (clk),
        .rst_n      (rst_n),
        .in_valid   (in_valid),
        .in_ready   (in_ready),
        .a_data     (a_data),
        .b_data     (b_data),
        .out_valid  (out_valid),
        .out_ready  (out_ready),
        .c_data     (c_data),
        .cache_hit  (cache_hit),
        .cache_miss (cache_miss)
`ifdef TENSOR_UNIT_HAS_DFT_PORTS
        ,
        .dft_mode    (dft_mode),
        .scan_enable (scan_enable),
        .scan_in     (scan_in),
        .scan_out    (scan_out)
`endif
      );
    end else begin : gen_param_dut
      tensor_unit #(
        .DATA_WIDTH   (DATA_WIDTH),
        .MAT_DIM      (MAT_DIM),
        .ACC_WIDTH    (ACC_WIDTH),
        .ENABLE_CACHE (ENABLE_CACHE),
        .CACHE_DEPTH  (CACHE_DEPTH)
      ) dut (
        .clk        (clk),
        .rst_n      (rst_n),
        .in_valid   (in_valid),
        .in_ready   (in_ready),
        .a_data     (a_data),
        .b_data     (b_data),
        .out_valid  (out_valid),
        .out_ready  (out_ready),
        .c_data     (c_data),
        .cache_hit  (cache_hit),
        .cache_miss (cache_miss)
`ifdef TENSOR_UNIT_HAS_DFT_PORTS
        ,
        .dft_mode    (dft_mode),
        .scan_enable (scan_enable),
        .scan_in     (scan_in),
        .scan_out    (scan_out)
`endif
      );
    end
  endgenerate

  tensor_unit_scoreboard #(
    .DATA_WIDTH         (DATA_WIDTH),
    .MAT_DIM            (MAT_DIM),
    .ACC_WIDTH          (ACC_WIDTH),
    .ENABLE_CACHE       (ENABLE_CACHE),
    .CACHE_DEPTH        (CACHE_DEPTH),
    .CHECK_CACHE_STATUS (1'b1),
    .NAME               (NAME)
  ) scoreboard (
    .clk        (clk),
    .rst_n      (rst_n),
    .in_valid   (in_valid),
    .in_ready   (in_ready),
    .a_data     (a_data),
    .b_data     (b_data),
    .out_valid  (out_valid),
    .out_ready  (out_ready),
    .c_data     (c_data),
    .cache_hit  (cache_hit),
    .cache_miss (cache_miss)
  );

  initial begin
    clk = 1'b0;
    forever #5 clk = ~clk;
  end

  function automatic logic [A_BITS-1:0] make_matrix(
    input logic [DATA_WIDTH-1:0] e0,
    input logic [DATA_WIDTH-1:0] e1,
    input logic [DATA_WIDTH-1:0] e2,
    input logic [DATA_WIDTH-1:0] e3
  );
    logic [A_BITS-1:0] packed_matrix;
    logic [DATA_WIDTH-1:0] value;
    int idx;
    begin
      packed_matrix = '0;
      for (idx = 0; idx < MAT_ELEMS; idx++) begin
        case (idx)
          0: value = e0;
          1: value = e1;
          2: value = e2;
          3: value = e3;
          default: value = '0;
        endcase
        packed_matrix[(idx*DATA_WIDTH) +: DATA_WIDTH] = value;
      end
      return packed_matrix;
    end
  endfunction

  function automatic logic [A_BITS-1:0] all_value_matrix(input logic [DATA_WIDTH-1:0] value);
    logic [A_BITS-1:0] packed_matrix;
    int idx;
    begin
      packed_matrix = '0;
      for (idx = 0; idx < MAT_ELEMS; idx++) begin
        packed_matrix[(idx*DATA_WIDTH) +: DATA_WIDTH] = value;
      end
      return packed_matrix;
    end
  endfunction

  function automatic logic [A_BITS-1:0] identity_matrix();
    logic [A_BITS-1:0] packed_matrix;
    int row;
    int col;
    int idx;
    begin
      packed_matrix = '0;
      for (row = 0; row < MAT_DIM; row++) begin
        for (col = 0; col < MAT_DIM; col++) begin
          idx = (row * MAT_DIM) + col;
          packed_matrix[(idx*DATA_WIDTH) +: DATA_WIDTH] = (row == col) ? {{(DATA_WIDTH-1){1'b0}}, 1'b1} : '0;
        end
      end
      return packed_matrix;
    end
  endfunction

  function automatic logic [C_BITS-1:0] make_result(
    input logic [ACC_WIDTH-1:0] e0,
    input logic [ACC_WIDTH-1:0] e1,
    input logic [ACC_WIDTH-1:0] e2,
    input logic [ACC_WIDTH-1:0] e3
  );
    logic [C_BITS-1:0] packed_result;
    logic [ACC_WIDTH-1:0] value;
    int idx;
    begin
      packed_result = '0;
      for (idx = 0; idx < MAT_ELEMS; idx++) begin
        case (idx)
          0: value = e0;
          1: value = e1;
          2: value = e2;
          3: value = e3;
          default: value = '0;
        endcase
        packed_result[(idx*ACC_WIDTH) +: ACC_WIDTH] = value;
      end
      return packed_result;
    end
  endfunction

  function automatic logic [A_BITS-1:0] pattern_matrix(input int unsigned seed);
    logic [A_BITS-1:0] packed_matrix;
    longint unsigned value;
    int idx;
    begin
      packed_matrix = '0;
      for (idx = 0; idx < MAT_ELEMS; idx++) begin
        value = (64'd97 * (seed + 1)) + (64'd131 * idx) + (64'd17 * seed * idx);
        packed_matrix[(idx*DATA_WIDTH) +: DATA_WIDTH] = value[DATA_WIDTH-1:0];
      end
      return packed_matrix;
    end
  endfunction

  task automatic reset_fixture;
    begin
      @(negedge clk);
      rst_n     = 1'b0;
      in_valid  = 1'b0;
      a_data    = '0;
      b_data    = '0;
      out_ready = 1'b0;
`ifdef TENSOR_UNIT_HAS_DFT_PORTS
      dft_mode    = 1'b0;
      scan_enable = 1'b0;
      scan_in     = 1'b0;
`endif
      repeat (3) @(posedge clk);
      @(negedge clk);
      rst_n     = 1'b1;
      out_ready = 1'b1;
      repeat (2) @(posedge clk);
    end
  endtask

  task automatic send_one(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat,
    input string check_name
  );
    begin
      @(negedge clk);
      while (!in_ready) begin
        @(negedge clk);
      end
      a_data   = a_flat;
      b_data   = b_flat;
      in_valid = 1'b1;

      @(posedge clk);
      if (!(rst_n && in_valid && in_ready)) begin
        record_error(NAME, {check_name, ": input was not accepted"});
      end

      @(negedge clk);
      in_valid = 1'b0;
      a_data   = '0;
      b_data   = '0;
    end
  endtask

  task automatic wait_idle(input string check_name);
    int cycles;
    begin
      out_ready = 1'b1;
      cycles = 0;
      while (((scoreboard.pending_count() != 0) || out_valid) && (cycles < 100)) begin
        @(posedge clk);
        #1;
        cycles++;
      end
      if ((scoreboard.pending_count() != 0) || out_valid) begin
        record_error(NAME, {check_name, ": timed out waiting for fixture to drain"});
      end
      scoreboard.check_empty(check_name);
    end
  endtask

  task automatic send_and_wait(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat,
    input string check_name
  );
    begin
      send_one(a_flat, b_flat, check_name);
      wait_idle(check_name);
    end
  endtask

  task automatic run_no_accept_during_reset;
    begin
      @(negedge clk);
      rst_n     = 1'b0;
      out_ready = 1'b1;
      a_data    = pattern_matrix(1);
      b_data    = pattern_matrix(2);
      in_valid  = 1'b1;
      repeat (3) @(posedge clk);
      @(negedge clk);
      in_valid = 1'b0;
      a_data   = '0;
      b_data   = '0;
      rst_n    = 1'b1;
      out_ready = 1'b1;
      repeat (3) @(posedge clk);
      wait_idle("no accept during reset");
    end
  endtask

  task automatic run_idle_cycles;
    int idx;
    begin
      wait_idle("idle setup");
      in_valid  = 1'b0;
      a_data    = '0;
      b_data    = '0;
      out_ready = 1'b1;
      for (idx = 0; idx < 5; idx++) begin
        @(posedge clk);
        #1;
        if (out_valid !== 1'b0) begin
          record_error(NAME, "idle cycle produced out_valid");
        end
        if ((cache_hit !== 1'b0) || (cache_miss !== 1'b0)) begin
          record_error(NAME, "idle cycle asserted cache status");
        end
      end
    end
  endtask

  task automatic run_back_to_back;
    int idx;
    begin
      out_ready = 1'b1;
      @(negedge clk);
      while (!in_ready) begin
        @(negedge clk);
      end
      in_valid = 1'b1;
      for (idx = 0; idx < 6; idx++) begin
        a_data = pattern_matrix(20 + idx);
        b_data = pattern_matrix(40 + idx);
        @(posedge clk);
        if (!(rst_n && in_valid && in_ready)) begin
          record_error(NAME, $sformatf("back-to-back transaction %0d was not accepted", idx));
        end
        @(negedge clk);
      end
      in_valid = 1'b0;
      a_data   = '0;
      b_data   = '0;
      wait_idle("back-to-back traffic");
    end
  endtask

  task automatic run_backpressure;
    logic [A_BITS-1:0] a_flat;
    logic [A_BITS-1:0] b_flat;
    int idx;
    begin
      log_check("backpressure stable output/status");
      a_flat = pattern_matrix(70);
      b_flat = pattern_matrix(71);
      out_ready = 1'b0;
      send_one(a_flat, b_flat, "output backpressure setup");
      for (idx = 0; idx < 4; idx++) begin
        @(posedge clk);
        #1;
        if (out_valid !== 1'b1) begin
          record_error(NAME, "backpressured output was not held valid");
        end
      end
      @(negedge clk);
      out_ready = 1'b1;
      wait_idle("output backpressure");
    end
  endtask

  task automatic run_simultaneous_consume_accept;
    logic [A_BITS-1:0] old_a;
    logic [A_BITS-1:0] old_b;
    logic [A_BITS-1:0] new_a;
    logic [A_BITS-1:0] new_b;
    begin
      old_a = pattern_matrix(80);
      old_b = pattern_matrix(81);
      new_a = pattern_matrix(82);
      new_b = pattern_matrix(83);

      out_ready = 1'b0;
      send_one(old_a, old_b, "simultaneous consume/accept old");
      @(negedge clk);
      if (out_valid !== 1'b1) begin
        record_error(NAME, "old output was not valid before simultaneous consume/accept");
      end

      a_data    = new_a;
      b_data    = new_b;
      in_valid  = 1'b1;
      out_ready = 1'b1;

      @(posedge clk);
      if (!(rst_n && in_valid && in_ready)) begin
        record_error(NAME, "new input was not accepted during simultaneous consume/accept");
      end

      @(negedge clk);
      in_valid = 1'b0;
      a_data   = '0;
      b_data   = '0;
      wait_idle("simultaneous consume/accept");
    end
  endtask

  task automatic run_reset_while_valid;
    begin
      log_check("reset while active cleared output");
      out_ready = 1'b0;
      send_one(pattern_matrix(90), pattern_matrix(91), "reset while valid setup");
      @(negedge clk);
      if (out_valid !== 1'b1) begin
        record_error(NAME, "output was not valid before reset assertion");
      end
      rst_n = 1'b0;
      in_valid = 1'b0;
      a_data = '0;
      b_data = '0;
      repeat (2) @(posedge clk);
      @(negedge clk);
      rst_n = 1'b1;
      out_ready = 1'b1;
      repeat (2) @(posedge clk);
      wait_idle("reset while output valid");
    end
  endtask

  task automatic run_unaccepted_input_no_cache_mutation;
    logic [A_BITS-1:0] stalled_a;
    logic [A_BITS-1:0] stalled_b;
    logic [A_BITS-1:0] probe_a;
    logic [A_BITS-1:0] probe_b;
    int idx;
    begin
      log_check("unaccepted input did not mutate cache");
      reset_fixture();
      stalled_a = pattern_matrix(100);
      stalled_b = pattern_matrix(101);
      probe_a   = pattern_matrix(102);
      probe_b   = pattern_matrix(103);

      out_ready = 1'b0;
      send_one(stalled_a, stalled_b, "unaccepted input stall setup");
      @(negedge clk);
      in_valid = 1'b1;
      a_data   = probe_a;
      b_data   = probe_b;
      for (idx = 0; idx < 3; idx++) begin
        @(posedge clk);
        if (in_ready !== 1'b0) begin
          record_error(NAME, "input was accepted while output was stalled");
        end
        @(negedge clk);
      end
      in_valid = 1'b0;
      a_data   = '0;
      b_data   = '0;
      out_ready = 1'b1;
      wait_idle("unaccepted input stall drain");

      send_and_wait(probe_a, probe_b, "probe after unaccepted input should miss");
      send_and_wait(probe_a, probe_b, "probe repeat should hit");
    end
  endtask

  task automatic run_ready_valid_sampled_data;
    logic [A_BITS-1:0] accepted_a;
    logic [A_BITS-1:0] accepted_b;
    logic [A_BITS-1:0] poison_a;
    logic [A_BITS-1:0] poison_b;
    begin
      log_check("ready-valid sampled accepted data only");
      wait_idle("sampled data setup");
      accepted_a = pattern_matrix(104);
      accepted_b = pattern_matrix(105);
      poison_a   = pattern_matrix(106);
      poison_b   = pattern_matrix(107);

      @(negedge clk);
      while (!in_ready) begin
        @(negedge clk);
      end
      a_data    = accepted_a;
      b_data    = accepted_b;
      in_valid  = 1'b1;
      out_ready = 1'b1;

      @(posedge clk);
      if (!(rst_n && in_valid && in_ready)) begin
        record_error(NAME, "sampled-data input was not accepted");
      end
      #1;
      a_data = poison_a;
      b_data = poison_b;

      @(negedge clk);
      in_valid = 1'b0;
      a_data   = '0;
      b_data   = '0;
      wait_idle("ready-valid sampled data");
    end
  endtask

  task automatic run_default_arithmetic;
    logic [A_BITS-1:0] max_matrix;
    logic [A_BITS-1:0] nonsym_a;
    logic [A_BITS-1:0] nonsym_b;
    logic [C_BITS-1:0] nonsym_expected;
    logic [C_BITS-1:0] all_max_expected;
    begin
      log_check("default16 non-symmetric multiply");
      nonsym_a = make_matrix(16'h0123, 16'h0456, 16'h0789, 16'h0abc);
      nonsym_b = make_matrix(16'h0100, 16'h7fff, 16'h8000, 16'hffff);
      nonsym_expected = make_result(33'd36446976, 33'd82279047,
                                    33'd90540288, 33'd243297723);
      if (scoreboard.ref_matmul(nonsym_a, nonsym_b) !== nonsym_expected) begin
        record_error(NAME, "hand-known non-symmetric reference result mismatch");
      end
      send_and_wait(nonsym_a, nonsym_b, "16-bit non-symmetric matrix");
      send_and_wait(make_matrix(16'h0000, 16'h0000, 16'h0000, 16'h0000),
                    pattern_matrix(7),
                    "zero matrix times non-zero matrix");
      send_and_wait(pattern_matrix(8),
                    make_matrix(16'h0000, 16'h0000, 16'h0000, 16'h0000),
                    "non-zero matrix times zero matrix");
      log_check("default16 unsigned high-bit multiply");
      send_and_wait(make_matrix(16'h0000, 16'h0001, 16'h00ff, 16'h0100),
                    make_matrix(16'h7fff, 16'h8000, 16'hffff, 16'h0001),
                    "16-bit boundary mix");
      send_and_wait(pattern_matrix(9),
                    identity_matrix(),
                    "identity matrix");
      log_check("default16 all-max result 8589672450");
      max_matrix = all_value_matrix(16'hffff);
      all_max_expected = make_result(33'd8589672450, 33'd8589672450,
                                     33'd8589672450, 33'd8589672450);
      if (scoreboard.ref_matmul(max_matrix, max_matrix) !== all_max_expected) begin
        record_error(NAME, "all-max 16-bit reference result mismatch");
      end
      send_and_wait(max_matrix, max_matrix, "all max 16-bit 2x2 expects 8589672450 per element");
    end
  endtask

  task automatic run_default_cache;
    logic [A_BITS-1:0] key0_a;
    logic [A_BITS-1:0] key0_b;
    logic [A_BITS-1:0] key1_a;
    logic [A_BITS-1:0] key1_b;
    logic [A_BITS-1:0] key2_a;
    logic [A_BITS-1:0] key2_b;
    logic [A_BITS-1:0] key3_a;
    logic [A_BITS-1:0] key3_b;
    logic [A_BITS-1:0] key4_a;
    logic [A_BITS-1:0] key4_b;
    begin
      reset_fixture();
      key0_a = pattern_matrix(110);
      key0_b = pattern_matrix(111);
      key1_a = key0_a;
      key1_b = pattern_matrix(112);
      key2_a = pattern_matrix(113);
      key2_b = key0_b;
      key3_a = pattern_matrix(114);
      key3_b = pattern_matrix(115);
      key4_a = pattern_matrix(116);
      key4_b = pattern_matrix(117);

      log_check("cache first unique miss");
      send_and_wait(key0_a, key0_b, "first unique transaction miss");
      log_check("cache repeated resident hit");
      send_and_wait(key0_a, key0_b, "repeated transaction hit");
      log_check("cache alias same-A-different-B miss");
      send_and_wait(key1_a, key1_b, "same A different B miss");
      log_check("cache alias same-B-different-A miss");
      send_and_wait(key2_a, key2_b, "same B different A miss");

      reset_fixture();
      key0_a = pattern_matrix(120);
      key0_b = pattern_matrix(121);
      key1_a = pattern_matrix(122);
      key1_b = pattern_matrix(123);
      key2_a = pattern_matrix(124);
      key2_b = pattern_matrix(125);
      key3_a = pattern_matrix(126);
      key3_b = pattern_matrix(127);
      key4_a = pattern_matrix(128);
      key4_b = pattern_matrix(129);

      send_and_wait(key0_a, key0_b, "replacement key0 fill");
      send_and_wait(key1_a, key1_b, "replacement key1 fill");
      send_and_wait(key0_a, key0_b, "replacement key0 hit must not advance pointer");
      send_and_wait(key2_a, key2_b, "replacement key2 fill");
      send_and_wait(key3_a, key3_b, "replacement key3 fill");
      send_and_wait(key0_a, key0_b, "replacement key0 still resident after hit");
      send_and_wait(key4_a, key4_b, "replacement key4 evicts key0");
      send_and_wait(key0_a, key0_b, "replacement key0 miss after eviction");

      reset_fixture();
      log_check("reset invalidation");
      send_and_wait(key0_a, key0_b, "cache fill before reset invalidation");
      send_and_wait(key0_a, key0_b, "cache hit before reset invalidation");
      reset_fixture();
      send_and_wait(key0_a, key0_b, "cache miss after reset invalidation");
    end
  endtask

  task automatic run_default_mode;
    begin
      reset_fixture();
      run_no_accept_during_reset();
      reset_fixture();
      run_idle_cycles();
      run_default_arithmetic();
      run_back_to_back();
      run_backpressure();
      run_ready_valid_sampled_data();
      run_simultaneous_consume_accept();
      run_reset_while_valid();
      run_default_cache();
      run_unaccepted_input_no_cache_mutation();
      wait_idle("default final drain");
    end
  endtask

  task automatic run_no_cache_mode;
    logic [A_BITS-1:0] a_flat;
    logic [A_BITS-1:0] b_flat;
    begin
      reset_fixture();
      log_check("cache disabled status zero");
      a_flat = pattern_matrix(200);
      b_flat = pattern_matrix(201);
      send_and_wait(a_flat, b_flat, "cache disabled first transaction");
      send_and_wait(a_flat, b_flat, "cache disabled repeat must not hit");
      send_and_wait(pattern_matrix(202), pattern_matrix(203), "cache disabled unique transaction");
      run_backpressure();
      wait_idle("cache disabled final drain");
    end
  endtask

  task automatic run_depth2_mode;
    logic [A_BITS-1:0] a0;
    logic [A_BITS-1:0] b0;
    logic [A_BITS-1:0] a1;
    logic [A_BITS-1:0] b1;
    logic [A_BITS-1:0] a2;
    logic [A_BITS-1:0] b2;
    begin
      reset_fixture();
      log_check("cache depth2 eviction sequence");
      a0 = pattern_matrix(250);
      b0 = pattern_matrix(251);
      a1 = pattern_matrix(252);
      b1 = pattern_matrix(253);
      a2 = pattern_matrix(254);
      b2 = pattern_matrix(255);

      send_and_wait(a0, b0, "CACHE_DEPTH=2 key A miss");
      send_and_wait(a1, b1, "CACHE_DEPTH=2 key B miss");
      send_and_wait(a0, b0, "CACHE_DEPTH=2 key A hit must not advance pointer");
      send_and_wait(a2, b2, "CACHE_DEPTH=2 key C miss evicts A");
      send_and_wait(a1, b1, "CACHE_DEPTH=2 key B remains resident after A hit");
      send_and_wait(a0, b0, "CACHE_DEPTH=2 key A miss after eviction");
      send_and_wait(a1, b1, "CACHE_DEPTH=2 key B miss after later eviction");
      wait_idle("CACHE_DEPTH=2 final drain");
    end
  endtask

  task automatic run_depth1_mode;
    logic [A_BITS-1:0] a0;
    logic [A_BITS-1:0] b0;
    logic [A_BITS-1:0] a1;
    logic [A_BITS-1:0] b1;
    begin
      reset_fixture();
      log_check("cache depth1 eviction sequence");
      a0 = pattern_matrix(300);
      b0 = pattern_matrix(301);
      a1 = pattern_matrix(302);
      b1 = pattern_matrix(303);
      send_and_wait(a0, b0, "CACHE_DEPTH=1 first key miss");
      send_and_wait(a0, b0, "CACHE_DEPTH=1 same key hit");
      send_and_wait(a1, b1, "CACHE_DEPTH=1 alternating key miss");
      send_and_wait(a0, b0, "CACHE_DEPTH=1 original key evicted miss");
      wait_idle("CACHE_DEPTH=1 final drain");
    end
  endtask

  task automatic run_dim1_mode;
    begin
      reset_fixture();
      send_and_wait(make_matrix(16'd7, '0, '0, '0),
                    make_matrix(16'd9, '0, '0, '0),
                    "MAT_DIM=1 basic multiply");
      log_check("MAT_DIM=1 scalar all-max");
      send_and_wait(make_matrix(16'hffff, '0, '0, '0),
                    make_matrix(16'hffff, '0, '0, '0),
                    "MAT_DIM=1 all max multiply expects 4294836225");
      send_and_wait(make_matrix(16'd7, '0, '0, '0),
                    make_matrix(16'd9, '0, '0, '0),
                    "MAT_DIM=1 repeated transaction hit");
      wait_idle("MAT_DIM=1 final drain");
    end
  endtask

  initial begin
    rst_n     = 1'b0;
    in_valid  = 1'b0;
    a_data    = '0;
    b_data    = '0;
    out_ready = 1'b0;
`ifdef TENSOR_UNIT_HAS_DFT_PORTS
    dft_mode    = 1'b0;
    scan_enable = 1'b0;
    scan_in     = 1'b0;
`endif

    case (MODE)
      MODE_DEFAULT:  run_default_mode();
      MODE_NO_CACHE: run_no_cache_mode();
      MODE_DEPTH2:   run_depth2_mode();
      MODE_DEPTH1:   run_depth1_mode();
      MODE_DIM1:     run_dim1_mode();
      default:       record_error(NAME, $sformatf("unknown fixture mode %0d", MODE));
    endcase

    mark_fixture_done(NAME);
  end
endmodule

module tensor_unit_legacy8_fixture;
  import tensor_unit_tb_pkg::*;

  localparam int DATA_WIDTH = 8;
  localparam int MAT_DIM    = 2;
  localparam int ACC_WIDTH  = (2 * DATA_WIDTH) + $clog2(MAT_DIM);
  localparam int MAT_ELEMS  = MAT_DIM * MAT_DIM;
  localparam int A_BITS     = MAT_ELEMS * DATA_WIDTH;
  localparam int C_BITS     = MAT_ELEMS * ACC_WIDTH;

  logic clk;
  logic rst_n;
  logic in_valid;
  logic in_ready;
  logic [A_BITS-1:0] a_data;
  logic [A_BITS-1:0] b_data;
  logic out_valid;
  logic out_ready;
  logic [C_BITS-1:0] c_data;
`ifdef TENSOR_UNIT_HAS_DFT_PORTS
  logic dft_mode;
  logic scan_enable;
  logic scan_in;
  logic scan_out;
`endif

  tensor_unit_8bit dut (
    .clk       (clk),
    .rst_n     (rst_n),
    .in_valid  (in_valid),
    .in_ready  (in_ready),
    .a_data    (a_data),
    .b_data    (b_data),
    .out_valid (out_valid),
    .out_ready (out_ready),
    .c_data    (c_data)
`ifdef TENSOR_UNIT_HAS_DFT_PORTS
    ,
    .dft_mode    (dft_mode),
    .scan_enable (scan_enable),
    .scan_in     (scan_in),
    .scan_out    (scan_out)
`endif
  );

  tensor_unit_scoreboard #(
    .DATA_WIDTH         (DATA_WIDTH),
    .MAT_DIM            (MAT_DIM),
    .ACC_WIDTH          (ACC_WIDTH),
    .ENABLE_CACHE       (1'b0),
    .CACHE_DEPTH        (1),
    .CHECK_CACHE_STATUS (1'b0),
    .NAME               ("legacy8")
  ) scoreboard (
    .clk        (clk),
    .rst_n      (rst_n),
    .in_valid   (in_valid),
    .in_ready   (in_ready),
    .a_data     (a_data),
    .b_data     (b_data),
    .out_valid  (out_valid),
    .out_ready  (out_ready),
    .c_data     (c_data),
    .cache_hit  (1'b0),
    .cache_miss (1'b0)
  );

  initial begin
    clk = 1'b0;
    forever #5 clk = ~clk;
  end

  function automatic logic [A_BITS-1:0] pack4(
    input logic [DATA_WIDTH-1:0] e0,
    input logic [DATA_WIDTH-1:0] e1,
    input logic [DATA_WIDTH-1:0] e2,
    input logic [DATA_WIDTH-1:0] e3
  );
    logic [A_BITS-1:0] packed_matrix;
    begin
      packed_matrix = '0;
      packed_matrix[(0*DATA_WIDTH) +: DATA_WIDTH] = e0;
      packed_matrix[(1*DATA_WIDTH) +: DATA_WIDTH] = e1;
      packed_matrix[(2*DATA_WIDTH) +: DATA_WIDTH] = e2;
      packed_matrix[(3*DATA_WIDTH) +: DATA_WIDTH] = e3;
      return packed_matrix;
    end
  endfunction

  function automatic logic [A_BITS-1:0] pattern_matrix(input int unsigned seed);
    logic [A_BITS-1:0] packed_matrix;
    int unsigned value;
    int idx;
    begin
      packed_matrix = '0;
      for (idx = 0; idx < MAT_ELEMS; idx++) begin
        value = ((seed + 1) * 37) + (idx * 53) + (seed * idx * 11);
        packed_matrix[(idx*DATA_WIDTH) +: DATA_WIDTH] = value[DATA_WIDTH-1:0];
      end
      return packed_matrix;
    end
  endfunction

  task automatic reset_fixture;
    begin
      @(negedge clk);
      rst_n     = 1'b0;
      in_valid  = 1'b0;
      a_data    = '0;
      b_data    = '0;
      out_ready = 1'b0;
`ifdef TENSOR_UNIT_HAS_DFT_PORTS
      dft_mode    = 1'b0;
      scan_enable = 1'b0;
      scan_in     = 1'b0;
`endif
      repeat (3) @(posedge clk);
      @(negedge clk);
      rst_n     = 1'b1;
      out_ready = 1'b1;
      repeat (2) @(posedge clk);
    end
  endtask

  task automatic send_one(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat,
    input string check_name
  );
    begin
      @(negedge clk);
      while (!in_ready) begin
        @(negedge clk);
      end
      a_data   = a_flat;
      b_data   = b_flat;
      in_valid = 1'b1;
      @(posedge clk);
      if (!(rst_n && in_valid && in_ready)) begin
        record_error("legacy8", {check_name, ": input was not accepted"});
      end
      @(negedge clk);
      in_valid = 1'b0;
      a_data   = '0;
      b_data   = '0;
    end
  endtask

  task automatic wait_idle(input string check_name);
    int cycles;
    begin
      out_ready = 1'b1;
      cycles = 0;
      while (((scoreboard.pending_count() != 0) || out_valid) && (cycles < 100)) begin
        @(posedge clk);
        #1;
        cycles++;
      end
      if ((scoreboard.pending_count() != 0) || out_valid) begin
        record_error("legacy8", {check_name, ": timed out waiting for drain"});
      end
      scoreboard.check_empty(check_name);
    end
  endtask

  task automatic send_and_wait(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat,
    input string check_name
  );
    begin
      send_one(a_flat, b_flat, check_name);
      wait_idle(check_name);
    end
  endtask

  task automatic run_backpressure_smoke;
    begin
      out_ready = 1'b0;
      send_one(pack4(8'd2, 8'd4, 8'd6, 8'd8),
               pack4(8'd1, 8'd3, 8'd5, 8'd7),
               "legacy backpressure setup");
      repeat (3) begin
        @(posedge clk);
        #1;
        if (out_valid !== 1'b1) begin
          record_error("legacy8", "backpressured output was not held valid");
        end
      end
      @(negedge clk);
      out_ready = 1'b1;
      wait_idle("legacy backpressure smoke");
    end
  endtask

  initial begin
    rst_n     = 1'b0;
    in_valid  = 1'b0;
    a_data    = '0;
    b_data    = '0;
    out_ready = 1'b0;
`ifdef TENSOR_UNIT_HAS_DFT_PORTS
    dft_mode    = 1'b0;
    scan_enable = 1'b0;
    scan_in     = 1'b0;
`endif

    reset_fixture();
    send_and_wait(pack4(8'd1, 8'd2, 8'd3, 8'd4),
                  pack4(8'd5, 8'd6, 8'd7, 8'd8),
                  "legacy known matrix multiply");
    log_check("legacy8 all-255 result 130050");
    send_and_wait(pack4(8'd255, 8'd255, 8'd255, 8'd255),
                  pack4(8'd255, 8'd255, 8'd255, 8'd255),
                  "legacy all-255 2x2 expects 130050 per element");
    run_backpressure_smoke();
    send_and_wait(pattern_matrix(8), pattern_matrix(9), "legacy ready/valid smoke");
    wait_idle("legacy final drain");

    mark_fixture_done("legacy8");
  end
endmodule

`ifdef TENSOR_UNIT_HAS_DFT_PORTS
module tensor_unit_dft_fixture #(
  parameter bit USE_LEGACY8 = 1'b0,
  parameter string NAME = "dft",
  parameter int DATA_WIDTH = 16,
  parameter int MAT_DIM = 2,
  parameter int ACC_WIDTH = (2 * DATA_WIDTH) + $clog2(MAT_DIM),
  parameter bit ENABLE_CACHE = 1'b1,
  parameter int CACHE_DEPTH = 4
);
  import tensor_unit_tb_pkg::*;

  localparam int MAT_ELEMS = MAT_DIM * MAT_DIM;
  localparam int A_BITS = MAT_ELEMS * DATA_WIDTH;
  localparam int C_BITS = MAT_ELEMS * ACC_WIDTH;
  localparam int KEY_BITS = 2 * A_BITS;
  localparam int CACHE_SLOTS = (CACHE_DEPTH <= 0) ? 1 : CACHE_DEPTH;
  localparam int CACHE_INDEX_WIDTH = (CACHE_SLOTS <= 1) ? 1 : $clog2(CACHE_SLOTS);
  localparam bit CACHE_ACTIVE = ENABLE_CACHE && (CACHE_DEPTH > 0);
  localparam int SCAN_OUT_VALID_LSB = 0;
  localparam int SCAN_C_DATA_LSB = SCAN_OUT_VALID_LSB + 1;
  localparam int SCAN_CACHE_HIT_LSB = SCAN_C_DATA_LSB + C_BITS;
  localparam int SCAN_CACHE_MISS_LSB = SCAN_CACHE_HIT_LSB + 1;
  localparam int SCAN_CACHE_VALID_LSB = SCAN_CACHE_MISS_LSB + 1;
  localparam int SCAN_REPLACE_PTR_LSB = SCAN_CACHE_VALID_LSB + CACHE_SLOTS;
  localparam int SCAN_CACHE_KEY_LSB = SCAN_REPLACE_PTR_LSB + CACHE_INDEX_WIDTH;
  localparam int SCAN_CACHE_DATA_LSB = SCAN_CACHE_KEY_LSB + (CACHE_SLOTS * KEY_BITS);
  localparam int SCAN_STATE_WIDTH = SCAN_CACHE_DATA_LSB + (CACHE_SLOTS * C_BITS);

  localparam int SCENARIO_IDLE = 0;
  localparam int SCENARIO_INACTIVE_NOISE = 1;
  localparam int SCENARIO_HOLD_VISIBLE = 2;
  localparam int SCENARIO_HOLD_FULL_STATE = 3;
  localparam int SCENARIO_SCANOUT_VISIBLE = 4;
  localparam int SCENARIO_SCANOUT_CACHE = 5;
  localparam int SCENARIO_RESET_SHIFT = 6;
  localparam int SCENARIO_RESET_HOLD = 7;
  localparam int SCENARIO_ROUNDTRIP = 8;
  localparam int SCENARIO_DESTRUCTIVE = 9;
  localparam int SCENARIO_RECOVERY = 10;
  localparam int SCENARIO_INACTIVE_BASELINE = 11;
  localparam int SCENARIO_INACTIVE_MISS = 12;
  localparam int SCENARIO_INACTIVE_HIT = 13;
  localparam int SCENARIO_INACTIVE_BACKPRESSURE = 14;
  localparam int SCENARIO_INACTIVE_SIMULTANEOUS = 15;
  localparam int SCENARIO_SCANIN_VISIBLE = 16;

  logic clk;
  logic rst_n;
  logic in_valid;
  logic in_ready;
  logic [A_BITS-1:0] a_data;
  logic [A_BITS-1:0] b_data;
  logic out_valid;
  logic out_ready;
  logic [C_BITS-1:0] c_data;
  logic cache_hit;
  logic cache_miss;
  logic dft_mode;
  logic scan_enable;
  logic scan_in;
  logic scan_out;
  int unsigned dft_scenario;

  generate
    if (USE_LEGACY8) begin : gen_legacy8_dut
      tensor_unit_8bit #(
        .DATA_WIDTH   (DATA_WIDTH),
        .MAT_DIM      (MAT_DIM),
        .ACC_WIDTH    (ACC_WIDTH),
        .ENABLE_CACHE (ENABLE_CACHE),
        .CACHE_DEPTH  (CACHE_DEPTH)
      ) dut (
        .clk         (clk),
        .rst_n       (rst_n),
        .in_valid    (in_valid),
        .in_ready    (in_ready),
        .a_data      (a_data),
        .b_data      (b_data),
        .out_valid   (out_valid),
        .out_ready   (out_ready),
        .c_data      (c_data),
        .dft_mode    (dft_mode),
        .scan_enable (scan_enable),
        .scan_in     (scan_in),
        .scan_out    (scan_out)
      );

      assign cache_hit = 1'b0;
      assign cache_miss = 1'b0;
    end else begin : gen_generic_dut
      tensor_unit #(
        .DATA_WIDTH   (DATA_WIDTH),
        .MAT_DIM      (MAT_DIM),
        .ACC_WIDTH    (ACC_WIDTH),
        .ENABLE_CACHE (ENABLE_CACHE),
        .CACHE_DEPTH  (CACHE_DEPTH)
      ) dut (
        .clk         (clk),
        .rst_n       (rst_n),
        .in_valid    (in_valid),
        .in_ready    (in_ready),
        .a_data      (a_data),
        .b_data      (b_data),
        .out_valid   (out_valid),
        .out_ready   (out_ready),
        .c_data      (c_data),
        .cache_hit   (cache_hit),
        .cache_miss  (cache_miss),
        .dft_mode    (dft_mode),
        .scan_enable (scan_enable),
        .scan_in     (scan_in),
        .scan_out    (scan_out)
      );
    end
  endgenerate

  covergroup dft_cg @(posedge clk);
    option.per_instance = 1;
    cp_dft_mode: coverpoint dft_mode iff (rst_n) {
      bins functional = {0};
      bins dft_active = {1};
    }
    cp_scan_enable: coverpoint scan_enable iff (rst_n && dft_mode) {
      bins hold = {0};
      bins shift = {1};
    }
    cp_dft_state: coverpoint {dft_mode, scan_enable} iff (rst_n) {
      bins functional = {2'b00, 2'b01};
      bins hold = {2'b10};
      bins shift = {2'b11};
    }
    cp_functional_scan_noise: coverpoint {scan_enable, scan_in} iff (rst_n && !dft_mode) {
      bins quiet = {2'b00};
      bins noisy[] = {2'b01, 2'b10, 2'b11};
    }
    cp_functional_scan_out: coverpoint scan_out iff (rst_n && !dft_mode) {
      bins forced_low = {0};
    }
    cp_reset_scan_out: coverpoint scan_out iff (!rst_n) {
      bins forced_low = {0};
    }
    cp_suppressed_accept: coverpoint (rst_n && dft_mode && in_valid && !in_ready) {
      bins observed = {1};
    }
    cp_reset_during_scan: coverpoint (!rst_n && dft_mode && scan_enable) {
      bins observed = {1};
    }
    cp_scan_out: coverpoint scan_out iff (rst_n && dft_mode && scan_enable) {
      bins zero = {0};
      bins one = {1};
    }
    cp_dft_scenario: coverpoint dft_scenario iff (rst_n) {
      bins inactive_noise = {SCENARIO_INACTIVE_NOISE};
      bins hold_visible = {SCENARIO_HOLD_VISIBLE};
      bins hold_full_state = {SCENARIO_HOLD_FULL_STATE};
      bins scanout_visible = {SCENARIO_SCANOUT_VISIBLE};
      bins scanout_cache = {SCENARIO_SCANOUT_CACHE};
      bins reset_shift = {SCENARIO_RESET_SHIFT};
      bins reset_hold = {SCENARIO_RESET_HOLD};
      bins roundtrip = {SCENARIO_ROUNDTRIP};
      bins destructive = {SCENARIO_DESTRUCTIVE};
      bins recovery = {SCENARIO_RECOVERY};
      bins inactive_baseline = {SCENARIO_INACTIVE_BASELINE};
      bins inactive_miss = {SCENARIO_INACTIVE_MISS};
      bins inactive_hit = {SCENARIO_INACTIVE_HIT};
      bins inactive_backpressure = {SCENARIO_INACTIVE_BACKPRESSURE};
      bins inactive_simultaneous = {SCENARIO_INACTIVE_SIMULTANEOUS};
      bins scanin_visible = {SCENARIO_SCANIN_VISIBLE};
    }
  endgroup

  dft_cg cov = new();

  property p_dft_suppresses_functional_ready;
    @(posedge clk) disable iff (!rst_n)
      dft_mode |-> (in_ready === 1'b0);
  endproperty

  assert property (p_dft_suppresses_functional_ready)
    else record_error(NAME, "DFT mode did not suppress functional input readiness");

  property p_scan_out_low_in_functional;
    @(posedge clk) disable iff (!rst_n)
      !dft_mode |-> (scan_out === 1'b0);
  endproperty

  assert property (p_scan_out_low_in_functional)
    else record_error(NAME, "scan_out was not forced low in functional mode");

  property p_scan_out_low_in_reset;
    @(posedge clk)
      !rst_n |-> (scan_out === 1'b0);
  endproperty

  assert property (p_scan_out_low_in_reset)
    else record_error(NAME, "scan_out was not forced low during reset");

  property p_hold_mode_preserves_visible_state;
    @(posedge clk) disable iff (!rst_n)
      (dft_mode && !scan_enable && $past(rst_n && dft_mode && !scan_enable)) |->
        ((out_valid === $past(out_valid)) && (c_data === $past(c_data)));
  endproperty

  assert property (p_hold_mode_preserves_visible_state)
    else record_error(NAME, "DFT hold mode did not preserve visible output state");

  initial begin
    clk = 1'b0;
    forever #5 clk = ~clk;
  end

  function automatic logic [C_BITS-1:0] ref_matmul(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat
  );
    logic [C_BITS-1:0] result;
    logic [ACC_WIDTH-1:0] acc;
    logic [ACC_WIDTH-1:0] a_ext;
    logic [ACC_WIDTH-1:0] b_ext;
    int row;
    int col;
    int k;
    int a_index;
    int b_index;
    int c_index;
    begin
      result = '0;
      for (row = 0; row < MAT_DIM; row++) begin
        for (col = 0; col < MAT_DIM; col++) begin
          acc = '0;
          for (k = 0; k < MAT_DIM; k++) begin
            a_index = (row * MAT_DIM) + k;
            b_index = (k * MAT_DIM) + col;
            a_ext = '0;
            b_ext = '0;
            a_ext[DATA_WIDTH-1:0] = a_flat[(a_index*DATA_WIDTH) +: DATA_WIDTH];
            b_ext[DATA_WIDTH-1:0] = b_flat[(b_index*DATA_WIDTH) +: DATA_WIDTH];
            acc = acc + (a_ext * b_ext);
          end
          c_index = (row * MAT_DIM) + col;
          result[(c_index*ACC_WIDTH) +: ACC_WIDTH] = acc;
        end
      end
      return result;
    end
  endfunction

  function automatic logic [A_BITS-1:0] pattern_matrix(input int unsigned seed);
    logic [A_BITS-1:0] packed_matrix;
    longint unsigned value;
    int idx;
    begin
      packed_matrix = '0;
      for (idx = 0; idx < MAT_ELEMS; idx++) begin
        value = (64'd41 * (seed + 3)) + (64'd73 * idx) + (64'd19 * seed * (idx + 1));
        packed_matrix[(idx*DATA_WIDTH) +: DATA_WIDTH] = value[DATA_WIDTH-1:0];
      end
      return packed_matrix;
    end
  endfunction

  function automatic logic scan_pattern_bit(input int idx);
    return (((idx * 7) + (idx / 3) + 1) & 1) == 1;
  endfunction

  task automatic toggle_inactive_scan_noise;
    begin
      scan_enable = ~scan_enable;
      scan_in = scan_enable ^ scan_in;
    end
  endtask

  task automatic drive_inactive_scan_noise(input int unsigned phase);
    begin
      dft_mode = 1'b0;
      scan_enable = phase[0];
      scan_in = phase[1] ^ phase[2];
    end
  endtask

  task automatic check_functional_scan_out_low(input string check_name);
    begin
      if (scan_out !== 1'b0) begin
        record_error(NAME, {check_name, ": scan_out was not forced low with dft_mode=0"});
      end
    end
  endtask

  task automatic check_reset_scan_out_low(input string check_name);
    begin
      if (scan_out !== 1'b0) begin
        record_error(NAME, {check_name, ": scan_out was not forced low during reset"});
      end
    end
  endtask

  task automatic check_functional_ready(input string check_name);
    logic expected_ready;
    begin
      expected_ready = (!out_valid || out_ready);
      if (in_ready !== expected_ready) begin
        record_error(NAME, $sformatf("%s: in_ready expected=%0b actual=%0b",
                                     check_name, expected_ready, in_ready));
      end
    end
  endtask

  task automatic check_cache_status(
    input bit expected_hit,
    input bit expected_miss,
    input string check_name
  );
    begin
      if ((cache_hit !== expected_hit) || (cache_miss !== expected_miss)) begin
        record_error(NAME, $sformatf("%s: cache status expected hit=%0b miss=%0b actual hit=%0b miss=%0b",
                                     check_name, expected_hit, expected_miss, cache_hit, cache_miss));
      end
    end
  endtask

  task automatic reset_functional(input string check_name);
    begin
      @(negedge clk);
      rst_n       = 1'b0;
      in_valid    = 1'b0;
      a_data      = '0;
      b_data      = '0;
      out_ready   = 1'b0;
      dft_mode    = 1'b0;
      scan_enable = 1'b0;
      scan_in     = 1'b0;
      dft_scenario = SCENARIO_IDLE;
      repeat (3) @(posedge clk);
      #1;
      check_reset_scan_out_low({check_name, ": reset"});
      if (out_valid !== 1'b0) begin
        record_error(NAME, {check_name, ": reset did not clear out_valid"});
      end
      if (c_data !== '0) begin
        record_error(NAME, {check_name, ": reset did not clear c_data"});
      end
      if (!USE_LEGACY8 && ((cache_hit !== 1'b0) || (cache_miss !== 1'b0))) begin
        record_error(NAME, {check_name, ": reset did not clear cache status"});
      end
      @(negedge clk);
      rst_n = 1'b1;
      out_ready = 1'b1;
      repeat (2) @(posedge clk);
    end
  endtask

  task automatic reset_during_active_scan;
    begin
      dft_scenario = SCENARIO_RESET_SHIFT;
      @(negedge clk);
      rst_n       = 1'b0;
      dft_mode    = 1'b1;
      scan_enable = 1'b1;
      scan_in     = 1'b1;
      in_valid    = 1'b1;
      a_data      = pattern_matrix(900);
      b_data      = pattern_matrix(901);
      out_ready   = 1'b1;
      repeat (3) @(posedge clk);
      #1;
      check_reset_scan_out_low("reset priority active scan");
      if (out_valid !== 1'b0) begin
        record_error(NAME, "reset priority failed to clear out_valid during active scan");
      end
      if (c_data !== '0) begin
        record_error(NAME, "reset priority failed to clear c_data during active scan");
      end
      if (!USE_LEGACY8 && ((cache_hit !== 1'b0) || (cache_miss !== 1'b0))) begin
        record_error(NAME, "reset priority failed to clear cache status during active scan");
      end
      @(negedge clk);
      rst_n       = 1'b1;
      scan_in     = 1'b0;
      in_valid    = 1'b0;
      a_data      = '0;
      b_data      = '0;
      out_ready   = 1'b1;
    end
  endtask

  task automatic reset_during_scan_hold;
    begin
      dft_scenario = SCENARIO_RESET_HOLD;
      @(negedge clk);
      rst_n       = 1'b0;
      dft_mode    = 1'b1;
      scan_enable = 1'b0;
      scan_in     = 1'b1;
      in_valid    = 1'b1;
      a_data      = pattern_matrix(910);
      b_data      = pattern_matrix(911);
      out_ready   = 1'b1;
      repeat (3) @(posedge clk);
      #1;
      check_reset_scan_out_low("reset priority scan hold");
      if (out_valid !== 1'b0) begin
        record_error(NAME, "reset priority failed to clear out_valid during scan hold");
      end
      if (c_data !== '0) begin
        record_error(NAME, "reset priority failed to clear c_data during scan hold");
      end
      if (!USE_LEGACY8 && ((cache_hit !== 1'b0) || (cache_miss !== 1'b0))) begin
        record_error(NAME, "reset priority failed to clear cache status during scan hold");
      end
      @(negedge clk);
      rst_n       = 1'b1;
      scan_in     = 1'b0;
      in_valid    = 1'b0;
      a_data      = '0;
      b_data      = '0;
      out_ready   = 1'b1;
    end
  endtask

  task automatic drive_functional_and_expect(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat,
    input string check_name,
    input bit inject_inactive_noise
  );
    logic [C_BITS-1:0] expected;
    int wait_cycles;
    begin
      expected = ref_matmul(a_flat, b_flat);
      dft_mode = 1'b0;
      out_ready = 1'b1;

      @(negedge clk);
      wait_cycles = 0;
      while (!in_ready && (wait_cycles < 20)) begin
        if (inject_inactive_noise) begin
          toggle_inactive_scan_noise();
        end else begin
          scan_enable = 1'b0;
          scan_in = 1'b0;
        end
        wait_cycles++;
        #1;
        check_functional_scan_out_low({check_name, ": waiting for ready"});
        check_functional_ready({check_name, ": waiting for ready"});
        @(negedge clk);
      end
      if (!in_ready) begin
        record_error(NAME, {check_name, ": timed out waiting for in_ready"});
      end
      check_functional_scan_out_low({check_name, ": before accept"});
      check_functional_ready({check_name, ": before accept"});

      a_data = a_flat;
      b_data = b_flat;
      in_valid = 1'b1;
      if (inject_inactive_noise) begin
        toggle_inactive_scan_noise();
      end else begin
        scan_enable = 1'b0;
        scan_in = 1'b0;
      end

      @(posedge clk);
      if (!(rst_n && in_valid && in_ready)) begin
        record_error(NAME, {check_name, ": functional input was not accepted"});
      end
      #1;
      check_functional_scan_out_low({check_name, ": accepted output"});
      check_functional_ready({check_name, ": accepted output"});
      if (out_valid !== 1'b1) begin
        record_error(NAME, {check_name, ": functional output was not valid after accept"});
      end
      if (c_data !== expected) begin
        record_error(NAME, $sformatf("%s: functional result mismatch expected=0x%0h actual=0x%0h",
                                     check_name, expected, c_data));
      end

      @(negedge clk);
      in_valid = 1'b0;
      a_data = '0;
      b_data = '0;
      if (inject_inactive_noise) begin
        toggle_inactive_scan_noise();
      end
      @(posedge clk);
      #1;
      check_functional_scan_out_low({check_name, ": drain"});
      check_functional_ready({check_name, ": drain"});
      if (out_valid !== 1'b0) begin
        record_error(NAME, {check_name, ": output did not drain after consume"});
      end
    end
  endtask

  task automatic run_idle_noise_window(
    input string check_name,
    input int unsigned scenario
  );
    int idx;
    begin
      dft_scenario = scenario;
      in_valid = 1'b0;
      a_data = '0;
      b_data = '0;
      out_ready = 1'b1;
      for (idx = 0; idx < 6; idx++) begin
        @(negedge clk);
        drive_inactive_scan_noise(idx + 1);
        @(posedge clk);
        #1;
        check_functional_scan_out_low({check_name, ": idle scan noise"});
        check_functional_ready({check_name, ": idle scan noise"});
        if (out_valid !== 1'b0) begin
          record_error(NAME, {check_name, ": idle functional scan noise produced out_valid"});
        end
        check_cache_status(1'b0, 1'b0, {check_name, ": idle scan noise"});
      end
    end
  endtask

  task automatic drive_noisy_transaction_status(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat,
    input string check_name,
    input bit expected_hit,
    input bit expected_miss,
    input int unsigned scenario
  );
    logic [C_BITS-1:0] expected;
    bit sampled_ready;
    int idx;
    int wait_cycles;
    begin
      expected = ref_matmul(a_flat, b_flat);
      dft_scenario = scenario;
      dft_mode = 1'b0;
      out_ready = 1'b1;

      for (idx = 0; idx < 3; idx++) begin
        @(negedge clk);
        drive_inactive_scan_noise(idx + 3);
        @(posedge clk);
        #1;
        check_functional_scan_out_low({check_name, ": pre-transaction noise"});
        check_functional_ready({check_name, ": pre-transaction noise"});
      end

      @(negedge clk);
      wait_cycles = 0;
      while (!in_ready && (wait_cycles < 20)) begin
        drive_inactive_scan_noise(wait_cycles + 9);
        wait_cycles++;
        @(negedge clk);
      end
      if (!in_ready) begin
        record_error(NAME, {check_name, ": timed out waiting for in_ready"});
      end

      a_data = a_flat;
      b_data = b_flat;
      in_valid = 1'b1;
      drive_inactive_scan_noise(17);
      #1;
      sampled_ready = in_ready;
      check_functional_scan_out_low({check_name, ": accept setup"});
      check_functional_ready({check_name, ": accept setup"});

      @(posedge clk);
      if (!(rst_n && in_valid && sampled_ready)) begin
        record_error(NAME, {check_name, ": noisy functional input was not accepted"});
      end
      #1;
      check_functional_scan_out_low({check_name, ": accepted"});
      check_functional_ready({check_name, ": accepted"});
      if (out_valid !== 1'b1) begin
        record_error(NAME, {check_name, ": noisy functional output was not valid"});
      end
      if (c_data !== expected) begin
        record_error(NAME, $sformatf("%s: noisy functional result mismatch expected=0x%0h actual=0x%0h",
                                     check_name, expected, c_data));
      end
      check_cache_status(expected_hit, expected_miss, {check_name, ": accepted"});

      @(negedge clk);
      in_valid = 1'b0;
      a_data = '0;
      b_data = '0;
      drive_inactive_scan_noise(23);
      @(posedge clk);
      #1;
      check_functional_scan_out_low({check_name, ": post-transaction noise"});
      check_functional_ready({check_name, ": post-transaction noise"});
      if (out_valid !== 1'b0) begin
        record_error(NAME, {check_name, ": noisy functional output did not drain"});
      end
      check_cache_status(1'b0, 1'b0, {check_name, ": post-transaction noise"});
    end
  endtask

  task automatic check_inactive_dft_noise_cache_paths;
    logic [A_BITS-1:0] a_flat;
    logic [A_BITS-1:0] b_flat;
    begin
      a_flat = pattern_matrix(20);
      b_flat = pattern_matrix(21);

      reset_functional("inactive DFT clean baseline reset");
      dft_scenario = SCENARIO_INACTIVE_BASELINE;
      drive_functional_and_expect(a_flat, b_flat,
                                  "inactive scan noise clean baseline transaction 0", 1'b0);
      if (CACHE_ACTIVE) begin
        drive_functional_and_expect(a_flat, b_flat,
                                    "inactive scan noise clean baseline cache hit", 1'b0);
      end

      reset_functional("inactive DFT noise cache-path reset");
      run_idle_noise_window("inactive scan noise before transactions", SCENARIO_INACTIVE_NOISE);
      if (CACHE_ACTIVE) begin
        drive_noisy_transaction_status(a_flat, b_flat,
                                       "inactive scan noise cache miss", 1'b0, 1'b1,
                                       SCENARIO_INACTIVE_MISS);
        drive_noisy_transaction_status(a_flat, b_flat,
                                       "inactive scan noise cache hit", 1'b1, 1'b0,
                                       SCENARIO_INACTIVE_HIT);
      end else begin
        $display("INFO[%s]: cache hit/miss inactive DFT-noise paths not applicable when cache is disabled", NAME);
        drive_noisy_transaction_status(a_flat, b_flat,
                                       "inactive scan noise cache-disabled transaction", 1'b0, 1'b0,
                                       SCENARIO_INACTIVE_MISS);
        drive_noisy_transaction_status(a_flat, b_flat,
                                       "inactive scan noise cache-disabled repeat", 1'b0, 1'b0,
                                       SCENARIO_INACTIVE_HIT);
      end
    end
  endtask

  task automatic check_inactive_dft_noise_backpressure;
    logic [A_BITS-1:0] a_flat;
    logic [A_BITS-1:0] b_flat;
    logic [A_BITS-1:0] poison_a;
    logic [A_BITS-1:0] poison_b;
    logic [C_BITS-1:0] expected;
    bit expected_miss;
    bit sampled_ready;
    int idx;
    begin
      log_check({NAME, " inactive DFT noise output backpressure"});
      reset_functional("inactive DFT noise backpressure reset");
      dft_scenario = SCENARIO_INACTIVE_BACKPRESSURE;
      a_flat = pattern_matrix(50);
      b_flat = pattern_matrix(51);
      poison_a = pattern_matrix(52);
      poison_b = pattern_matrix(53);
      expected = ref_matmul(a_flat, b_flat);
      expected_miss = CACHE_ACTIVE;

      @(negedge clk);
      out_ready = 1'b0;
      a_data = a_flat;
      b_data = b_flat;
      in_valid = 1'b1;
      drive_inactive_scan_noise(31);
      #1;
      sampled_ready = in_ready;
      check_functional_scan_out_low("inactive scan noise backpressure setup");
      check_functional_ready("inactive scan noise backpressure setup");

      @(posedge clk);
      if (!(rst_n && in_valid && sampled_ready)) begin
        record_error(NAME, "inactive scan noise backpressure setup was not accepted");
      end
      #1;
      check_functional_scan_out_low("inactive scan noise backpressure accepted");
      if (out_valid !== 1'b1) begin
        record_error(NAME, "inactive scan noise backpressure did not create a pending output");
      end
      if (c_data !== expected) begin
        record_error(NAME, $sformatf("inactive scan noise backpressure result mismatch expected=0x%0h actual=0x%0h",
                                     expected, c_data));
      end
      check_cache_status(1'b0, expected_miss, "inactive scan noise backpressure accepted");

      for (idx = 0; idx < 4; idx++) begin
        @(negedge clk);
        in_valid = 1'b1;
        a_data = poison_a;
        b_data = poison_b;
        out_ready = 1'b0;
        drive_inactive_scan_noise(40 + idx);
        @(posedge clk);
        #1;
        check_functional_scan_out_low("inactive scan noise backpressure hold");
        check_functional_ready("inactive scan noise backpressure hold");
        if (in_ready !== 1'b0) begin
          record_error(NAME, "inactive scan noise backpressure allowed an input accept");
        end
        if (out_valid !== 1'b1) begin
          record_error(NAME, "inactive scan noise backpressure dropped out_valid");
        end
        if (c_data !== expected) begin
          record_error(NAME, "inactive scan noise backpressure changed c_data");
        end
        check_cache_status(1'b0, expected_miss, "inactive scan noise backpressure hold");
      end

      @(negedge clk);
      in_valid = 1'b0;
      a_data = '0;
      b_data = '0;
      out_ready = 1'b1;
      drive_inactive_scan_noise(49);
      @(posedge clk);
      #1;
      check_functional_scan_out_low("inactive scan noise backpressure release");
      check_functional_ready("inactive scan noise backpressure release");
      if (out_valid !== 1'b0) begin
        record_error(NAME, "inactive scan noise backpressure output did not drain");
      end
      check_cache_status(1'b0, 1'b0, "inactive scan noise backpressure release");
    end
  endtask

  task automatic check_inactive_dft_noise_simultaneous;
    logic [A_BITS-1:0] old_a;
    logic [A_BITS-1:0] old_b;
    logic [A_BITS-1:0] new_a;
    logic [A_BITS-1:0] new_b;
    logic [C_BITS-1:0] old_expected;
    logic [C_BITS-1:0] new_expected;
    bit expected_miss;
    bit old_sampled_ready;
    bit new_sampled_ready;
    begin
      log_check({NAME, " inactive DFT noise simultaneous consume/accept"});
      reset_functional("inactive DFT noise simultaneous reset");
      dft_scenario = SCENARIO_INACTIVE_SIMULTANEOUS;
      old_a = pattern_matrix(60);
      old_b = pattern_matrix(61);
      new_a = pattern_matrix(62);
      new_b = pattern_matrix(63);
      old_expected = ref_matmul(old_a, old_b);
      new_expected = ref_matmul(new_a, new_b);
      expected_miss = CACHE_ACTIVE;

      @(negedge clk);
      out_ready = 1'b0;
      a_data = old_a;
      b_data = old_b;
      in_valid = 1'b1;
      drive_inactive_scan_noise(57);
      #1;
      old_sampled_ready = in_ready;
      check_functional_scan_out_low("inactive scan noise simultaneous old setup");
      check_functional_ready("inactive scan noise simultaneous old setup");

      @(posedge clk);
      if (!(rst_n && in_valid && old_sampled_ready)) begin
        record_error(NAME, "inactive scan noise simultaneous old input was not accepted");
      end
      #1;
      check_functional_scan_out_low("inactive scan noise simultaneous old accepted");
      if (out_valid !== 1'b1) begin
        record_error(NAME, "inactive scan noise simultaneous old output was not pending");
      end
      if (c_data !== old_expected) begin
        record_error(NAME, "inactive scan noise simultaneous old c_data mismatch");
      end
      check_cache_status(1'b0, expected_miss, "inactive scan noise simultaneous old accepted");

      @(negedge clk);
      a_data = new_a;
      b_data = new_b;
      in_valid = 1'b1;
      out_ready = 1'b1;
      drive_inactive_scan_noise(61);
      #1;
      new_sampled_ready = in_ready;
      check_functional_scan_out_low("inactive scan noise simultaneous accept setup");
      check_functional_ready("inactive scan noise simultaneous accept setup");

      @(posedge clk);
      if (!(rst_n && in_valid && new_sampled_ready)) begin
        record_error(NAME, "inactive scan noise simultaneous new input was not accepted");
      end
      #1;
      check_functional_scan_out_low("inactive scan noise simultaneous new accepted");
      check_functional_ready("inactive scan noise simultaneous new accepted");
      if (out_valid !== 1'b1) begin
        record_error(NAME, "inactive scan noise simultaneous new output was not valid");
      end
      if (c_data !== new_expected) begin
        record_error(NAME, $sformatf("inactive scan noise simultaneous new result mismatch expected=0x%0h actual=0x%0h",
                                     new_expected, c_data));
      end
      check_cache_status(1'b0, expected_miss, "inactive scan noise simultaneous new accepted");

      @(negedge clk);
      in_valid = 1'b0;
      a_data = '0;
      b_data = '0;
      out_ready = 1'b1;
      drive_inactive_scan_noise(67);
      @(posedge clk);
      #1;
      check_functional_scan_out_low("inactive scan noise simultaneous drain");
      check_functional_ready("inactive scan noise simultaneous drain");
      if (out_valid !== 1'b0) begin
        record_error(NAME, "inactive scan noise simultaneous output did not drain");
      end
      check_cache_status(1'b0, 1'b0, "inactive scan noise simultaneous drain");
    end
  endtask

  task automatic scan_cycle(input logic si, output logic so);
    begin
      @(negedge clk);
      rst_n       = 1'b1;
      dft_mode    = 1'b1;
      scan_enable = 1'b1;
      scan_in     = si;
      in_valid    = 1'b1;
      a_data      = pattern_matrix(1000 + (si ? 1 : 0));
      b_data      = pattern_matrix(1100 + (si ? 1 : 0));
      out_ready   = si;
      #1;
      so = scan_out;
      @(posedge clk);
      #1;
      if (in_ready !== 1'b0) begin
        record_error(NAME, "active scan did not suppress functional input readiness");
      end
    end
  endtask

  task automatic build_expected_scan_state(
    input logic visible_valid,
    input logic [C_BITS-1:0] visible_c,
    input logic visible_hit,
    input logic visible_miss,
    input logic [CACHE_SLOTS-1:0] expected_valid,
    input logic [CACHE_INDEX_WIDTH-1:0] expected_ptr,
    input logic [KEY_BITS-1:0] expected_key [CACHE_SLOTS],
    input logic [C_BITS-1:0] expected_data [CACHE_SLOTS],
    output logic [SCAN_STATE_WIDTH-1:0] expected_state
  );
    int entry;
    begin
      expected_state = '0;
      expected_state[SCAN_OUT_VALID_LSB] = visible_valid;
      expected_state[SCAN_C_DATA_LSB +: C_BITS] = visible_c;
      expected_state[SCAN_CACHE_HIT_LSB] = visible_hit;
      expected_state[SCAN_CACHE_MISS_LSB] = visible_miss;
      expected_state[SCAN_CACHE_VALID_LSB +: CACHE_SLOTS] = expected_valid;
      expected_state[SCAN_REPLACE_PTR_LSB +: CACHE_INDEX_WIDTH] = expected_ptr;
      for (entry = 0; entry < CACHE_SLOTS; entry++) begin
        expected_state[(SCAN_CACHE_KEY_LSB + (entry * KEY_BITS)) +: KEY_BITS] = expected_key[entry];
        expected_state[(SCAN_CACHE_DATA_LSB + (entry * C_BITS)) +: C_BITS] = expected_data[entry];
      end
    end
  endtask

  task automatic model_cache_fill(
    input logic [A_BITS-1:0] a_flat,
    input logic [A_BITS-1:0] b_flat,
    inout logic [CACHE_SLOTS-1:0] expected_valid,
    inout logic [CACHE_INDEX_WIDTH-1:0] expected_ptr,
    inout logic [KEY_BITS-1:0] expected_key [CACHE_SLOTS],
    inout logic [C_BITS-1:0] expected_data [CACHE_SLOTS]
  );
    begin
      expected_valid[expected_ptr] = 1'b1;
      expected_key[expected_ptr] = {a_flat, b_flat};
      expected_data[expected_ptr] = ref_matmul(a_flat, b_flat);
      if (expected_ptr == (CACHE_SLOTS - 1)) begin
        expected_ptr = '0;
      end else begin
        expected_ptr = expected_ptr + 1'b1;
      end
    end
  endtask

  task automatic create_known_functional_scan_state(
    input string check_name,
    output logic [SCAN_STATE_WIDTH-1:0] expected_state
  );
    logic [KEY_BITS-1:0] expected_key [CACHE_SLOTS];
    logic [C_BITS-1:0] expected_data [CACHE_SLOTS];
    logic [CACHE_SLOTS-1:0] expected_valid;
    logic [CACHE_INDEX_WIDTH-1:0] expected_ptr;
    logic [A_BITS-1:0] a0;
    logic [A_BITS-1:0] b0;
    logic [A_BITS-1:0] a1;
    logic [A_BITS-1:0] b1;
    logic [A_BITS-1:0] final_a;
    logic [A_BITS-1:0] final_b;
    logic [C_BITS-1:0] final_c;
    logic final_hit;
    logic final_miss;
    int entry;
    int wait_cycles;
    begin
      reset_functional({check_name, " reset"});
      expected_valid = '0;
      expected_ptr = '0;
      for (entry = 0; entry < CACHE_SLOTS; entry++) begin
        expected_key[entry] = '0;
        expected_data[entry] = '0;
      end

      a0 = pattern_matrix(40);
      b0 = pattern_matrix(41);
      a1 = pattern_matrix(42);
      b1 = pattern_matrix(43);
      final_a = pattern_matrix(44);
      final_b = pattern_matrix(45);
      final_hit = 1'b0;
      final_miss = 1'b0;

      if (CACHE_ACTIVE) begin
        drive_functional_and_expect(a0, b0, {check_name, " cache fill entry0"}, 1'b0);
        model_cache_fill(a0, b0, expected_valid, expected_ptr, expected_key, expected_data);
        if (CACHE_SLOTS > 1) begin
          drive_functional_and_expect(a1, b1, {check_name, " cache fill entry1"}, 1'b0);
          model_cache_fill(a1, b1, expected_valid, expected_ptr, expected_key, expected_data);
        end
        final_a = a0;
        final_b = b0;
        final_c = expected_data[0];
        final_hit = 1'b1;
      end else begin
        final_c = ref_matmul(final_a, final_b);
      end

      @(negedge clk);
      dft_mode = 1'b0;
      scan_enable = 1'b0;
      scan_in = 1'b0;
      out_ready = 1'b0;
      wait_cycles = 0;
      while (!in_ready && (wait_cycles < 20)) begin
        wait_cycles++;
        @(negedge clk);
      end
      if (!in_ready) begin
        record_error(NAME, {check_name, ": timed out waiting for known-state accept"});
      end

      a_data = final_a;
      b_data = final_b;
      in_valid = 1'b1;
      @(posedge clk);
      if (!(rst_n && in_valid && in_ready)) begin
        record_error(NAME, {check_name, ": known-state input was not accepted"});
      end
      #1;
      if (out_valid !== 1'b1) begin
        record_error(NAME, {check_name, ": known-state output was not pending"});
      end
      if (c_data !== final_c) begin
        record_error(NAME, $sformatf("%s: known-state c_data mismatch expected=0x%0h actual=0x%0h",
                                     check_name, final_c, c_data));
      end
      if (!USE_LEGACY8) begin
        if (cache_hit !== final_hit) begin
          record_error(NAME, $sformatf("%s: known-state cache_hit expected=%0b actual=%0b",
                                       check_name, final_hit, cache_hit));
        end
        if (cache_miss !== final_miss) begin
          record_error(NAME, $sformatf("%s: known-state cache_miss expected=%0b actual=%0b",
                                       check_name, final_miss, cache_miss));
        end
      end

      @(negedge clk);
      in_valid = 1'b0;
      a_data = '0;
      b_data = '0;
      build_expected_scan_state(1'b1, final_c, final_hit, final_miss,
                                expected_valid, expected_ptr, expected_key,
                                expected_data, expected_state);
    end
  endtask

  task automatic scan_out_expected_state(
    input logic [SCAN_STATE_WIDTH-1:0] expected_state,
    input string check_name,
    input int unsigned scenario
  );
    logic observed;
    int idx;
    begin
      dft_scenario = scenario;
      for (idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        scan_cycle(1'b0, observed);
        if (observed !== expected_state[idx]) begin
          record_error(NAME, $sformatf("%s scan bit %0d expected %0b observed %0b",
                                       check_name, idx, expected_state[idx], observed));
        end
      end
      reset_functional({check_name, " destructive scan cleanup"});
    end
  endtask

  task automatic check_reset_scan_observability;
    logic observed;
    int idx;
    begin
      log_check({NAME, " scan reset priority/observability"});
      reset_during_active_scan();
      for (idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        scan_cycle(1'b0, observed);
        if (observed !== 1'b0) begin
          record_error(NAME, $sformatf("reset scan bit %0d expected 0 observed %0b", idx, observed));
        end
      end
    end
  endtask

  task automatic check_reset_hold_observability;
    logic [SCAN_STATE_WIDTH-1:0] expected_state;
    logic observed;
    int idx;
    begin
      log_check({NAME, " scan hold reset priority/observability"});
      create_known_functional_scan_state("reset hold preload", expected_state);
      reset_during_scan_hold();
      for (idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        scan_cycle(1'b0, observed);
        if (observed !== 1'b0) begin
          record_error(NAME, $sformatf("reset-hold scan bit %0d expected 0 observed %0b", idx, observed));
        end
      end
      reset_functional("reset hold cleanup");
    end
  endtask

  task automatic check_scan_roundtrip;
    logic [SCAN_STATE_WIDTH-1:0] pattern;
    logic observed;
    int idx;
    begin
      log_check({NAME, " scan controllability round-trip"});
      reset_functional("scan roundtrip reset");
      dft_mode = 1'b1;
      scan_enable = 1'b1;
      dft_scenario = SCENARIO_DESTRUCTIVE;
      for (idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        pattern[idx] = scan_pattern_bit(idx);
        scan_cycle(pattern[idx], observed);
      end
      dft_scenario = SCENARIO_ROUNDTRIP;
      for (idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        scan_cycle(1'b0, observed);
        if (observed !== pattern[idx]) begin
          record_error(NAME, $sformatf("scan round-trip bit %0d expected %0b observed %0b",
                                       idx, pattern[idx], observed));
        end
      end
    end
  endtask

  task automatic check_scanin_controls_visible_state;
    logic [SCAN_STATE_WIDTH-1:0] target_state;
    logic [KEY_BITS-1:0] target_key [CACHE_SLOTS];
    logic [C_BITS-1:0] target_data [CACHE_SLOTS];
    logic [CACHE_SLOTS-1:0] target_valid;
    logic [CACHE_INDEX_WIDTH-1:0] target_ptr;
    logic [A_BITS-1:0] a_flat;
    logic [A_BITS-1:0] b_flat;
    logic [C_BITS-1:0] target_c;
    logic observed;
    int entry;
    int idx;
    begin
      log_check({NAME, " scan-in controls visible state"});
      reset_functional("scan-in visible controllability reset");

      a_flat = pattern_matrix(70);
      b_flat = pattern_matrix(71);
      target_c = ref_matmul(a_flat, b_flat);
      target_valid = '0;
      target_ptr = '0;
      for (entry = 0; entry < CACHE_SLOTS; entry++) begin
        target_key[entry] = {pattern_matrix(80 + entry), pattern_matrix(90 + entry)};
        target_data[entry] = ref_matmul(pattern_matrix(80 + entry), pattern_matrix(90 + entry));
        target_valid[entry] = CACHE_ACTIVE;
      end
      if (CACHE_SLOTS > 1) begin
        target_ptr = CACHE_SLOTS - 1;
      end

      build_expected_scan_state(1'b1, target_c, 1'b1, 1'b0,
                                target_valid, target_ptr, target_key,
                                target_data, target_state);

      dft_scenario = SCENARIO_SCANIN_VISIBLE;
      for (idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        scan_cycle(target_state[idx], observed);
      end

      @(negedge clk);
      dft_mode = 1'b1;
      scan_enable = 1'b0;
      scan_in = 1'b1;
      in_valid = 1'b1;
      a_data = pattern_matrix(72);
      b_data = pattern_matrix(73);
      out_ready = 1'b1;

      repeat (3) begin
        @(posedge clk);
        #1;
        if (in_ready !== 1'b0) begin
          record_error(NAME, "scan-loaded hold did not suppress functional input readiness");
        end
        if (scan_out !== target_state[SCAN_OUT_VALID_LSB]) begin
          record_error(NAME, $sformatf("scan-loaded hold tail expected %0b observed %0b",
                                       target_state[SCAN_OUT_VALID_LSB], scan_out));
        end
        if (out_valid !== target_state[SCAN_OUT_VALID_LSB]) begin
          record_error(NAME, "scan-loaded hold did not control out_valid");
        end
        if (c_data !== target_c) begin
          record_error(NAME, $sformatf("scan-loaded hold did not control c_data expected=0x%0h actual=0x%0h",
                                       target_c, c_data));
        end
        if (!USE_LEGACY8) begin
          if (cache_hit !== target_state[SCAN_CACHE_HIT_LSB]) begin
            record_error(NAME, "scan-loaded hold did not control cache_hit");
          end
          if (cache_miss !== target_state[SCAN_CACHE_MISS_LSB]) begin
            record_error(NAME, "scan-loaded hold did not control cache_miss");
          end
        end
      end

      reset_functional("scan-in visible controllability cleanup");
    end
  endtask

  task automatic check_hold_mode;
    logic [A_BITS-1:0] a_flat;
    logic [A_BITS-1:0] b_flat;
    logic [C_BITS-1:0] held_c;
    logic held_valid;
    int idx;
    begin
      log_check({NAME, " DFT hold preserves pending output"});
      reset_functional("hold mode reset");
      dft_scenario = SCENARIO_HOLD_VISIBLE;
      a_flat = pattern_matrix(10);
      b_flat = pattern_matrix(11);

      @(negedge clk);
      out_ready = 1'b0;
      a_data = a_flat;
      b_data = b_flat;
      in_valid = 1'b1;
      @(posedge clk);
      if (!(rst_n && in_valid && in_ready)) begin
        record_error(NAME, "hold setup transaction was not accepted");
      end
      #1;
      if (out_valid !== 1'b1) begin
        record_error(NAME, "hold setup did not create a pending output");
      end
      held_valid = out_valid;
      held_c = c_data;

      @(negedge clk);
      dft_mode = 1'b1;
      scan_enable = 1'b0;
      scan_in = 1'b1;
      in_valid = 1'b1;
      a_data = pattern_matrix(12);
      b_data = pattern_matrix(13);
      out_ready = 1'b1;

      for (idx = 0; idx < 5; idx++) begin
        @(posedge clk);
        #1;
        if (in_ready !== 1'b0) begin
          record_error(NAME, "hold mode did not suppress functional input readiness");
        end
        if (out_valid !== held_valid) begin
          record_error(NAME, "hold mode changed out_valid");
        end
        if (c_data !== held_c) begin
          record_error(NAME, "hold mode changed c_data");
        end
      end

      @(negedge clk);
      dft_mode = 1'b0;
      scan_enable = 1'b0;
      scan_in = 1'b0;
      in_valid = 1'b0;
      a_data = '0;
      b_data = '0;
      out_ready = 1'b1;
      @(posedge clk);
      #1;
      if (out_valid !== 1'b0) begin
        record_error(NAME, "held output did not drain after DFT mode deasserted");
      end
    end
  endtask

  task automatic check_hold_preserves_full_scan_state;
    logic [SCAN_STATE_WIDTH-1:0] expected_state;
    logic held_valid;
    logic [C_BITS-1:0] held_c;
    logic held_hit;
    logic held_miss;
    int idx;
    begin
      log_check({NAME, " DFT hold preserves full scanned state"});
      create_known_functional_scan_state("hold full-state preload", expected_state);
      held_valid = expected_state[SCAN_OUT_VALID_LSB];
      held_c = expected_state[SCAN_C_DATA_LSB +: C_BITS];
      held_hit = expected_state[SCAN_CACHE_HIT_LSB];
      held_miss = expected_state[SCAN_CACHE_MISS_LSB];

      @(negedge clk);
      dft_mode = 1'b1;
      scan_enable = 1'b0;
      scan_in = 1'b1;
      in_valid = 1'b1;
      a_data = pattern_matrix(46);
      b_data = pattern_matrix(47);
      out_ready = 1'b1;
      dft_scenario = SCENARIO_HOLD_FULL_STATE;

      for (idx = 0; idx < 5; idx++) begin
        @(posedge clk);
        #1;
        if (in_ready !== 1'b0) begin
          record_error(NAME, "full-state hold did not suppress functional input readiness");
        end
        if (out_valid !== held_valid) begin
          record_error(NAME, "full-state hold changed out_valid");
        end
        if (c_data !== held_c) begin
          record_error(NAME, "full-state hold changed c_data");
        end
        if (!USE_LEGACY8 && ((cache_hit !== held_hit) || (cache_miss !== held_miss))) begin
          record_error(NAME, "full-state hold changed visible cache status");
        end
      end

      scan_out_expected_state(expected_state, "full-state hold", SCENARIO_HOLD_FULL_STATE);
    end
  endtask

  task automatic check_inactive_dft_noise;
    begin
      log_check({NAME, " inactive DFT noise functional equivalence"});
      check_inactive_dft_noise_cache_paths();
      check_inactive_dft_noise_backpressure();
      check_inactive_dft_noise_simultaneous();
    end
  endtask

  task automatic check_functional_state_scanout;
    logic [SCAN_STATE_WIDTH-1:0] expected_state;
    begin
      log_check({NAME, " independent functional-state scan-out"});
      create_known_functional_scan_state("functional-state scanout", expected_state);
      scan_out_expected_state(expected_state, "functional-state scanout",
                              CACHE_ACTIVE ? SCENARIO_SCANOUT_CACHE : SCENARIO_SCANOUT_VISIBLE);
    end
  endtask

  task automatic check_post_scan_recovery;
    begin
      log_check({NAME, " post-scan reset recovery"});
      reset_functional("post-scan recovery reset");
      dft_scenario = SCENARIO_RECOVERY;
      drive_functional_and_expect(pattern_matrix(30), pattern_matrix(31),
                                  "post-scan functional transaction", 1'b0);
    end
  endtask

  initial begin
    rst_n       = 1'b0;
    in_valid    = 1'b0;
    a_data      = '0;
    b_data      = '0;
    out_ready   = 1'b0;
    dft_mode    = 1'b0;
    scan_enable = 1'b0;
    scan_in     = 1'b0;
    dft_scenario = SCENARIO_IDLE;

    check_inactive_dft_noise();
    check_hold_mode();
    check_hold_preserves_full_scan_state();
    check_functional_state_scanout();
    check_reset_scan_observability();
    check_reset_hold_observability();
    check_scan_roundtrip();
    check_scanin_controls_visible_state();
    check_post_scan_recovery();

    mark_fixture_done(NAME);
  end
endmodule
`endif

module tensor_unit_dft_static_checks;
  import tensor_unit_tb_pkg::*;

  localparam bit [7:0] SEQ_OUT_VALID    = 8'h01;
  localparam bit [7:0] SEQ_C_DATA       = 8'h02;
  localparam bit [7:0] SEQ_CACHE_HIT    = 8'h04;
  localparam bit [7:0] SEQ_CACHE_MISS   = 8'h08;
  localparam bit [7:0] SEQ_CACHE_VALID  = 8'h10;
  localparam bit [7:0] SEQ_REPLACE_PTR  = 8'h20;
  localparam bit [7:0] SEQ_CACHE_KEY    = 8'h40;
  localparam bit [7:0] SEQ_CACHE_DATA   = 8'h80;
  localparam bit [7:0] SEQ_EXPECTED_ALL = 8'hff;

  function automatic bit str_contains(input string haystack, input string needle);
    int hlen;
    int nlen;
    int idx;
    begin
      hlen = haystack.len();
      nlen = needle.len();
      if (nlen == 0) begin
        return 1'b1;
      end
      if (hlen < nlen) begin
        return 1'b0;
      end
      for (idx = 0; idx <= (hlen - nlen); idx++) begin
        if (haystack.substr(idx, idx + nlen - 1) == needle) begin
          return 1'b1;
        end
      end
      return 1'b0;
    end
  endfunction

  function automatic int str_find(input string haystack, input string needle);
    int hlen;
    int nlen;
    int idx;
    begin
      hlen = haystack.len();
      nlen = needle.len();
      if ((nlen == 0) || (hlen < nlen)) begin
        return -1;
      end
      for (idx = 0; idx <= (hlen - nlen); idx++) begin
        if (haystack.substr(idx, idx + nlen - 1) == needle) begin
          return idx;
        end
      end
      return -1;
    end
  endfunction

  function automatic string append_char(input string text, input byte ch);
    string result;
    begin
      result = {text, " "};
      result.putc(result.len() - 1, ch);
      return result;
    end
  endfunction

  function automatic string strip_line_comment(input string line);
    int comment_idx;
    begin
      comment_idx = str_find(line, "//");
      if (comment_idx < 0) begin
        return line;
      end
      if (comment_idx == 0) begin
        return "";
      end
      return line.substr(0, comment_idx - 1);
    end
  endfunction

  function automatic string compact(input string line);
    string result;
    byte ch;
    int idx;
    begin
      result = "";
      for (idx = 0; idx < line.len(); idx++) begin
        ch = line.getc(idx);
        if ((ch != 8'd32) && (ch != 8'd9) && (ch != 8'd10) && (ch != 8'd13)) begin
          result = append_char(result, ch);
        end
      end
      return result;
    end
  endfunction

  function automatic string lhs_base(input string compact_line);
    int assign_idx;
    int idx;
    string lhs;
    begin
      assign_idx = str_find(compact_line, "<=");
      if (assign_idx <= 0) begin
        return "";
      end
      lhs = compact_line.substr(0, assign_idx - 1);
      for (idx = 0; idx < lhs.len(); idx++) begin
        if (lhs.getc(idx) == "[") begin
          if (idx == 0) begin
            return "";
          end
          return lhs.substr(0, idx - 1);
        end
      end
      return lhs;
    end
  endfunction

  task automatic observe_sequential_lhs(
    input string path,
    input int line_no,
    input string compact_line,
    inout bit [7:0] observed_seq
  );
    string base;
    begin
      base = lhs_base(compact_line);
      if (base == "") begin
        return;
      end
      if (base == "out_valid") begin
        observed_seq |= SEQ_OUT_VALID;
      end else if (base == "c_data") begin
        observed_seq |= SEQ_C_DATA;
      end else if (base == "cache_hit") begin
        observed_seq |= SEQ_CACHE_HIT;
      end else if (base == "cache_miss") begin
        observed_seq |= SEQ_CACHE_MISS;
      end else if (base == "cache_valid_q") begin
        observed_seq |= SEQ_CACHE_VALID;
      end else if (base == "replace_ptr_q") begin
        observed_seq |= SEQ_REPLACE_PTR;
      end else if (base == "cache_key_q") begin
        observed_seq |= SEQ_CACHE_KEY;
      end else if (base == "cache_data_q") begin
        observed_seq |= SEQ_CACHE_DATA;
      end else begin
        record_error("dft_static", $sformatf(
                     "%s:%0d unexpected sequential assignment target '%s'",
                     path, line_no, base));
      end
    end
  endtask

  task automatic require_seen(input bit condition, input string message);
    begin
      if (!condition) begin
        record_error("dft_static", message);
      end
    end
  endtask

  task automatic check_tensor_unit_scan_inventory(input string path);
    int fd;
    int line_no;
    int always_ff_count;
    bit in_always_ff;
    bit [7:0] observed_seq;
    bit pack_out_valid;
    bit pack_c_data;
    bit pack_cache_hit;
    bit pack_cache_miss;
    bit pack_cache_valid;
    bit pack_replace_ptr;
    bit pack_cache_key_lsb;
    bit pack_cache_key_source;
    bit pack_cache_data_lsb;
    bit pack_cache_data_source;
    bit shift_expression;
    bit shift_out_valid;
    bit shift_c_data;
    bit shift_cache_hit;
    bit shift_cache_miss;
    bit shift_cache_valid;
    bit shift_replace_ptr;
    bit shift_cache_key;
    bit shift_cache_key_rhs;
    bit shift_cache_data;
    bit shift_cache_data_rhs;
    string line;
    string clean;
    string cmp;
    begin
      fd = $fopen(path, "r");
      if (fd == 0) begin
        record_error("dft_static", {"could not open ", path, " for tensor_unit scan inventory check"});
        return;
      end

      line_no = 0;
      always_ff_count = 0;
      in_always_ff = 1'b0;
      observed_seq = '0;

      while ($fgets(line, fd)) begin
        line_no++;
        clean = strip_line_comment(line);
        cmp = compact(clean);

        if (str_contains(cmp, "always_ff")) begin
          always_ff_count++;
          in_always_ff = 1'b1;
        end
        if (str_contains(cmp, "always_latch")) begin
          record_error("dft_static", $sformatf("%s:%0d unexpected always_latch in tensor_unit", path, line_no));
        end
        if (in_always_ff && str_contains(cmp, "<=")) begin
          observe_sequential_lhs(path, line_no, cmp, observed_seq);
        end

        if (str_contains(cmp, "scan_state[SCAN_OUT_VALID_LSB]=out_valid")) pack_out_valid = 1'b1;
        if (str_contains(cmp, "scan_state[SCAN_C_DATA_LSB+:C_FLAT_BITS]=c_data")) pack_c_data = 1'b1;
        if (str_contains(cmp, "scan_state[SCAN_CACHE_HIT_LSB]=cache_hit")) pack_cache_hit = 1'b1;
        if (str_contains(cmp, "scan_state[SCAN_CACHE_MISS_LSB]=cache_miss")) pack_cache_miss = 1'b1;
        if (str_contains(cmp, "scan_state[SCAN_CACHE_VALID_LSB+:CACHE_SLOTS]=cache_valid_q")) pack_cache_valid = 1'b1;
        if (str_contains(cmp, "scan_state[SCAN_REPLACE_PTR_LSB+:CACHE_INDEX_WIDTH]=replace_ptr_q")) pack_replace_ptr = 1'b1;
        if (str_contains(cmp, "SCAN_CACHE_KEY_LSB") && str_contains(cmp, "CACHE_KEY_WIDTH")) pack_cache_key_lsb = 1'b1;
        if (str_contains(cmp, "cache_key_q[entry]")) pack_cache_key_source = 1'b1;
        if (str_contains(cmp, "SCAN_CACHE_DATA_LSB") && str_contains(cmp, "C_FLAT_BITS")) pack_cache_data_lsb = 1'b1;
        if (str_contains(cmp, "cache_data_q[entry]")) pack_cache_data_source = 1'b1;

        if (str_contains(cmp, "scan_state_shifted={scan_in,scan_state[SCAN_STATE_WIDTH-1:1]}")) shift_expression = 1'b1;
        if (str_contains(cmp, "out_valid<=scan_state_shifted[SCAN_OUT_VALID_LSB]")) shift_out_valid = 1'b1;
        if (str_contains(cmp, "c_data<=scan_state_shifted[SCAN_C_DATA_LSB+:C_FLAT_BITS]")) shift_c_data = 1'b1;
        if (str_contains(cmp, "cache_hit<=scan_state_shifted[SCAN_CACHE_HIT_LSB]")) shift_cache_hit = 1'b1;
        if (str_contains(cmp, "cache_miss<=scan_state_shifted[SCAN_CACHE_MISS_LSB]")) shift_cache_miss = 1'b1;
        if (str_contains(cmp, "cache_valid_q<=scan_state_shifted[SCAN_CACHE_VALID_LSB+:CACHE_SLOTS]")) shift_cache_valid = 1'b1;
        if (str_contains(cmp, "replace_ptr_q<=scan_state_shifted[SCAN_REPLACE_PTR_LSB+:CACHE_INDEX_WIDTH]")) shift_replace_ptr = 1'b1;
        if (str_contains(cmp, "cache_key_q[entry]<=")) shift_cache_key = 1'b1;
        if (str_contains(cmp, "scan_state_shifted[(SCAN_CACHE_KEY_LSB")) shift_cache_key_rhs = 1'b1;
        if (str_contains(cmp, "cache_data_q[entry]<=")) shift_cache_data = 1'b1;
        if (str_contains(cmp, "scan_state_shifted[(SCAN_CACHE_DATA_LSB")) shift_cache_data_rhs = 1'b1;
      end
      $fclose(fd);

      require_seen(always_ff_count == 1,
                   $sformatf("tensor_unit scan inventory expected exactly one always_ff, observed %0d",
                             always_ff_count));
      require_seen(observed_seq == SEQ_EXPECTED_ALL,
                   $sformatf("tensor_unit sequential inventory mismatch observed mask=0x%0h expected=0x%0h",
                             observed_seq, SEQ_EXPECTED_ALL));
      require_seen(pack_out_valid && pack_c_data && pack_cache_hit && pack_cache_miss &&
                   pack_cache_valid && pack_replace_ptr && pack_cache_key_lsb &&
                   pack_cache_key_source && pack_cache_data_lsb && pack_cache_data_source,
                   "tensor_unit scan packing inventory is missing one or more expected state fields");
      require_seen(shift_expression && shift_out_valid && shift_c_data && shift_cache_hit &&
                   shift_cache_miss && shift_cache_valid && shift_replace_ptr &&
                   shift_cache_key && shift_cache_key_rhs &&
                   shift_cache_data && shift_cache_data_rhs,
                   "tensor_unit scan shift inventory is missing one or more expected state fields");
      if ((always_ff_count == 1) && (observed_seq == SEQ_EXPECTED_ALL) &&
          pack_out_valid && pack_c_data && pack_cache_hit && pack_cache_miss &&
          pack_cache_valid && pack_replace_ptr && pack_cache_key_lsb &&
          pack_cache_key_source && pack_cache_data_lsb && pack_cache_data_source &&
          shift_expression && shift_out_valid && shift_c_data && shift_cache_hit &&
          shift_cache_miss && shift_cache_valid && shift_replace_ptr &&
          shift_cache_key && shift_cache_key_rhs &&
          shift_cache_data && shift_cache_data_rhs) begin
        $display("INFO[dft_static]: tensor_unit scan-inventory check passed");
      end
    end
  endtask

  task automatic check_tensor_unit_8bit_wrapper(input string path);
    int fd;
    int line_no;
    bit saw_inner;
    bit saw_dft_mode;
    bit saw_scan_enable;
    bit saw_scan_in;
    bit saw_scan_out;
    string line;
    string clean;
    string cmp;
    begin
      fd = $fopen(path, "r");
      if (fd == 0) begin
        record_error("dft_static", {"could not open ", path, " for tensor_unit_8bit structural check"});
        return;
      end

      line_no = 0;
      while ($fgets(line, fd)) begin
        line_no++;
        clean = strip_line_comment(line);
        cmp = compact(clean);

        if (str_contains(cmp, "always_ff") || str_contains(cmp, "always@") ||
            str_contains(cmp, "always_latch")) begin
          record_error("dft_static", $sformatf(
                       "%s:%0d tensor_unit_8bit wrapper contains forbidden sequential process",
                       path, line_no));
        end
        if (str_contains(cmp, "initial")) begin
          record_error("dft_static", $sformatf(
                       "%s:%0d tensor_unit_8bit wrapper contains forbidden initial state",
                       path, line_no));
        end
        if ((str_contains(cmp, "logicscan_") || str_contains(cmp, "regscan_")) &&
            !str_contains(cmp, "inputlogicscan_") &&
            !str_contains(cmp, "outputlogicscan_")) begin
          record_error("dft_static", $sformatf(
                       "%s:%0d tensor_unit_8bit wrapper declares local scan state",
                       path, line_no));
        end
        if (str_contains(cmp, "assignscan_out") || str_contains(cmp, "scan_out<=")) begin
          record_error("dft_static", $sformatf(
                       "%s:%0d tensor_unit_8bit wrapper transforms scan_out instead of passing it through",
                       path, line_no));
        end

        if (str_contains(cmp, "tensor_unit#(")) saw_inner = 1'b1;
        if (str_contains(cmp, ".dft_mode(dft_mode)")) saw_dft_mode = 1'b1;
        if (str_contains(cmp, ".scan_enable(scan_enable)")) saw_scan_enable = 1'b1;
        if (str_contains(cmp, ".scan_in(scan_in)")) saw_scan_in = 1'b1;
        if (str_contains(cmp, ".scan_out(scan_out)")) saw_scan_out = 1'b1;
      end
      $fclose(fd);

      require_seen(saw_inner, "tensor_unit_8bit wrapper did not instantiate tensor_unit");
      require_seen(saw_dft_mode && saw_scan_enable && saw_scan_in && saw_scan_out,
                   "tensor_unit_8bit wrapper DFT pins are not direct pass-through connections");
      if (saw_inner && saw_dft_mode && saw_scan_enable && saw_scan_in && saw_scan_out) begin
        $display("INFO[dft_static]: tensor_unit_8bit wrapper structural check passed");
      end
    end
  endtask

  initial begin
    check_tensor_unit_scan_inventory("rtl/tensor_unit.sv");
    check_tensor_unit_8bit_wrapper("rtl/tensor_unit_8bit.sv");
  end
endmodule

module tb_tensor_unit;
  import tensor_unit_tb_pkg::*;

`ifdef TENSOR_UNIT_HAS_DFT_PORTS
  localparam int EXPECTED_FIXTURES = 12;
  localparam bit [FIXTURE_BITS-1:0] EXPECTED_FIXTURE_MASK = 12'hfff;
`else
  localparam int EXPECTED_FIXTURES = 6;
  localparam bit [FIXTURE_BITS-1:0] EXPECTED_FIXTURE_MASK = 12'h03f;
`endif
  string fsdb_file;

  tensor_unit_dft_static_checks dft_static_checks();

  tensor_unit_generic_fixture #(
    .MODE             (MODE_DEFAULT),
    .NAME             ("default16_cache"),
    .USE_DUT_DEFAULTS (1'b1),
    .DATA_WIDTH       (16),
    .MAT_DIM          (2),
    .ACC_WIDTH        (33),
    .ENABLE_CACHE     (1'b1),
    .CACHE_DEPTH      (4)
  ) default16_cache_fixture();

  tensor_unit_generic_fixture #(
    .MODE             (MODE_NO_CACHE),
    .NAME             ("cache_disabled"),
    .USE_DUT_DEFAULTS (1'b0),
    .DATA_WIDTH       (16),
    .MAT_DIM          (2),
    .ACC_WIDTH        (33),
    .ENABLE_CACHE     (1'b0),
    .CACHE_DEPTH      (4)
  ) cache_disabled_fixture();

  tensor_unit_generic_fixture #(
    .MODE             (MODE_DEPTH2),
    .NAME             ("cache_depth2"),
    .USE_DUT_DEFAULTS (1'b0),
    .DATA_WIDTH       (16),
    .MAT_DIM          (2),
    .ACC_WIDTH        (33),
    .ENABLE_CACHE     (1'b1),
    .CACHE_DEPTH      (2)
  ) cache_depth2_fixture();

  tensor_unit_generic_fixture #(
    .MODE             (MODE_DEPTH1),
    .NAME             ("cache_depth1"),
    .USE_DUT_DEFAULTS (1'b0),
    .DATA_WIDTH       (16),
    .MAT_DIM          (2),
    .ACC_WIDTH        (33),
    .ENABLE_CACHE     (1'b1),
    .CACHE_DEPTH      (1)
  ) cache_depth1_fixture();

  tensor_unit_generic_fixture #(
    .MODE             (MODE_DIM1),
    .NAME             ("mat_dim1"),
    .USE_DUT_DEFAULTS (1'b0),
    .DATA_WIDTH       (16),
    .MAT_DIM          (1),
    .ACC_WIDTH        (32),
    .ENABLE_CACHE     (1'b1),
    .CACHE_DEPTH      (2)
  ) mat_dim1_fixture();

  tensor_unit_legacy8_fixture legacy8_fixture();

`ifdef TENSOR_UNIT_HAS_DFT_PORTS
  tensor_unit_dft_fixture #(
    .USE_LEGACY8  (1'b0),
    .NAME         ("generic_dft"),
    .DATA_WIDTH   (16),
    .MAT_DIM      (2),
    .ACC_WIDTH    (33),
    .ENABLE_CACHE (1'b1),
    .CACHE_DEPTH  (4)
  ) generic_dft_fixture();

  tensor_unit_dft_fixture #(
    .USE_LEGACY8  (1'b1),
    .NAME         ("legacy8_dft"),
    .DATA_WIDTH   (8),
    .MAT_DIM      (2),
    .ACC_WIDTH    (17),
    .ENABLE_CACHE (1'b0),
    .CACHE_DEPTH  (4)
  ) legacy8_dft_fixture();

  tensor_unit_dft_fixture #(
    .USE_LEGACY8  (1'b0),
    .NAME         ("cache_disabled_dft"),
    .DATA_WIDTH   (16),
    .MAT_DIM      (2),
    .ACC_WIDTH    (33),
    .ENABLE_CACHE (1'b0),
    .CACHE_DEPTH  (4)
  ) cache_disabled_dft_fixture();

  tensor_unit_dft_fixture #(
    .USE_LEGACY8  (1'b0),
    .NAME         ("cache_depth1_dft"),
    .DATA_WIDTH   (16),
    .MAT_DIM      (2),
    .ACC_WIDTH    (33),
    .ENABLE_CACHE (1'b1),
    .CACHE_DEPTH  (1)
  ) cache_depth1_dft_fixture();

  tensor_unit_dft_fixture #(
    .USE_LEGACY8  (1'b0),
    .NAME         ("cache_depth2_dft"),
    .DATA_WIDTH   (16),
    .MAT_DIM      (2),
    .ACC_WIDTH    (33),
    .ENABLE_CACHE (1'b1),
    .CACHE_DEPTH  (2)
  ) cache_depth2_dft_fixture();

  tensor_unit_dft_fixture #(
    .USE_LEGACY8  (1'b0),
    .NAME         ("mat_dim1_dft"),
    .DATA_WIDTH   (16),
    .MAT_DIM      (1),
    .ACC_WIDTH    (32),
    .ENABLE_CACHE (1'b1),
    .CACHE_DEPTH  (2)
  ) mat_dim1_dft_fixture();
`else
  initial begin
    $display("INFO[tb]: DFT fixtures disabled; define TENSOR_UNIT_HAS_DFT_PORTS after adding DUT ports dft_mode, scan_enable, scan_in, and scan_out.");
  end
`endif

`ifdef FSDB_DUMP
  initial begin
    if (!$value$plusargs("FSDB_FILE=%s", fsdb_file)) begin
      fsdb_file = "sim/tb_tensor_unit.fsdb";
    end
    $fsdbDumpfile(fsdb_file);
    $fsdbDumpvars(0, tb_tensor_unit);
  end
`endif

  initial begin
    error_count = 0;
    fixture_done_count = 0;
    fixture_done_seen = '0;
  end

  initial begin : watchdog
    #200000;
    record_error("tb", "watchdog timeout waiting for fixtures to complete");
    $fatal(1, "tensor_unit testbench timeout");
  end

  initial begin : completion
    wait (fixture_done_count == EXPECTED_FIXTURES);
    #20;
    check_fixture_execution(EXPECTED_FIXTURE_MASK, EXPECTED_FIXTURES);
    if (error_count == 0) begin
      $display("PASS: all tensor_unit tests passed");
      $finish;
    end else begin
      $display("FAIL: tensor_unit tests failed with %0d errors", error_count);
      $fatal(1, "tensor_unit test failure");
    end
  end
endmodule
