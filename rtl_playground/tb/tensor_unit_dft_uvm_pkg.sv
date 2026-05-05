package tensor_unit_dft_uvm_pkg;
  import uvm_pkg::*;
  `include "uvm_macros.svh"

  localparam int DATA_WIDTH        = 16;
  localparam int MAT_DIM           = 2;
  localparam int ACC_WIDTH         = (2 * DATA_WIDTH) + $clog2(MAT_DIM);
  localparam int CACHE_DEPTH       = 4;
  localparam int MAT_ELEMS         = MAT_DIM * MAT_DIM;
  localparam int A_BITS            = MAT_ELEMS * DATA_WIDTH;
  localparam int C_BITS            = MAT_ELEMS * ACC_WIDTH;
  localparam int KEY_BITS          = 2 * A_BITS;
  localparam int CACHE_SLOTS       = CACHE_DEPTH;
  localparam int CACHE_INDEX_WIDTH = (CACHE_SLOTS <= 1) ? 1 : $clog2(CACHE_SLOTS);

  localparam int SCAN_OUT_VALID_LSB   = 0;
  localparam int SCAN_C_DATA_LSB      = SCAN_OUT_VALID_LSB + 1;
  localparam int SCAN_CACHE_HIT_LSB   = SCAN_C_DATA_LSB + C_BITS;
  localparam int SCAN_CACHE_MISS_LSB  = SCAN_CACHE_HIT_LSB + 1;
  localparam int SCAN_CACHE_VALID_LSB = SCAN_CACHE_MISS_LSB + 1;
  localparam int SCAN_REPLACE_PTR_LSB = SCAN_CACHE_VALID_LSB + CACHE_SLOTS;
  localparam int SCAN_CACHE_KEY_LSB   = SCAN_REPLACE_PTR_LSB + CACHE_INDEX_WIDTH;
  localparam int SCAN_CACHE_DATA_LSB  = SCAN_CACHE_KEY_LSB + (CACHE_SLOTS * KEY_BITS);
  localparam int SCAN_STATE_WIDTH     = SCAN_CACHE_DATA_LSB + (CACHE_SLOTS * C_BITS);

  typedef logic [A_BITS-1:0]        matrix_t;
  typedef logic [C_BITS-1:0]        result_t;
  typedef logic [KEY_BITS-1:0]      key_t;
  typedef logic [SCAN_STATE_WIDTH-1:0] scan_state_t;
  typedef logic [CACHE_SLOTS-1:0]   cache_valid_t;
  typedef logic [CACHE_INDEX_WIDTH-1:0] cache_ptr_t;

  typedef virtual tensor_unit_dft_if #(
    .DATA_WIDTH(DATA_WIDTH),
    .MAT_DIM(MAT_DIM),
    .ACC_WIDTH(ACC_WIDTH),
    .CACHE_DEPTH(CACHE_DEPTH)
  ) tensor_unit_dft_vif;

  typedef enum int unsigned {
    DFT_SCENARIO_IDLE,
    DFT_SCENARIO_INACTIVE_NOISE,
    DFT_SCENARIO_HOLD_VISIBLE,
    DFT_SCENARIO_HOLD_FULL_STATE,
    DFT_SCENARIO_SCANOUT_STATE,
    DFT_SCENARIO_RESET_SHIFT,
    DFT_SCENARIO_RESET_HOLD,
    DFT_SCENARIO_ROUNDTRIP_LOAD,
    DFT_SCENARIO_ROUNDTRIP_UNLOAD,
    DFT_SCENARIO_SCANIN_VISIBLE,
    DFT_SCENARIO_POST_SCAN_RECOVERY
  } tensor_dft_scenario_e;

  `uvm_analysis_imp_decl(_sample)

  class tensor_unit_dft_item extends uvm_sequence_item;
    rand tensor_dft_scenario_e scenario;

    `uvm_object_utils_begin(tensor_unit_dft_item)
      `uvm_field_enum(tensor_dft_scenario_e, scenario, UVM_DEFAULT)
    `uvm_object_utils_end

    function new(string name = "tensor_unit_dft_item");
      super.new(name);
      scenario = DFT_SCENARIO_IDLE;
    endfunction
  endclass

  class tensor_unit_dft_sample extends uvm_sequence_item;
    tensor_dft_scenario_e scenario;
    bit rst_n;
    bit dft_mode;
    bit scan_enable;
    bit scan_in;
    bit scan_out;
    bit in_valid;
    bit in_ready;
    bit out_valid;
    bit out_ready;
    bit cache_hit;
    bit cache_miss;
    result_t c_data;

    `uvm_object_utils(tensor_unit_dft_sample)

    function new(string name = "tensor_unit_dft_sample");
      super.new(name);
      scenario = DFT_SCENARIO_IDLE;
    endfunction
  endclass

  class tensor_unit_dft_sequence extends uvm_sequence #(tensor_unit_dft_item);
    `uvm_object_utils(tensor_unit_dft_sequence)

    function new(string name = "tensor_unit_dft_sequence");
      super.new(name);
    endfunction

    task body();
      tensor_unit_dft_item item;

      item = tensor_unit_dft_item::type_id::create("item");
      start_item(item);
      item.scenario = DFT_SCENARIO_IDLE;
      finish_item(item);
    endtask
  endclass

  class tensor_unit_dft_sequencer extends uvm_sequencer #(tensor_unit_dft_item);
    `uvm_component_utils(tensor_unit_dft_sequencer)

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction
  endclass

  class tensor_unit_dft_driver extends uvm_driver #(tensor_unit_dft_item);
    `uvm_component_utils(tensor_unit_dft_driver)

    tensor_unit_dft_vif vif;

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      if (!uvm_config_db#(tensor_unit_dft_vif)::get(this, "", "vif", vif)) begin
        `uvm_fatal("NOVIF", "tensor_unit_dft_driver requires tensor_unit_dft_if")
      end
    endfunction

    task run_phase(uvm_phase phase);
      forever begin
        seq_item_port.get_next_item(req);
        run_dft_regression();
        seq_item_port.item_done();
      end
    endtask

    function void check_or_error(bit condition, string message);
      if (!condition) begin
        `uvm_error("DFTDRV", message)
      end
    endfunction

    function void set_scenario(tensor_dft_scenario_e scenario);
      vif.scenario = int'(scenario);
    endfunction

    function matrix_t pattern_matrix(int unsigned seed);
      matrix_t packed_matrix;
      longint unsigned value;

      packed_matrix = '0;
      for (int idx = 0; idx < MAT_ELEMS; idx++) begin
        value = (64'd41 * (seed + 3)) + (64'd73 * idx) + (64'd19 * seed * (idx + 1));
        packed_matrix[(idx * DATA_WIDTH) +: DATA_WIDTH] = value[DATA_WIDTH-1:0];
      end
      return packed_matrix;
    endfunction

    function result_t ref_matmul(matrix_t a_flat, matrix_t b_flat);
      result_t result;
      logic [ACC_WIDTH-1:0] acc;
      logic [ACC_WIDTH-1:0] a_ext;
      logic [ACC_WIDTH-1:0] b_ext;
      int a_index;
      int b_index;
      int c_index;

      result = '0;
      for (int row = 0; row < MAT_DIM; row++) begin
        for (int col = 0; col < MAT_DIM; col++) begin
          acc = '0;
          for (int k = 0; k < MAT_DIM; k++) begin
            a_index = (row * MAT_DIM) + k;
            b_index = (k * MAT_DIM) + col;
            a_ext = '0;
            b_ext = '0;
            a_ext[DATA_WIDTH-1:0] = a_flat[(a_index * DATA_WIDTH) +: DATA_WIDTH];
            b_ext[DATA_WIDTH-1:0] = b_flat[(b_index * DATA_WIDTH) +: DATA_WIDTH];
            acc = acc + (a_ext * b_ext);
          end
          c_index = (row * MAT_DIM) + col;
          result[(c_index * ACC_WIDTH) +: ACC_WIDTH] = acc;
        end
      end
      return result;
    endfunction

    function bit scan_pattern_bit(int idx);
      return ((((idx * 7) + (idx / 3) + 1) & 1) == 1);
    endfunction

    task wait_sample();
      @(posedge vif.clk);
      #1;
    endtask

    task drive_idle();
      vif.in_valid    = 1'b0;
      vif.a_data      = '0;
      vif.b_data      = '0;
      vif.out_ready   = 1'b1;
      vif.dft_mode    = 1'b0;
      vif.scan_enable = 1'b0;
      vif.scan_in     = 1'b0;
    endtask

    task reset_functional(string check_name);
      @(negedge vif.clk);
      vif.rst_n       = 1'b0;
      vif.in_valid    = 1'b0;
      vif.a_data      = '0;
      vif.b_data      = '0;
      vif.out_ready   = 1'b0;
      vif.dft_mode    = 1'b0;
      vif.scan_enable = 1'b0;
      vif.scan_in     = 1'b0;
      set_scenario(DFT_SCENARIO_IDLE);

      repeat (3) wait_sample();
      check_or_error(vif.scan_out === 1'b0, {check_name, ": scan_out was not low during reset"});
      check_or_error(vif.out_valid === 1'b0, {check_name, ": out_valid was not reset"});
      check_or_error(vif.c_data === '0, {check_name, ": c_data was not reset"});
      check_or_error(vif.cache_hit === 1'b0, {check_name, ": cache_hit was not reset"});
      check_or_error(vif.cache_miss === 1'b0, {check_name, ": cache_miss was not reset"});

      @(negedge vif.clk);
      vif.rst_n = 1'b1;
      vif.out_ready = 1'b1;
      repeat (2) wait_sample();
    endtask

    task drive_inactive_scan_noise(int unsigned phase);
      vif.dft_mode    = 1'b0;
      vif.scan_enable = phase[0];
      vif.scan_in     = phase[1] ^ phase[2];
    endtask

    task check_functional_ready(string check_name);
      bit expected_ready;

      expected_ready = (!vif.out_valid || vif.out_ready);
      check_or_error(vif.in_ready === expected_ready,
             $sformatf("%s: in_ready expected=%0b actual=%0b",
                       check_name, expected_ready, vif.in_ready));
    endtask

    task check_cache_status(bit expected_hit, bit expected_miss, string check_name);
      check_or_error((vif.cache_hit === expected_hit) && (vif.cache_miss === expected_miss),
             $sformatf("%s: cache status expected hit=%0b miss=%0b actual hit=%0b miss=%0b",
                       check_name, expected_hit, expected_miss, vif.cache_hit, vif.cache_miss));
    endtask

    task drive_functional_and_expect(
      matrix_t a_flat,
      matrix_t b_flat,
      string check_name,
      bit inject_inactive_noise,
      bit expected_hit,
      bit expected_miss
    );
      result_t expected;
      bit sampled_ready;
      int wait_cycles;

      expected = ref_matmul(a_flat, b_flat);
      vif.dft_mode = 1'b0;
      vif.out_ready = 1'b1;

      @(negedge vif.clk);
      wait_cycles = 0;
      while (!vif.in_ready && (wait_cycles < 20)) begin
        if (inject_inactive_noise) begin
          drive_inactive_scan_noise(wait_cycles + 1);
        end else begin
          vif.scan_enable = 1'b0;
          vif.scan_in = 1'b0;
        end
        wait_cycles++;
        @(negedge vif.clk);
      end
      check_or_error(vif.in_ready === 1'b1, {check_name, ": timed out waiting for in_ready"});

      vif.a_data = a_flat;
      vif.b_data = b_flat;
      vif.in_valid = 1'b1;
      if (inject_inactive_noise) begin
        drive_inactive_scan_noise(17);
      end
      #1;
      sampled_ready = vif.in_ready;
      check_or_error(vif.scan_out === 1'b0, {check_name, ": scan_out was not low before accept"});
      check_functional_ready({check_name, ": accept setup"});

      wait_sample();
      check_or_error(vif.rst_n && vif.in_valid && sampled_ready,
             {check_name, ": functional input was not accepted"});
      check_or_error(vif.scan_out === 1'b0, {check_name, ": scan_out was not low after accept"});
      check_or_error(vif.out_valid === 1'b1, {check_name, ": output was not valid after accept"});
      check_or_error(vif.c_data === expected,
             $sformatf("%s: c_data expected=0x%0h actual=0x%0h", check_name, expected, vif.c_data));
      check_cache_status(expected_hit, expected_miss, {check_name, ": output"});

      @(negedge vif.clk);
      vif.in_valid = 1'b0;
      vif.a_data = '0;
      vif.b_data = '0;
      if (inject_inactive_noise) begin
        drive_inactive_scan_noise(23);
      end else begin
        vif.scan_enable = 1'b0;
        vif.scan_in = 1'b0;
      end
      wait_sample();
      check_or_error(vif.scan_out === 1'b0, {check_name, ": scan_out was not low after drain"});
      check_or_error(vif.out_valid === 1'b0, {check_name, ": output did not drain"});
      check_cache_status(1'b0, 1'b0, {check_name, ": drain"});
    endtask

    task build_expected_scan_state(
      input bit visible_valid,
      input result_t visible_c,
      input bit visible_hit,
      input bit visible_miss,
      input cache_valid_t expected_valid,
      input cache_ptr_t expected_ptr,
      input key_t expected_key [CACHE_SLOTS],
      input result_t expected_data [CACHE_SLOTS],
      output scan_state_t expected_state
    );
      expected_state = '0;
      expected_state[SCAN_OUT_VALID_LSB] = visible_valid;
      expected_state[SCAN_C_DATA_LSB +: C_BITS] = visible_c;
      expected_state[SCAN_CACHE_HIT_LSB] = visible_hit;
      expected_state[SCAN_CACHE_MISS_LSB] = visible_miss;
      expected_state[SCAN_CACHE_VALID_LSB +: CACHE_SLOTS] = expected_valid;
      expected_state[SCAN_REPLACE_PTR_LSB +: CACHE_INDEX_WIDTH] = expected_ptr;
      for (int entry = 0; entry < CACHE_SLOTS; entry++) begin
        expected_state[(SCAN_CACHE_KEY_LSB + (entry * KEY_BITS)) +: KEY_BITS] = expected_key[entry];
        expected_state[(SCAN_CACHE_DATA_LSB + (entry * C_BITS)) +: C_BITS] = expected_data[entry];
      end
    endtask

    task model_cache_fill(
      input matrix_t a_flat,
      input matrix_t b_flat,
      inout cache_valid_t expected_valid,
      inout cache_ptr_t expected_ptr,
      inout key_t expected_key [CACHE_SLOTS],
      inout result_t expected_data [CACHE_SLOTS]
    );
      expected_valid[expected_ptr] = 1'b1;
      expected_key[expected_ptr] = {a_flat, b_flat};
      expected_data[expected_ptr] = ref_matmul(a_flat, b_flat);
      if (expected_ptr == (CACHE_SLOTS - 1)) begin
        expected_ptr = '0;
      end else begin
        expected_ptr = expected_ptr + 1'b1;
      end
    endtask

    task create_known_functional_scan_state(string check_name, output scan_state_t expected_state);
      key_t expected_key [CACHE_SLOTS];
      result_t expected_data [CACHE_SLOTS];
      cache_valid_t expected_valid;
      cache_ptr_t expected_ptr;
      matrix_t a0;
      matrix_t b0;
      matrix_t a1;
      matrix_t b1;
      result_t final_c;
      bit sampled_ready;

      reset_functional({check_name, " reset"});
      expected_valid = '0;
      expected_ptr = '0;
      for (int entry = 0; entry < CACHE_SLOTS; entry++) begin
        expected_key[entry] = '0;
        expected_data[entry] = '0;
      end

      a0 = pattern_matrix(40);
      b0 = pattern_matrix(41);
      a1 = pattern_matrix(42);
      b1 = pattern_matrix(43);

      drive_functional_and_expect(a0, b0, {check_name, " cache fill entry0"}, 1'b0, 1'b0, 1'b1);
      model_cache_fill(a0, b0, expected_valid, expected_ptr, expected_key, expected_data);
      drive_functional_and_expect(a1, b1, {check_name, " cache fill entry1"}, 1'b0, 1'b0, 1'b1);
      model_cache_fill(a1, b1, expected_valid, expected_ptr, expected_key, expected_data);

      final_c = expected_data[0];
      @(negedge vif.clk);
      vif.dft_mode = 1'b0;
      vif.scan_enable = 1'b0;
      vif.scan_in = 1'b0;
      vif.out_ready = 1'b0;
      vif.a_data = a0;
      vif.b_data = b0;
      vif.in_valid = 1'b1;
      #1;
      sampled_ready = vif.in_ready;

      wait_sample();
      check_or_error(vif.rst_n && vif.in_valid && sampled_ready,
             {check_name, ": known-state input was not accepted"});
      check_or_error(vif.out_valid === 1'b1, {check_name, ": known-state output was not pending"});
      check_or_error(vif.c_data === final_c,
             $sformatf("%s: known-state c_data expected=0x%0h actual=0x%0h",
                       check_name, final_c, vif.c_data));
      check_cache_status(1'b1, 1'b0, {check_name, ": known-state output"});

      @(negedge vif.clk);
      vif.in_valid = 1'b0;
      vif.a_data = '0;
      vif.b_data = '0;
      build_expected_scan_state(1'b1, final_c, 1'b1, 1'b0,
                                expected_valid, expected_ptr, expected_key,
                                expected_data, expected_state);
    endtask

    task scan_cycle(input bit si, output bit so);
      @(negedge vif.clk);
      vif.rst_n       = 1'b1;
      vif.dft_mode    = 1'b1;
      vif.scan_enable = 1'b1;
      vif.scan_in     = si;
      vif.in_valid    = 1'b1;
      vif.a_data      = pattern_matrix(1000 + (si ? 1 : 0));
      vif.b_data      = pattern_matrix(1100 + (si ? 1 : 0));
      vif.out_ready   = si;
      #1;
      so = vif.scan_out;
      wait_sample();
      check_or_error(vif.in_ready === 1'b0, "active scan did not suppress functional input readiness");
    endtask

    task scan_out_expected_state(scan_state_t expected_state, string check_name, tensor_dft_scenario_e scenario);
      bit observed;

      set_scenario(scenario);
      for (int idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        scan_cycle(1'b0, observed);
        check_or_error(observed === expected_state[idx],
               $sformatf("%s: scan bit %0d expected=%0b observed=%0b",
                         check_name, idx, expected_state[idx], observed));
      end
    endtask

    task run_inactive_noise_checks();
      matrix_t a_flat;
      matrix_t b_flat;

      `uvm_info("DFTSEQ", "Running functional-mode scan-pin noise checks", UVM_LOW)
      a_flat = pattern_matrix(20);
      b_flat = pattern_matrix(21);

      reset_functional("inactive DFT clean baseline");
      drive_functional_and_expect(a_flat, b_flat, "inactive clean miss", 1'b0, 1'b0, 1'b1);
      drive_functional_and_expect(a_flat, b_flat, "inactive clean hit", 1'b0, 1'b1, 1'b0);

      reset_functional("inactive DFT noise");
      set_scenario(DFT_SCENARIO_INACTIVE_NOISE);
      for (int idx = 0; idx < 6; idx++) begin
        @(negedge vif.clk);
        drive_inactive_scan_noise(idx + 1);
        wait_sample();
        check_or_error(vif.scan_out === 1'b0, "functional-mode scan_out was not forced low during scan-pin noise");
        check_functional_ready("inactive scan noise idle");
        check_or_error(vif.out_valid === 1'b0, "inactive scan noise produced out_valid");
        check_cache_status(1'b0, 1'b0, "inactive scan noise idle");
      end
      drive_functional_and_expect(a_flat, b_flat, "inactive noisy miss", 1'b1, 1'b0, 1'b1);
      drive_functional_and_expect(a_flat, b_flat, "inactive noisy hit", 1'b1, 1'b1, 1'b0);
    endtask

    task run_hold_and_scanout_checks();
      scan_state_t expected_state;
      int hold_idx;

      `uvm_info("DFTSEQ", "Running DFT hold and functional-state scan-out checks", UVM_LOW)
      create_known_functional_scan_state("hold full-state preload", expected_state);

      @(negedge vif.clk);
      vif.dft_mode = 1'b1;
      vif.scan_enable = 1'b0;
      vif.scan_in = 1'b1;
      vif.in_valid = 1'b1;
      vif.a_data = pattern_matrix(46);
      vif.b_data = pattern_matrix(47);
      vif.out_ready = 1'b1;
      set_scenario(DFT_SCENARIO_HOLD_VISIBLE);

      for (hold_idx = 0; hold_idx < 5; hold_idx++) begin
        if (hold_idx == 2) begin
          set_scenario(DFT_SCENARIO_HOLD_FULL_STATE);
        end
        wait_sample();
        check_or_error(vif.in_ready === 1'b0, "DFT hold did not suppress functional input readiness");
        check_or_error(vif.scan_out === expected_state[SCAN_OUT_VALID_LSB], "DFT hold scan_out tail mismatch");
        check_or_error(vif.out_valid === expected_state[SCAN_OUT_VALID_LSB], "DFT hold changed out_valid");
        check_or_error(vif.c_data === expected_state[SCAN_C_DATA_LSB +: C_BITS], "DFT hold changed c_data");
        check_or_error(vif.cache_hit === expected_state[SCAN_CACHE_HIT_LSB], "DFT hold changed cache_hit");
        check_or_error(vif.cache_miss === expected_state[SCAN_CACHE_MISS_LSB], "DFT hold changed cache_miss");
      end

      scan_out_expected_state(expected_state, "functional-state scan-out", DFT_SCENARIO_SCANOUT_STATE);
      reset_functional("scan-out cleanup");
    endtask

    task reset_during_active_scan();
      @(negedge vif.clk);
      vif.rst_n       = 1'b0;
      vif.dft_mode    = 1'b1;
      vif.scan_enable = 1'b1;
      vif.scan_in     = 1'b1;
      vif.in_valid    = 1'b1;
      vif.a_data      = pattern_matrix(900);
      vif.b_data      = pattern_matrix(901);
      vif.out_ready   = 1'b1;
      set_scenario(DFT_SCENARIO_RESET_SHIFT);
      repeat (3) wait_sample();
      check_or_error(vif.scan_out === 1'b0, "reset during active scan did not force scan_out low");
      check_or_error(vif.out_valid === 1'b0, "reset during active scan did not clear out_valid");
      check_or_error(vif.c_data === '0, "reset during active scan did not clear c_data");
      check_cache_status(1'b0, 1'b0, "reset during active scan");

      @(negedge vif.clk);
      vif.rst_n = 1'b1;
      vif.scan_in = 1'b0;
    endtask

    task reset_during_scan_hold();
      @(negedge vif.clk);
      vif.rst_n       = 1'b0;
      vif.dft_mode    = 1'b1;
      vif.scan_enable = 1'b0;
      vif.scan_in     = 1'b1;
      vif.in_valid    = 1'b1;
      vif.a_data      = pattern_matrix(910);
      vif.b_data      = pattern_matrix(911);
      vif.out_ready   = 1'b1;
      set_scenario(DFT_SCENARIO_RESET_HOLD);
      repeat (3) wait_sample();
      check_or_error(vif.scan_out === 1'b0, "reset during scan hold did not force scan_out low");
      check_or_error(vif.out_valid === 1'b0, "reset during scan hold did not clear out_valid");
      check_or_error(vif.c_data === '0, "reset during scan hold did not clear c_data");
      check_cache_status(1'b0, 1'b0, "reset during scan hold");

      @(negedge vif.clk);
      vif.rst_n = 1'b1;
      vif.scan_in = 1'b0;
    endtask

    task run_reset_priority_checks();
      scan_state_t zero_state;
      scan_state_t expected_state;

      `uvm_info("DFTSEQ", "Running reset priority and reset observability checks", UVM_LOW)
      zero_state = '0;
      reset_during_active_scan();
      scan_out_expected_state(zero_state, "reset active-scan observability", DFT_SCENARIO_RESET_SHIFT);
      reset_functional("reset active-scan cleanup");

      create_known_functional_scan_state("reset hold preload", expected_state);
      reset_during_scan_hold();
      scan_out_expected_state(zero_state, "reset hold observability", DFT_SCENARIO_RESET_HOLD);
      reset_functional("reset hold cleanup");
    endtask

    task run_scan_roundtrip_check();
      scan_state_t pattern;
      bit observed;

      `uvm_info("DFTSEQ", "Running destructive scan round-trip check", UVM_LOW)
      reset_functional("scan roundtrip reset");
      set_scenario(DFT_SCENARIO_ROUNDTRIP_LOAD);
      for (int idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        pattern[idx] = scan_pattern_bit(idx);
        scan_cycle(pattern[idx], observed);
      end

      set_scenario(DFT_SCENARIO_ROUNDTRIP_UNLOAD);
      for (int idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        scan_cycle(1'b0, observed);
        check_or_error(observed === pattern[idx],
               $sformatf("scan round-trip bit %0d expected=%0b observed=%0b",
                         idx, pattern[idx], observed));
      end
    endtask

    task run_scanin_visible_check();
      scan_state_t target_state;
      key_t target_key [CACHE_SLOTS];
      result_t target_data [CACHE_SLOTS];
      cache_valid_t target_valid;
      cache_ptr_t target_ptr;
      matrix_t a_flat;
      matrix_t b_flat;
      result_t target_c;
      bit observed;

      `uvm_info("DFTSEQ", "Running scan-in visible controllability check", UVM_LOW)
      reset_functional("scan-in visible reset");

      a_flat = pattern_matrix(70);
      b_flat = pattern_matrix(71);
      target_c = ref_matmul(a_flat, b_flat);
      target_valid = '1;
      target_ptr = CACHE_SLOTS - 1;
      for (int entry = 0; entry < CACHE_SLOTS; entry++) begin
        target_key[entry] = {pattern_matrix(80 + entry), pattern_matrix(90 + entry)};
        target_data[entry] = ref_matmul(pattern_matrix(80 + entry), pattern_matrix(90 + entry));
      end

      build_expected_scan_state(1'b1, target_c, 1'b1, 1'b0,
                                target_valid, target_ptr, target_key,
                                target_data, target_state);

      set_scenario(DFT_SCENARIO_SCANIN_VISIBLE);
      for (int idx = 0; idx < SCAN_STATE_WIDTH; idx++) begin
        scan_cycle(target_state[idx], observed);
      end

      @(negedge vif.clk);
      vif.dft_mode = 1'b1;
      vif.scan_enable = 1'b0;
      vif.scan_in = 1'b1;
      vif.in_valid = 1'b1;
      vif.a_data = pattern_matrix(72);
      vif.b_data = pattern_matrix(73);
      vif.out_ready = 1'b1;

      repeat (3) begin
        wait_sample();
        check_or_error(vif.in_ready === 1'b0, "scan-loaded hold did not suppress functional readiness");
        check_or_error(vif.scan_out === target_state[SCAN_OUT_VALID_LSB], "scan-loaded hold scan_out mismatch");
        check_or_error(vif.out_valid === target_state[SCAN_OUT_VALID_LSB], "scan-loaded hold out_valid mismatch");
        check_or_error(vif.c_data === target_c,
               $sformatf("scan-loaded hold c_data expected=0x%0h actual=0x%0h", target_c, vif.c_data));
        check_or_error(vif.cache_hit === target_state[SCAN_CACHE_HIT_LSB], "scan-loaded hold cache_hit mismatch");
        check_or_error(vif.cache_miss === target_state[SCAN_CACHE_MISS_LSB], "scan-loaded hold cache_miss mismatch");
      end
    endtask

    task run_post_scan_recovery_check();
      `uvm_info("DFTSEQ", "Running post-scan reset recovery check", UVM_LOW)
      reset_functional("post-scan recovery");
      set_scenario(DFT_SCENARIO_POST_SCAN_RECOVERY);
      drive_functional_and_expect(pattern_matrix(30), pattern_matrix(31),
                                  "post-scan functional transaction", 1'b0, 1'b0, 1'b1);
    endtask

    task run_dft_regression();
      vif.rst_n       = 1'b0;
      vif.in_valid    = 1'b0;
      vif.a_data      = '0;
      vif.b_data      = '0;
      vif.out_ready   = 1'b0;
      vif.dft_mode    = 1'b0;
      vif.scan_enable = 1'b0;
      vif.scan_in     = 1'b0;
      set_scenario(DFT_SCENARIO_IDLE);

      run_inactive_noise_checks();
      run_hold_and_scanout_checks();
      run_reset_priority_checks();
      run_scan_roundtrip_check();
      run_scanin_visible_check();
      run_post_scan_recovery_check();
      drive_idle();

      `uvm_info("DFTSEQ", "Tensor DFT UVM regression completed", UVM_LOW)
    endtask
  endclass

  class tensor_unit_dft_monitor extends uvm_component;
    `uvm_component_utils(tensor_unit_dft_monitor)

    tensor_unit_dft_vif vif;
    uvm_analysis_port #(tensor_unit_dft_sample) sample_ap;

    function new(string name, uvm_component parent);
      super.new(name, parent);
      sample_ap = new("sample_ap", this);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      if (!uvm_config_db#(tensor_unit_dft_vif)::get(this, "", "vif", vif)) begin
        `uvm_fatal("NOVIF", "tensor_unit_dft_monitor requires tensor_unit_dft_if")
      end
    endfunction

    task run_phase(uvm_phase phase);
      tensor_unit_dft_sample sample;

      forever begin
        @(posedge vif.clk);
        #1;
        sample = tensor_unit_dft_sample::type_id::create("sample", this);
        sample.scenario = tensor_dft_scenario_e'(vif.scenario);
        sample.rst_n = vif.rst_n;
        sample.dft_mode = vif.dft_mode;
        sample.scan_enable = vif.scan_enable;
        sample.scan_in = vif.scan_in;
        sample.scan_out = vif.scan_out;
        sample.in_valid = vif.in_valid;
        sample.in_ready = vif.in_ready;
        sample.out_valid = vif.out_valid;
        sample.out_ready = vif.out_ready;
        sample.cache_hit = vif.cache_hit;
        sample.cache_miss = vif.cache_miss;
        sample.c_data = vif.c_data;
        sample_ap.write(sample);
      end
    endtask
  endclass

  class tensor_unit_dft_scoreboard extends uvm_component;
    `uvm_component_utils(tensor_unit_dft_scoreboard)

    uvm_analysis_imp_sample #(tensor_unit_dft_sample, tensor_unit_dft_scoreboard) sample_export;
    tensor_unit_dft_sample prev_sample;
    bit have_prev;

    function new(string name, uvm_component parent);
      super.new(name, parent);
      sample_export = new("sample_export", this);
    endfunction

    function void write_sample(tensor_unit_dft_sample sample);
      if (!sample.rst_n) begin
        if (sample.scan_out !== 1'b0) begin
          `uvm_error("DFTSB", "scan_out was not forced low during reset")
        end
        if (sample.out_valid !== 1'b0) begin
          `uvm_error("DFTSB", "out_valid was not reset")
        end
        if (sample.c_data !== '0) begin
          `uvm_error("DFTSB", "c_data was not reset")
        end
        if ((sample.cache_hit !== 1'b0) || (sample.cache_miss !== 1'b0)) begin
          `uvm_error("DFTSB", "cache status was not reset")
        end
      end else begin
        if (!sample.dft_mode && (sample.scan_out !== 1'b0)) begin
          `uvm_error("DFTSB", "scan_out was not forced low in functional mode")
        end
        if (sample.dft_mode && (sample.in_ready !== 1'b0)) begin
          `uvm_error("DFTSB", "DFT mode did not suppress in_ready")
        end
        if (!sample.dft_mode && (sample.in_ready !== (!sample.out_valid || sample.out_ready))) begin
          `uvm_error("DFTSB", "functional in_ready did not match !out_valid || out_ready")
        end
      end

      if (have_prev && sample.rst_n && prev_sample.rst_n &&
          sample.dft_mode && !sample.scan_enable &&
          prev_sample.dft_mode && !prev_sample.scan_enable) begin
        if ((sample.out_valid !== prev_sample.out_valid) ||
            (sample.c_data !== prev_sample.c_data) ||
            (sample.cache_hit !== prev_sample.cache_hit) ||
            (sample.cache_miss !== prev_sample.cache_miss)) begin
          `uvm_error("DFTSB", "DFT hold mode changed visible state")
        end
      end

      prev_sample = sample;
      have_prev = 1'b1;
    endfunction
  endclass

  class tensor_unit_dft_coverage extends uvm_component;
    `uvm_component_utils(tensor_unit_dft_coverage)

    uvm_analysis_imp_sample #(tensor_unit_dft_sample, tensor_unit_dft_coverage) sample_export;

    covergroup dft_cg with function sample(tensor_unit_dft_sample sample);
      option.per_instance = 1;

      cp_dft_mode: coverpoint sample.dft_mode iff (sample.rst_n) {
        bins functional = {0};
        bins dft_active = {1};
      }

      cp_scan_enable: coverpoint sample.scan_enable iff (sample.rst_n && sample.dft_mode) {
        bins hold = {0};
        bins shift = {1};
      }

      cp_dft_state: coverpoint {sample.dft_mode, sample.scan_enable} iff (sample.rst_n) {
        bins functional_quiet_or_noise[] = {2'b00, 2'b01};
        bins hold = {2'b10};
        bins shift = {2'b11};
      }

      cp_functional_scan_noise: coverpoint {sample.scan_enable, sample.scan_in}
          iff (sample.rst_n && !sample.dft_mode) {
        bins quiet = {2'b00};
        bins noisy[] = {2'b01, 2'b10, 2'b11};
      }

      cp_scan_out_functional: coverpoint sample.scan_out iff (sample.rst_n && !sample.dft_mode) {
        bins forced_low = {0};
      }

      cp_scan_out_reset: coverpoint sample.scan_out iff (!sample.rst_n) {
        bins forced_low = {0};
      }

      cp_suppressed_accept: coverpoint (sample.rst_n && sample.dft_mode && sample.in_valid && !sample.in_ready) {
        bins observed = {1};
      }

      cp_scan_out_shift: coverpoint sample.scan_out iff (sample.rst_n && sample.dft_mode && sample.scan_enable) {
        bins zero = {0};
        bins one = {1};
      }

      cp_cache_status: coverpoint {sample.cache_hit, sample.cache_miss} iff (sample.rst_n && sample.out_valid) {
        bins miss = {2'b01};
        bins hit = {2'b10};
        bins both_from_scan = {2'b11};
      }

      cp_scenario: coverpoint int'(sample.scenario) iff (sample.rst_n) {
        bins inactive_noise = {DFT_SCENARIO_INACTIVE_NOISE};
        bins hold_visible = {DFT_SCENARIO_HOLD_VISIBLE};
        bins hold_full_state = {DFT_SCENARIO_HOLD_FULL_STATE};
        bins scanout_state = {DFT_SCENARIO_SCANOUT_STATE};
        bins reset_shift = {DFT_SCENARIO_RESET_SHIFT};
        bins reset_hold = {DFT_SCENARIO_RESET_HOLD};
        bins roundtrip_load = {DFT_SCENARIO_ROUNDTRIP_LOAD};
        bins roundtrip_unload = {DFT_SCENARIO_ROUNDTRIP_UNLOAD};
        bins scanin_visible = {DFT_SCENARIO_SCANIN_VISIBLE};
        bins post_scan_recovery = {DFT_SCENARIO_POST_SCAN_RECOVERY};
      }
    endgroup

    function new(string name, uvm_component parent);
      super.new(name, parent);
      sample_export = new("sample_export", this);
      dft_cg = new();
    endfunction

    function void write_sample(tensor_unit_dft_sample sample);
      dft_cg.sample(sample);
    endfunction
  endclass

  class tensor_unit_dft_agent extends uvm_agent;
    `uvm_component_utils(tensor_unit_dft_agent)

    tensor_unit_dft_sequencer sequencer;
    tensor_unit_dft_driver driver;
    tensor_unit_dft_monitor monitor;

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      sequencer = tensor_unit_dft_sequencer::type_id::create("sequencer", this);
      driver = tensor_unit_dft_driver::type_id::create("driver", this);
      monitor = tensor_unit_dft_monitor::type_id::create("monitor", this);
    endfunction

    function void connect_phase(uvm_phase phase);
      super.connect_phase(phase);
      driver.seq_item_port.connect(sequencer.seq_item_export);
    endfunction
  endclass

  class tensor_unit_dft_env extends uvm_env;
    `uvm_component_utils(tensor_unit_dft_env)

    tensor_unit_dft_agent agent;
    tensor_unit_dft_scoreboard scoreboard;
    tensor_unit_dft_coverage coverage;

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      agent = tensor_unit_dft_agent::type_id::create("agent", this);
      scoreboard = tensor_unit_dft_scoreboard::type_id::create("scoreboard", this);
      coverage = tensor_unit_dft_coverage::type_id::create("coverage", this);
    endfunction

    function void connect_phase(uvm_phase phase);
      super.connect_phase(phase);
      agent.monitor.sample_ap.connect(scoreboard.sample_export);
      agent.monitor.sample_ap.connect(coverage.sample_export);
    endfunction
  endclass

  class tensor_unit_dft_test extends uvm_test;
    `uvm_component_utils(tensor_unit_dft_test)

    tensor_unit_dft_env env;

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      env = tensor_unit_dft_env::type_id::create("env", this);
    endfunction

    task run_phase(uvm_phase phase);
      tensor_unit_dft_sequence seq;

      phase.raise_objection(this);
      seq = tensor_unit_dft_sequence::type_id::create("seq");
      seq.start(env.agent.sequencer);
      phase.drop_objection(this);
    endtask
  endclass
endpackage
