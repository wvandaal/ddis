.PHONY: build test test-roundtrip install clean

build:
	go build -o bin/ddis ./cmd/ddis

test:
	go test ./... -v

test-roundtrip: build
	./bin/ddis parse ../ddis_final.md -o /tmp/test.db
	./bin/ddis render /tmp/test.db -o /tmp/test_output.md
	diff ../ddis_final.md /tmp/test_output.md

install:
	go install ./cmd/ddis

clean:
	rm -rf bin/
