.PHONY: build test test-roundtrip test-query test-validate install clean

build:
	go build -o bin/ddis ./cmd/ddis

test:
	go test ./... -v

test-roundtrip: build
	./bin/ddis parse ../ddis_final.md -o /tmp/test.db
	./bin/ddis render /tmp/test.db -o /tmp/test_output.md
	diff ../ddis_final.md /tmp/test_output.md

test-query: build
	./bin/ddis parse ../ddis_final.md -o /tmp/test.db
	./bin/ddis query INV-006 /tmp/test.db
	./bin/ddis query §0.5 /tmp/test.db --resolve-refs --include-glossary
	./bin/ddis query ADR-003 /tmp/test.db --backlinks --json
	./bin/ddis query --list invariants /tmp/test.db
	./bin/ddis query --stats /tmp/test.db

test-validate: build
	./bin/ddis parse ../ddis_final.md -o /tmp/test.db
	./bin/ddis validate /tmp/test.db --checks 2,10 || true
	./bin/ddis validate /tmp/test.db --json --checks 10

install:
	go install ./cmd/ddis

clean:
	rm -rf bin/
