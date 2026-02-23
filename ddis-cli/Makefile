.PHONY: build test test-roundtrip test-query test-validate test-diff test-impact test-oplog test-tx install clean

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

test-diff: build
	./bin/ddis parse ../ddis_final.md -o /tmp/base.db
	cp /tmp/base.db /tmp/head.db
	./bin/ddis diff /tmp/base.db /tmp/head.db
	./bin/ddis diff /tmp/base.db /tmp/head.db --json

test-impact: build
	./bin/ddis parse ../ddis_final.md -o /tmp/test.db
	./bin/ddis impact INV-006 /tmp/test.db
	./bin/ddis impact §0.5 /tmp/test.db --direction backward
	./bin/ddis impact ADR-003 /tmp/test.db --depth 3 --json

test-oplog: build
	./bin/ddis parse ../ddis_final.md -o /tmp/test.db
	rm -f /tmp/test-oplog.jsonl
	./bin/ddis seed /tmp/test.db --oplog-path /tmp/test-oplog.jsonl
	./bin/ddis log /tmp/test-oplog.jsonl
	./bin/ddis validate /tmp/test.db --log --oplog-path /tmp/test-oplog.jsonl || true
	./bin/ddis log /tmp/test-oplog.jsonl --type validate

test-tx: build
	./bin/ddis parse ../ddis_final.md -o /tmp/test.db
	./bin/ddis tx begin /tmp/test.db "Test transaction" > /tmp/tx_id.txt
	./bin/ddis tx list /tmp/test.db
	./bin/ddis tx commit $$(cat /tmp/tx_id.txt) /tmp/test.db --oplog /tmp/test-oplog.jsonl

install:
	go install ./cmd/ddis

clean:
	rm -rf bin/
