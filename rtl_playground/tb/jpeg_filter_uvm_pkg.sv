package jpeg_filter_uvm_pkg;
  import uvm_pkg::*;
  `include "uvm_macros.svh"

  typedef enum bit [1:0] {
    JPEG_FILTER_IDENTITY = 2'd0,
    JPEG_FILTER_BLUR     = 2'd1,
    JPEG_FILTER_SHARPEN  = 2'd2,
    JPEG_FILTER_EDGE     = 2'd3
  } jpeg_filter_mode_e;

  `uvm_analysis_imp_decl(_in)
  `uvm_analysis_imp_decl(_out)

  class jpeg_filter_ref_model #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  );
    localparam int BLOCK_SAMPLES = BLOCK_DIM * BLOCK_DIM;
    localparam int BLOCK_BITS    = BLOCK_SAMPLES * SAMPLE_WIDTH;

    typedef bit [SAMPLE_WIDTH-1:0] sample_t;
    typedef bit [BLOCK_BITS-1:0]   block_t;

    static function int clamp_index(int value, int low, int high);
      if (value < low) begin
        return low;
      end
      if (value > high) begin
        return high;
      end
      return value;
    endfunction

    static function sample_t get_sample(block_t block, int unsigned sample_index);
      return sample_t'(block[sample_index*SAMPLE_WIDTH +: SAMPLE_WIDTH]);
    endfunction

    static function int signed kernel_coeff(bit [1:0] mode, int tap_row, int tap_col);
      case (mode)
        JPEG_FILTER_BLUR: begin
          if (tap_row == 1 && tap_col == 1) begin
            return 4;
          end
          if (tap_row == 1 || tap_col == 1) begin
            return 2;
          end
          return 1;
        end
        JPEG_FILTER_SHARPEN: begin
          if (tap_row == 1 && tap_col == 1) begin
            return 5;
          end
          if ((tap_row == 1) ^ (tap_col == 1)) begin
            return -1;
          end
          return 0;
        end
        JPEG_FILTER_EDGE: begin
          if (tap_row == 1 && tap_col == 1) begin
            return 8;
          end
          return -1;
        end
        default: begin
          if (tap_row == 1 && tap_col == 1) begin
            return 1;
          end
          return 0;
        end
      endcase
    endfunction

    static function sample_t clamp_sample(int signed value);
      int signed max_sample;

      max_sample = (1 << SAMPLE_WIDTH) - 1;
      if (value < 0) begin
        return '0;
      end
      if (value > max_sample) begin
        return sample_t'(max_sample);
      end
      return sample_t'(value);
    endfunction

    static function sample_t filter_sample(block_t block, bit [1:0] mode, int unsigned sample_index);
      int row;
      int col;
      int tap_row;
      int tap_col;
      int neighbor_row;
      int neighbor_col;
      int unsigned neighbor_index;
      int signed coeff;
      int signed acc;
      int signed normalized;

      if (mode == JPEG_FILTER_IDENTITY) begin
        return get_sample(block, sample_index);
      end

      row = sample_index / BLOCK_DIM;
      col = sample_index % BLOCK_DIM;
      acc = 0;

      for (tap_row = 0; tap_row < 3; tap_row++) begin
        for (tap_col = 0; tap_col < 3; tap_col++) begin
          neighbor_row = clamp_index(row + tap_row - 1, 0, BLOCK_DIM - 1);
          neighbor_col = clamp_index(col + tap_col - 1, 0, BLOCK_DIM - 1);
          neighbor_index = neighbor_row * BLOCK_DIM + neighbor_col;
          coeff = kernel_coeff(mode, tap_row, tap_col);
          acc += coeff * int'(get_sample(block, neighbor_index));
        end
      end

      normalized = (mode == JPEG_FILTER_BLUR) ? (acc >>> 4) : acc;
      return clamp_sample(normalized);
    endfunction
  endclass

  class jpeg_filter_patterns #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8
  );
    localparam int BLOCK_SAMPLES = BLOCK_DIM * BLOCK_DIM;
    localparam int BLOCK_BITS    = BLOCK_SAMPLES * SAMPLE_WIDTH;

    typedef bit [SAMPLE_WIDTH-1:0] sample_t;
    typedef bit [BLOCK_BITS-1:0]   block_t;

    static function sample_t max_sample();
      return sample_t'((1 << SAMPLE_WIDTH) - 1);
    endfunction

    static function block_t set_sample(block_t block, int unsigned sample_index, sample_t value);
      block[sample_index*SAMPLE_WIDTH +: SAMPLE_WIDTH] = value;
      return block;
    endfunction

    static function block_t all_zero();
      return '0;
    endfunction

    static function block_t all_max();
      block_t block;
      int unsigned index;

      block = '0;
      for (index = 0; index < BLOCK_SAMPLES; index++) begin
        block = set_sample(block, index, max_sample());
      end
      return block;
    endfunction

    static function block_t ramp();
      block_t block;
      int unsigned index;

      block = '0;
      for (index = 0; index < BLOCK_SAMPLES; index++) begin
        block = set_sample(block, index, sample_t'((index * 3 + 7) & ((1 << SAMPLE_WIDTH) - 1)));
      end
      return block;
    endfunction

    static function block_t impulse(int unsigned sample_index, sample_t value);
      block_t block;

      block = '0;
      return set_sample(block, sample_index, value);
    endfunction

    static function block_t checkerboard();
      block_t block;
      int unsigned row;
      int unsigned col;
      int unsigned index;

      block = '0;
      for (row = 0; row < BLOCK_DIM; row++) begin
        for (col = 0; col < BLOCK_DIM; col++) begin
          index = row * BLOCK_DIM + col;
          block = set_sample(block, index, ((row + col) % 2) ? max_sample() : sample_t'(0));
        end
      end
      return block;
    endfunction

    static function block_t sharpen_overflow();
      block_t block;
      int unsigned center;

      center = (BLOCK_DIM / 2) * BLOCK_DIM + (BLOCK_DIM / 2);
      block = '0;
      block = set_sample(block, center, max_sample());
      return block;
    endfunction

    static function block_t edge_negative_clamp();
      block_t block;
      int unsigned center;

      center = (BLOCK_DIM / 2) * BLOCK_DIM + (BLOCK_DIM / 2);
      block = all_max();
      block = set_sample(block, center, sample_t'(0));
      return block;
    endfunction

    static function block_t corner_boundary();
      return impulse(0, max_sample());
    endfunction

    static function block_t top_edge_boundary();
      block_t block;
      int unsigned col;

      block = '0;
      for (col = 0; col < BLOCK_DIM; col++) begin
        block = set_sample(block, col, max_sample());
      end
      return block;
    endfunction
  endclass

  class jpeg_filter_seq_item #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_sequence_item;
    localparam int BLOCK_SAMPLES = BLOCK_DIM * BLOCK_DIM;
    localparam int BLOCK_BITS    = BLOCK_SAMPLES * SAMPLE_WIDTH;

    typedef bit [BLOCK_BITS-1:0] block_t;

    rand block_t block_data;
    rand bit [1:0] filter_mode;
    rand int unsigned pre_accept_delay;

    string scenario;
    bit enable_backpressure;
    int unsigned stall_after_beat;
    int unsigned stall_cycles;

    constraint c_pre_accept_delay {
      pre_accept_delay inside {[0:3]};
    }

    `uvm_object_param_utils(jpeg_filter_seq_item #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name = "jpeg_filter_seq_item");
      super.new(name);
      block_data = '0;
      filter_mode = JPEG_FILTER_IDENTITY;
      pre_accept_delay = 0;
      scenario = "";
      enable_backpressure = 1'b0;
      stall_after_beat = 0;
      stall_cycles = 0;
    endfunction

    function string convert2string();
      return $sformatf("scenario=%s mode=%0d lanes=%0d backpressure=%0b stall_after=%0d stall_cycles=%0d",
                       scenario, filter_mode, LANES, enable_backpressure, stall_after_beat, stall_cycles);
    endfunction
  endclass

  class jpeg_filter_in_txn #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_sequence_item;
    localparam int BLOCK_SAMPLES = BLOCK_DIM * BLOCK_DIM;
    localparam int BLOCK_BITS    = BLOCK_SAMPLES * SAMPLE_WIDTH;

    typedef bit [BLOCK_BITS-1:0] block_t;

    block_t block_data;
    bit [1:0] filter_mode;

    `uvm_object_param_utils(jpeg_filter_in_txn #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name = "jpeg_filter_in_txn");
      super.new(name);
    endfunction
  endclass

  class jpeg_filter_out_beat #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_sequence_item;
    localparam int SAMPLE_BITS = LANES * SAMPLE_WIDTH;

    bit [SAMPLE_BITS-1:0] sample_data;
    bit [LANES-1:0]       sample_keep;
    bit                   out_last;
    bit                   stalled_before_handshake;

    `uvm_object_param_utils(jpeg_filter_out_beat #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name = "jpeg_filter_out_beat");
      super.new(name);
    endfunction
  endclass

  class jpeg_filter_single_block_seq #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_sequence #(jpeg_filter_seq_item #(SAMPLE_WIDTH, BLOCK_DIM, LANES));
    localparam int BLOCK_SAMPLES = BLOCK_DIM * BLOCK_DIM;
    localparam int BLOCK_BITS    = BLOCK_SAMPLES * SAMPLE_WIDTH;

    typedef bit [BLOCK_BITS-1:0] block_t;
    typedef jpeg_filter_seq_item #(SAMPLE_WIDTH, BLOCK_DIM, LANES) item_t;

    block_t block_data;
    bit [1:0] filter_mode;
    string scenario;
    bit enable_backpressure;
    int unsigned stall_after_beat;
    int unsigned stall_cycles;

    `uvm_object_param_utils(jpeg_filter_single_block_seq #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name = "jpeg_filter_single_block_seq");
      super.new(name);
      block_data = '0;
      filter_mode = JPEG_FILTER_IDENTITY;
      scenario = "single_block";
      enable_backpressure = 1'b0;
      stall_after_beat = 0;
      stall_cycles = 0;
    endfunction

    task body();
      item_t req;

      req = item_t::type_id::create("req");
      start_item(req);
      req.block_data = block_data;
      req.filter_mode = filter_mode;
      req.scenario = scenario;
      req.enable_backpressure = enable_backpressure;
      req.stall_after_beat = stall_after_beat;
      req.stall_cycles = stall_cycles;
      req.pre_accept_delay = 0;
      finish_item(req);
    endtask
  endclass

  class jpeg_filter_scenario_seq #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_sequence #(jpeg_filter_seq_item #(SAMPLE_WIDTH, BLOCK_DIM, LANES));
    localparam int BLOCK_SAMPLES = BLOCK_DIM * BLOCK_DIM;
    localparam int BLOCK_BITS    = BLOCK_SAMPLES * SAMPLE_WIDTH;
    localparam int EXPECTED_BEATS = (BLOCK_SAMPLES + LANES - 1) / LANES;

    typedef bit [BLOCK_BITS-1:0] block_t;
    typedef jpeg_filter_seq_item #(SAMPLE_WIDTH, BLOCK_DIM, LANES) item_t;
    typedef jpeg_filter_patterns #(SAMPLE_WIDTH, BLOCK_DIM) patterns_t;

    `uvm_object_param_utils(jpeg_filter_scenario_seq #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name = "jpeg_filter_scenario_seq");
      super.new(name);
    endfunction

    task send_block(
      string scenario,
      bit [1:0] filter_mode,
      block_t block_data,
      bit enable_backpressure = 1'b0,
      int unsigned stall_after_beat = 0,
      int unsigned stall_cycles = 0,
      int pre_accept_delay_override = -1
    );
      item_t req;

      req = item_t::type_id::create($sformatf("req_%s", scenario));
      start_item(req);
      req.block_data = block_data;
      req.filter_mode = filter_mode;
      req.scenario = scenario;
      req.enable_backpressure = enable_backpressure;
      req.stall_after_beat = stall_after_beat;
      req.stall_cycles = stall_cycles;
      if (pre_accept_delay_override >= 0) begin
        req.pre_accept_delay = pre_accept_delay_override;
      end else begin
        req.pre_accept_delay = (LANES + scenario.len()) % 4;
      end
      finish_item(req);
    endtask

    task body();
      send_block("identity_ramp", JPEG_FILTER_IDENTITY, patterns_t::ramp());
      send_block("back_to_back_identity_a", JPEG_FILTER_IDENTITY, patterns_t::ramp(),
                 1'b0, 0, 0, 0);
      send_block("back_to_back_identity_b", JPEG_FILTER_IDENTITY, patterns_t::checkerboard(),
                 1'b0, 0, 0, 0);
      send_block("blur_ramp", JPEG_FILTER_BLUR, patterns_t::ramp());
      send_block("blur_impulse", JPEG_FILTER_BLUR,
                 patterns_t::impulse((BLOCK_DIM / 2) * BLOCK_DIM + (BLOCK_DIM / 2), patterns_t::max_sample()));
      send_block("blur_all_zero", JPEG_FILTER_BLUR, patterns_t::all_zero());
      send_block("blur_all_max", JPEG_FILTER_BLUR, patterns_t::all_max());
      send_block("sharpen_overflow_clamp", JPEG_FILTER_SHARPEN, patterns_t::sharpen_overflow());
      send_block("edge_negative_clamp", JPEG_FILTER_EDGE, patterns_t::edge_negative_clamp());
      send_block("edge_corner_boundary", JPEG_FILTER_EDGE, patterns_t::corner_boundary());
      send_block("blur_corner_boundary", JPEG_FILTER_BLUR, patterns_t::corner_boundary());
      send_block("blur_top_edge_boundary", JPEG_FILTER_BLUR, patterns_t::top_edge_boundary());
      send_block("backpressure_first_beat", JPEG_FILTER_IDENTITY, patterns_t::checkerboard(),
                 1'b1, 0, 3);
      send_block("backpressure_middle_beat", JPEG_FILTER_SHARPEN, patterns_t::checkerboard(),
                 1'b1, (EXPECTED_BEATS > 2) ? (EXPECTED_BEATS / 2) : 0, 4);
      send_block("backpressure_final_beat", JPEG_FILTER_EDGE, patterns_t::ramp(),
                 1'b1, EXPECTED_BEATS - 1, 5);
    endtask
  endclass

  class jpeg_filter_sequencer #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_sequencer #(jpeg_filter_seq_item #(SAMPLE_WIDTH, BLOCK_DIM, LANES));
    `uvm_component_param_utils(jpeg_filter_sequencer #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction
  endclass

  class jpeg_filter_driver #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_driver #(jpeg_filter_seq_item #(SAMPLE_WIDTH, BLOCK_DIM, LANES));
    typedef jpeg_filter_seq_item #(SAMPLE_WIDTH, BLOCK_DIM, LANES) item_t;

    virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES) vif;

    `uvm_component_param_utils(jpeg_filter_driver #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      if (!uvm_config_db #(virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES))::get(this, "", "vif", vif)) begin
        `uvm_fatal(get_type_name(), $sformatf("Missing virtual interface for LANES=%0d", LANES))
      end
    endfunction

    task drive_idle();
      vif.in_valid   <= 1'b0;
      vif.block_data <= '0;
      vif.filter_mode <= JPEG_FILTER_IDENTITY;
      vif.out_ready  <= 1'b1;
    endtask

    task wait_reset_release();
      while (vif.rst_n !== 1'b1) begin
        drive_idle();
        @(posedge vif.clk);
      end
    endtask

    task drive_item(item_t req);
      int unsigned beat_count;
      int unsigned stall_remaining;
      bit stall_started;

      wait_reset_release();
      repeat (req.pre_accept_delay) @(negedge vif.clk);

      vif.block_data  <= req.block_data;
      vif.filter_mode <= req.filter_mode;
      vif.in_valid    <= 1'b1;

      do begin
        @(posedge vif.clk);
        if (vif.rst_n !== 1'b1) begin
          drive_idle();
          wait_reset_release();
          return;
        end
      end while (!vif.in_ready);

      vif.in_valid   <= 1'b0;
      vif.block_data <= '0;
      vif.filter_mode <= JPEG_FILTER_IDENTITY;

      beat_count = 0;
      stall_remaining = 0;
      stall_started = 1'b0;

      forever begin
        @(negedge vif.clk);
        if (vif.rst_n !== 1'b1) begin
          drive_idle();
          wait_reset_release();
          return;
        end

        if (req.enable_backpressure && !stall_started &&
            vif.out_valid && beat_count == req.stall_after_beat) begin
          stall_started = 1'b1;
          stall_remaining = req.stall_cycles;
        end

        if (stall_remaining > 0) begin
          vif.out_ready <= 1'b0;
          stall_remaining--;
        end else begin
          vif.out_ready <= 1'b1;
        end

        @(posedge vif.clk);
        if (vif.rst_n === 1'b1 && vif.out_valid && vif.out_ready) begin
          beat_count++;
          if (vif.out_last) begin
            break;
          end
        end
      end

      vif.out_ready <= 1'b1;
    endtask

    task run_phase(uvm_phase phase);
      item_t req;

      drive_idle();
      forever begin
        seq_item_port.get_next_item(req);
        `uvm_info(get_type_name(), {"Driving ", req.convert2string()}, UVM_MEDIUM)
        drive_item(req);
        seq_item_port.item_done();
      end
    endtask
  endclass

  class jpeg_filter_monitor #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_monitor;
    typedef jpeg_filter_in_txn #(SAMPLE_WIDTH, BLOCK_DIM, LANES) in_txn_t;
    typedef jpeg_filter_out_beat #(SAMPLE_WIDTH, BLOCK_DIM, LANES) out_beat_t;

    virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES) vif;
    uvm_analysis_port #(in_txn_t) in_ap;
    uvm_analysis_port #(out_beat_t) out_ap;

    bit stalled_before_handshake;

    `uvm_component_param_utils(jpeg_filter_monitor #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name, uvm_component parent);
      super.new(name, parent);
      in_ap = new("in_ap", this);
      out_ap = new("out_ap", this);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      if (!uvm_config_db #(virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES))::get(this, "", "vif", vif)) begin
        `uvm_fatal(get_type_name(), $sformatf("Missing virtual interface for LANES=%0d", LANES))
      end
    endfunction

    task run_phase(uvm_phase phase);
      in_txn_t in_txn;
      out_beat_t out_beat;

      stalled_before_handshake = 1'b0;
      forever begin
        @(posedge vif.clk);

        if (vif.rst_n !== 1'b1) begin
          stalled_before_handshake = 1'b0;
          continue;
        end

        if (vif.in_valid && vif.in_ready) begin
          in_txn = in_txn_t::type_id::create("in_txn", this);
          in_txn.block_data = vif.block_data;
          in_txn.filter_mode = vif.filter_mode;
          in_ap.write(in_txn);
        end

        if (vif.out_valid && !vif.out_ready) begin
          stalled_before_handshake = 1'b1;
        end

        if (vif.out_valid && vif.out_ready) begin
          out_beat = out_beat_t::type_id::create("out_beat", this);
          out_beat.sample_data = vif.sample_data;
          out_beat.sample_keep = vif.sample_keep;
          out_beat.out_last = vif.out_last;
          out_beat.stalled_before_handshake = stalled_before_handshake;
          out_ap.write(out_beat);
          stalled_before_handshake = 1'b0;
        end
      end
    endtask
  endclass

  class jpeg_filter_scoreboard #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_scoreboard;
    localparam int BLOCK_SAMPLES = BLOCK_DIM * BLOCK_DIM;
    localparam int BLOCK_BITS    = BLOCK_SAMPLES * SAMPLE_WIDTH;
    localparam int SAMPLE_BITS   = LANES * SAMPLE_WIDTH;
    localparam int EXPECTED_BEATS = (BLOCK_SAMPLES + LANES - 1) / LANES;

    typedef bit [SAMPLE_WIDTH-1:0] sample_t;
    typedef bit [BLOCK_BITS-1:0]   block_t;
    typedef jpeg_filter_ref_model #(SAMPLE_WIDTH, BLOCK_DIM, LANES) ref_t;
    typedef jpeg_filter_in_txn #(SAMPLE_WIDTH, BLOCK_DIM, LANES) in_txn_t;
    typedef jpeg_filter_out_beat #(SAMPLE_WIDTH, BLOCK_DIM, LANES) out_beat_t;

    typedef struct {
      block_t block_data;
      bit [1:0] filter_mode;
      int unsigned beat_index;
    } expected_block_t;

    virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES) vif;
    uvm_analysis_imp_in #(in_txn_t, jpeg_filter_scoreboard #(SAMPLE_WIDTH, BLOCK_DIM, LANES)) in_export;
    uvm_analysis_imp_out #(out_beat_t, jpeg_filter_scoreboard #(SAMPLE_WIDTH, BLOCK_DIM, LANES)) out_export;

    expected_block_t expected_q[$];
    int unsigned checked_blocks;
    int unsigned checked_beats;
    int unsigned checked_valid_samples;
    int unsigned checked_invalid_lanes;
    int unsigned stalled_checked_beats;
    int unsigned partial_final_beats;
    int unsigned checked_mode_count[4];

    `uvm_component_param_utils(jpeg_filter_scoreboard #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name, uvm_component parent);
      int unsigned mode;

      super.new(name, parent);
      in_export = new("in_export", this);
      out_export = new("out_export", this);
      checked_blocks = 0;
      checked_beats = 0;
      checked_valid_samples = 0;
      checked_invalid_lanes = 0;
      stalled_checked_beats = 0;
      partial_final_beats = 0;
      for (mode = 0; mode < 4; mode++) begin
        checked_mode_count[mode] = 0;
      end
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      if (!uvm_config_db #(virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES))::get(this, "", "vif", vif)) begin
        `uvm_fatal(get_type_name(), $sformatf("Missing virtual interface for LANES=%0d", LANES))
      end
    endfunction

    function void write_in(in_txn_t txn);
      expected_block_t expected;

      expected.block_data = txn.block_data;
      expected.filter_mode = txn.filter_mode;
      expected.beat_index = 0;
      expected_q.push_back(expected);
      `uvm_info(get_type_name(),
                $sformatf("Accepted input block mode=%0d lanes=%0d expected_beats=%0d queue=%0d",
                          txn.filter_mode, LANES, EXPECTED_BEATS, expected_q.size()),
                UVM_HIGH)
    endfunction

    function void write_out(out_beat_t beat);
      expected_block_t expected;
      int unsigned lane;
      int unsigned global_index;
      bit expected_keep;
      bit expected_last;
      sample_t expected_sample;
      sample_t actual_sample;
      int unsigned mode_index;

      if (expected_q.size() == 0) begin
        `uvm_error(get_type_name(), $sformatf("Unexpected output beat for LANES=%0d with no accepted input", LANES))
        return;
      end

      expected = expected_q[0];
      expected_last = (expected.beat_index == EXPECTED_BEATS - 1);

      if (beat.out_last !== expected_last) begin
        `uvm_error(get_type_name(),
                   $sformatf("out_last mismatch lanes=%0d beat=%0d actual=%0b expected=%0b",
                             LANES, expected.beat_index, beat.out_last, expected_last))
      end

      for (lane = 0; lane < LANES; lane++) begin
        global_index = expected.beat_index * LANES + lane;
        expected_keep = (global_index < BLOCK_SAMPLES);

        if (beat.sample_keep[lane] !== expected_keep) begin
          `uvm_error(get_type_name(),
                     $sformatf("sample_keep mismatch lanes=%0d beat=%0d lane=%0d actual=%0b expected=%0b",
                               LANES, expected.beat_index, lane, beat.sample_keep[lane], expected_keep))
        end

        actual_sample = sample_t'(beat.sample_data[lane*SAMPLE_WIDTH +: SAMPLE_WIDTH]);
        if (expected_keep) begin
          checked_valid_samples++;
          expected_sample = ref_t::filter_sample(expected.block_data, expected.filter_mode, global_index);
          if (actual_sample !== expected_sample) begin
            `uvm_error(get_type_name(),
                       $sformatf("sample mismatch lanes=%0d mode=%0d beat=%0d lane=%0d index=%0d actual=%0d expected=%0d",
                                 LANES, expected.filter_mode, expected.beat_index, lane, global_index,
                                 actual_sample, expected_sample))
          end
        end else if (actual_sample !== '0) begin
          checked_invalid_lanes++;
          `uvm_error(get_type_name(),
                     $sformatf("invalid final lane not zero lanes=%0d beat=%0d lane=%0d actual=%0d",
                               LANES, expected.beat_index, lane, actual_sample))
        end else begin
          checked_invalid_lanes++;
        end
      end

      checked_beats++;
      if (beat.stalled_before_handshake) begin
        stalled_checked_beats++;
      end
      if (expected_last) begin
        if (beat.sample_keep !== {LANES{1'b1}}) begin
          partial_final_beats++;
        end
        mode_index = int'(expected.filter_mode);
        checked_mode_count[mode_index]++;
        checked_blocks++;
        expected_q.pop_front();
      end else begin
        expected_q[0].beat_index++;
      end
    endfunction

    task run_phase(uvm_phase phase);
      forever begin
        @(negedge vif.rst_n);
        if (expected_q.size() != 0) begin
          `uvm_info(get_type_name(),
                    $sformatf("Reset flushed %0d pending expected block(s) for LANES=%0d", expected_q.size(), LANES),
                    UVM_LOW)
          expected_q.delete();
        end
      end
    endtask

    function void check_phase(uvm_phase phase);
      int unsigned mode;

      super.check_phase(phase);
      if (expected_q.size() != 0) begin
        `uvm_error(get_type_name(),
                   $sformatf("Simulation ended with %0d pending expected block(s) for LANES=%0d",
                             expected_q.size(), LANES))
      end
      if (checked_blocks == 0) begin
        `uvm_error(get_type_name(), $sformatf("No completed blocks checked for LANES=%0d", LANES))
      end
      if (checked_blocks < 2) begin
        `uvm_error(get_type_name(), $sformatf("Fewer than two completed blocks checked for LANES=%0d", LANES))
      end
      if (checked_beats != (checked_blocks * EXPECTED_BEATS)) begin
        `uvm_error(get_type_name(),
                   $sformatf("Beat KPI mismatch for LANES=%0d checked_beats=%0d checked_blocks=%0d expected_beats_per_block=%0d",
                             LANES, checked_beats, checked_blocks, EXPECTED_BEATS))
      end
      if (checked_valid_samples != (checked_blocks * BLOCK_SAMPLES)) begin
        `uvm_error(get_type_name(),
                   $sformatf("Valid sample KPI mismatch for LANES=%0d checked_samples=%0d checked_blocks=%0d block_samples=%0d",
                             LANES, checked_valid_samples, checked_blocks, BLOCK_SAMPLES))
      end
      if (stalled_checked_beats == 0) begin
        `uvm_error(get_type_name(), $sformatf("No stalled output beat observed for LANES=%0d", LANES))
      end
      if ((BLOCK_SAMPLES % LANES) == 0) begin
        if (partial_final_beats != 0) begin
          `uvm_error(get_type_name(),
                     $sformatf("Unexpected partial final beat for evenly-dividing LANES=%0d count=%0d",
                               LANES, partial_final_beats))
        end
      end else if (partial_final_beats != checked_blocks) begin
        `uvm_error(get_type_name(),
                   $sformatf("Partial final beat count mismatch for LANES=%0d partial=%0d checked_blocks=%0d",
                             LANES, partial_final_beats, checked_blocks))
      end
      for (mode = 0; mode < 4; mode++) begin
        if (checked_mode_count[mode] == 0) begin
          `uvm_error(get_type_name(),
                     $sformatf("No completed block checked for LANES=%0d filter_mode=%0d", LANES, mode))
        end
      end
      `uvm_info(get_type_name(),
                $sformatf("Checked %0d block(s), %0d beat(s), %0d valid sample(s), %0d invalid lane(s), %0d stalled beat(s), %0d partial final beat(s), expected beats per block=%0d for LANES=%0d",
                          checked_blocks, checked_beats, checked_valid_samples, checked_invalid_lanes,
                          stalled_checked_beats, partial_final_beats, EXPECTED_BEATS, LANES),
                UVM_LOW)
    endfunction
  endclass

  class jpeg_filter_coverage #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_subscriber #(jpeg_filter_out_beat #(SAMPLE_WIDTH, BLOCK_DIM, LANES));
    localparam int BLOCK_SAMPLES = BLOCK_DIM * BLOCK_DIM;
    localparam int BLOCK_BITS    = BLOCK_SAMPLES * SAMPLE_WIDTH;

    typedef jpeg_filter_in_txn #(SAMPLE_WIDTH, BLOCK_DIM, LANES) in_txn_t;
    typedef jpeg_filter_out_beat #(SAMPLE_WIDTH, BLOCK_DIM, LANES) out_beat_t;
    typedef jpeg_filter_ref_model #(SAMPLE_WIDTH, BLOCK_DIM, LANES) ref_t;
    typedef bit [SAMPLE_WIDTH-1:0] sample_t;

    uvm_analysis_imp_in #(in_txn_t, jpeg_filter_coverage #(SAMPLE_WIDTH, BLOCK_DIM, LANES)) in_export;

    covergroup input_cg with function sample(int unsigned mode, bit all_zero, bit all_max,
                                             bit corner_active, bit edge_active);
      option.per_instance = 1;
      cp_mode: coverpoint mode {
        bins identity = {JPEG_FILTER_IDENTITY};
        bins blur     = {JPEG_FILTER_BLUR};
        bins sharpen  = {JPEG_FILTER_SHARPEN};
        bins edge_mode = {JPEG_FILTER_EDGE};
      }
      cp_all_zero: coverpoint all_zero;
      cp_all_max: coverpoint all_max;
      cp_corner_active: coverpoint corner_active;
      cp_edge_active: coverpoint edge_active;
      mode_x_corner: cross cp_mode, cp_corner_active;
      mode_x_edge: cross cp_mode, cp_edge_active;
    endgroup

    covergroup output_cg with function sample(int unsigned lane_count, int unsigned keep_count,
                                              bit partial_beat, bit out_last,
                                              bit stalled_before_handshake);
      option.per_instance = 1;
      cp_lane_count: coverpoint lane_count {
        bins lanes_1  = {1};
        bins lanes_5  = {5};
        bins lanes_8  = {8};
        bins lanes_64 = {64};
      }
      cp_keep_count: coverpoint keep_count {
        bins legal[] = {[0:LANES]};
      }
      cp_partial_beat: coverpoint partial_beat;
      cp_out_last: coverpoint out_last;
      cp_stalled: coverpoint stalled_before_handshake;
      last_x_partial: cross cp_out_last, cp_partial_beat;
      last_x_stall: cross cp_out_last, cp_stalled;
    endgroup

    `uvm_component_param_utils(jpeg_filter_coverage #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name, uvm_component parent);
      super.new(name, parent);
      in_export = new("in_export", this);
      input_cg = new();
      output_cg = new();
    endfunction

    function void write_in(in_txn_t txn);
      int unsigned index;
      bit all_zero;
      bit all_max;
      bit corner_active;
      bit edge_active;
      sample_t sample;
      sample_t max_value;

      all_zero = 1'b1;
      all_max = 1'b1;
      corner_active = 1'b0;
      edge_active = 1'b0;
      max_value = sample_t'((1 << SAMPLE_WIDTH) - 1);

      for (index = 0; index < BLOCK_SAMPLES; index++) begin
        sample = ref_t::get_sample(txn.block_data, index);
        all_zero &= (sample == '0);
        all_max &= (sample == max_value);
        if ((index == 0) || (index == BLOCK_DIM - 1) ||
            (index == BLOCK_SAMPLES - BLOCK_DIM) || (index == BLOCK_SAMPLES - 1)) begin
          corner_active |= (sample != '0);
        end
        if ((index < BLOCK_DIM) || (index >= BLOCK_SAMPLES - BLOCK_DIM) ||
            ((index % BLOCK_DIM) == 0) || ((index % BLOCK_DIM) == BLOCK_DIM - 1)) begin
          edge_active |= (sample != '0);
        end
      end

      input_cg.sample(txn.filter_mode, all_zero, all_max, corner_active, edge_active);
    endfunction

    function void write(out_beat_t beat);
      int unsigned lane;
      int unsigned keep_count;

      keep_count = 0;
      for (lane = 0; lane < LANES; lane++) begin
        keep_count += beat.sample_keep[lane];
      end

      output_cg.sample(LANES, keep_count, keep_count < LANES, beat.out_last,
                       beat.stalled_before_handshake);
    endfunction
  endclass

  class jpeg_filter_agent #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_agent;
    typedef jpeg_filter_sequencer #(SAMPLE_WIDTH, BLOCK_DIM, LANES) sequencer_t;
    typedef jpeg_filter_driver #(SAMPLE_WIDTH, BLOCK_DIM, LANES) driver_t;
    typedef jpeg_filter_monitor #(SAMPLE_WIDTH, BLOCK_DIM, LANES) monitor_t;

    sequencer_t sequencer;
    driver_t driver;
    monitor_t monitor;

    `uvm_component_param_utils(jpeg_filter_agent #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      sequencer = sequencer_t::type_id::create("sequencer", this);
      driver = driver_t::type_id::create("driver", this);
      monitor = monitor_t::type_id::create("monitor", this);
    endfunction

    function void connect_phase(uvm_phase phase);
      super.connect_phase(phase);
      driver.seq_item_port.connect(sequencer.seq_item_export);
    endfunction
  endclass

  class jpeg_filter_env #(
    parameter int SAMPLE_WIDTH = 8,
    parameter int BLOCK_DIM    = 8,
    parameter int LANES        = 8
  ) extends uvm_env;
    typedef jpeg_filter_agent #(SAMPLE_WIDTH, BLOCK_DIM, LANES) agent_t;
    typedef jpeg_filter_scoreboard #(SAMPLE_WIDTH, BLOCK_DIM, LANES) scoreboard_t;
    typedef jpeg_filter_coverage #(SAMPLE_WIDTH, BLOCK_DIM, LANES) coverage_t;

    virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES) vif;

    agent_t agent;
    scoreboard_t scoreboard;
    coverage_t coverage;

    `uvm_component_param_utils(jpeg_filter_env #(SAMPLE_WIDTH, BLOCK_DIM, LANES))

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);
      if (!uvm_config_db #(virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES))::get(this, "", "vif", vif)) begin
        `uvm_fatal(get_type_name(), $sformatf("Missing virtual interface for LANES=%0d", LANES))
      end
      uvm_config_db #(virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES))::set(this, "agent.*", "vif", vif);
      uvm_config_db #(virtual jpeg_filter_if #(SAMPLE_WIDTH, BLOCK_DIM, LANES))::set(this, "scoreboard", "vif", vif);
      agent = agent_t::type_id::create("agent", this);
      scoreboard = scoreboard_t::type_id::create("scoreboard", this);
      coverage = coverage_t::type_id::create("coverage", this);
    endfunction

    function void connect_phase(uvm_phase phase);
      super.connect_phase(phase);
      agent.monitor.in_ap.connect(scoreboard.in_export);
      agent.monitor.out_ap.connect(scoreboard.out_export);
      agent.monitor.in_ap.connect(coverage.in_export);
      agent.monitor.out_ap.connect(coverage.analysis_export);
    endfunction
  endclass

  class jpeg_filter_all_lanes_test extends uvm_test;
    typedef jpeg_filter_env #(8, 8, 1) env_lanes1_t;
    typedef jpeg_filter_env #(8, 8, 5) env_lanes5_t;
    typedef jpeg_filter_env #(8, 8, 8) env_lanes8_t;
    typedef jpeg_filter_env #(8, 8, 64) env_lanes64_t;

    virtual jpeg_filter_if #(8, 8, 1)  vif_lanes1;
    virtual jpeg_filter_if #(8, 8, 5)  vif_lanes5;
    virtual jpeg_filter_if #(8, 8, 8)  vif_lanes8;
    virtual jpeg_filter_if #(8, 8, 64) vif_lanes64;

    env_lanes1_t  env_lanes1;
    env_lanes5_t  env_lanes5;
    env_lanes8_t  env_lanes8;
    env_lanes64_t env_lanes64;

    `uvm_component_utils(jpeg_filter_all_lanes_test)

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction

    function void build_phase(uvm_phase phase);
      super.build_phase(phase);

      if (!uvm_config_db #(virtual jpeg_filter_if #(8, 8, 1))::get(this, "", "vif_lanes1", vif_lanes1)) begin
        `uvm_fatal(get_type_name(), "Missing vif_lanes1")
      end
      if (!uvm_config_db #(virtual jpeg_filter_if #(8, 8, 5))::get(this, "", "vif_lanes5", vif_lanes5)) begin
        `uvm_fatal(get_type_name(), "Missing vif_lanes5")
      end
      if (!uvm_config_db #(virtual jpeg_filter_if #(8, 8, 8))::get(this, "", "vif_lanes8", vif_lanes8)) begin
        `uvm_fatal(get_type_name(), "Missing vif_lanes8")
      end
      if (!uvm_config_db #(virtual jpeg_filter_if #(8, 8, 64))::get(this, "", "vif_lanes64", vif_lanes64)) begin
        `uvm_fatal(get_type_name(), "Missing vif_lanes64")
      end

      uvm_config_db #(virtual jpeg_filter_if #(8, 8, 1))::set(this, "env_lanes1", "vif", vif_lanes1);
      uvm_config_db #(virtual jpeg_filter_if #(8, 8, 5))::set(this, "env_lanes5", "vif", vif_lanes5);
      uvm_config_db #(virtual jpeg_filter_if #(8, 8, 8))::set(this, "env_lanes8", "vif", vif_lanes8);
      uvm_config_db #(virtual jpeg_filter_if #(8, 8, 64))::set(this, "env_lanes64", "vif", vif_lanes64);

      env_lanes1  = env_lanes1_t::type_id::create("env_lanes1", this);
      env_lanes5  = env_lanes5_t::type_id::create("env_lanes5", this);
      env_lanes8  = env_lanes8_t::type_id::create("env_lanes8", this);
      env_lanes64 = env_lanes64_t::type_id::create("env_lanes64", this);
    endfunction

    task apply_reset_all(int unsigned cycles = 4);
      vif_lanes1.rst_n  <= 1'b0;
      vif_lanes5.rst_n  <= 1'b0;
      vif_lanes8.rst_n  <= 1'b0;
      vif_lanes64.rst_n <= 1'b0;
      repeat (cycles) @(posedge vif_lanes8.clk);
      vif_lanes1.rst_n  <= 1'b1;
      vif_lanes5.rst_n  <= 1'b1;
      vif_lanes8.rst_n  <= 1'b1;
      vif_lanes64.rst_n <= 1'b1;
      repeat (2) @(posedge vif_lanes8.clk);
    endtask

    task reset_during_active_output();
      jpeg_filter_single_block_seq #(8, 8, 8) seq;

      seq = jpeg_filter_single_block_seq #(8, 8, 8)::type_id::create("reset_active_seq");
      seq.block_data = jpeg_filter_patterns #(8, 8)::ramp();
      seq.filter_mode = JPEG_FILTER_BLUR;
      seq.scenario = "reset_during_active_output";

      fork
        seq.start(env_lanes8.agent.sequencer);
      join_none

      wait (vif_lanes8.out_valid === 1'b1);
      @(posedge vif_lanes8.clk);
      vif_lanes8.rst_n <= 1'b0;
      repeat (3) @(posedge vif_lanes8.clk);
      vif_lanes8.rst_n <= 1'b1;
      repeat (3) @(posedge vif_lanes8.clk);
      wait fork;
    endtask

    task reset_during_stalled_output();
      jpeg_filter_single_block_seq #(8, 8, 5) seq;

      seq = jpeg_filter_single_block_seq #(8, 8, 5)::type_id::create("reset_stalled_seq");
      seq.block_data = jpeg_filter_patterns #(8, 8)::checkerboard();
      seq.filter_mode = JPEG_FILTER_SHARPEN;
      seq.scenario = "reset_during_stalled_output";
      seq.enable_backpressure = 1'b1;
      seq.stall_after_beat = 0;
      seq.stall_cycles = 8;

      fork
        seq.start(env_lanes5.agent.sequencer);
      join_none

      wait (vif_lanes5.out_valid === 1'b1 && vif_lanes5.out_ready === 1'b0);
      @(posedge vif_lanes5.clk);
      vif_lanes5.rst_n <= 1'b0;
      repeat (3) @(posedge vif_lanes5.clk);
      vif_lanes5.rst_n <= 1'b1;
      repeat (3) @(posedge vif_lanes5.clk);
      wait fork;
    endtask

    task run_phase(uvm_phase phase);
      jpeg_filter_scenario_seq #(8, 8, 1)  seq_lanes1;
      jpeg_filter_scenario_seq #(8, 8, 5)  seq_lanes5;
      jpeg_filter_scenario_seq #(8, 8, 8)  seq_lanes8;
      jpeg_filter_scenario_seq #(8, 8, 64) seq_lanes64;

      phase.raise_objection(this);

      apply_reset_all();
      reset_during_active_output();
      reset_during_stalled_output();

      seq_lanes1  = jpeg_filter_scenario_seq #(8, 8, 1)::type_id::create("seq_lanes1");
      seq_lanes5  = jpeg_filter_scenario_seq #(8, 8, 5)::type_id::create("seq_lanes5");
      seq_lanes8  = jpeg_filter_scenario_seq #(8, 8, 8)::type_id::create("seq_lanes8");
      seq_lanes64 = jpeg_filter_scenario_seq #(8, 8, 64)::type_id::create("seq_lanes64");

      fork
        seq_lanes1.start(env_lanes1.agent.sequencer);
        seq_lanes5.start(env_lanes5.agent.sequencer);
        seq_lanes8.start(env_lanes8.agent.sequencer);
        seq_lanes64.start(env_lanes64.agent.sequencer);
      join

      repeat (20) @(posedge vif_lanes8.clk);
      phase.drop_objection(this);
    endtask

    function void report_lane_kpi(
      int unsigned lanes,
      int unsigned expected_beats,
      int unsigned checked_beats,
      int unsigned checked_blocks
    );
      int unsigned observed_beats;
      string status;
      bit pass;

      observed_beats = (checked_blocks == 0) ? 0 : (checked_beats / checked_blocks);
      pass = (checked_blocks > 0) &&
             (checked_beats == (checked_blocks * expected_beats)) &&
             (observed_beats == expected_beats);
      status = pass ? "PASS" : "FAIL";

      if (!pass) begin
        `uvm_error(get_type_name(),
                   $sformatf("KPI failure LANES=%0d expected_beats=%0d checked_beats=%0d checked_blocks=%0d observed_beats_per_block=%0d",
                             lanes, expected_beats, checked_beats, checked_blocks, observed_beats))
      end

      $display("JPEG_FILTER_KPI LANES=%0d EXPECTED_BEATS=%0d OBSERVED_BEATS=%0d CHECKED_BLOCKS=%0d %s",
               lanes, expected_beats, observed_beats, checked_blocks, status);
    endfunction

    function void final_phase(uvm_phase phase);
      uvm_report_server server;

      super.final_phase(phase);
      report_lane_kpi(1,  64, env_lanes1.scoreboard.checked_beats,  env_lanes1.scoreboard.checked_blocks);
      report_lane_kpi(5,  13, env_lanes5.scoreboard.checked_beats,  env_lanes5.scoreboard.checked_blocks);
      report_lane_kpi(8,  8,  env_lanes8.scoreboard.checked_beats,  env_lanes8.scoreboard.checked_blocks);
      report_lane_kpi(64, 1,  env_lanes64.scoreboard.checked_beats, env_lanes64.scoreboard.checked_blocks);

      server = uvm_report_server::get_server();
      if (server.get_severity_count(UVM_ERROR) == 0 &&
          server.get_severity_count(UVM_FATAL) == 0) begin
        $display("JPEG_FILTER_APPLIER_TEST_PASS");
      end
    endfunction
  endclass
endpackage
