PROG=chsi
build:
	cargo build --bin $(PROG)

release:
	cargo build --release --bin $(PROG)

test: build
	./rere.py replay test.list

record: build
	./rere.py record test.list

$(PROG): release

help:
	@echo "usage: make $(prog)"
