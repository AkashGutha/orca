TOP ?= tb_tensor_unit

.PHONY: build sim waves verdi clean

build:
	TOP=$(TOP) ./scripts/run_vcs.sh build

sim:
	TOP=$(TOP) ./scripts/run_vcs.sh sim

waves:
	TOP=$(TOP) ./scripts/run_vcs.sh waves

verdi:
	TOP=$(TOP) ./scripts/run_verdi.sh

clean:
	./scripts/clean.sh
