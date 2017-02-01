all: run_example examples

.PHONY: examples
examples:
	for file in examples/*.rs; do \
		cargo build --example $$(basename $${file%.*}); \
	done

