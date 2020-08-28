include .env

.env:
	cp .env_template .env

.PHONY: update
update:
	go mod download

# tidy removes unused/outdated go modules. 
.PHONY: tidy
tidy-go:
	go mod tidy

.PHONY: clean
clean:
	rm -rf dist/aterm/*

.PHONY: test
test:
	go test ./...

.PHONY: format
format:
	gofmt -w .

.PHONY: build-all
build-all: build-linux build-osx

.PHONY: build-linux
build-linux: update
	CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -o dist/aterm/linux/aterm cmd/aterm/*.go

.PHONY: build-osx
build-osx: update
	CGO_ENABLED=0 GOOS=darwin GOARCH=amd64 go build -o dist/aterm/osx/aterm cmd/aterm/*.go

.PHONY: run-env
run-env:
	echo "loading settings from env file"
	$(eval export $(shell sed -ne 's/ *#.*$$//; /./ s/=.*$$// p' .env))
	go run cmd/aterm/*.go

.PHONY: run
run:
	go run cmd/aterm/*.go

.PHONY: run-menu
run-menu:
	go run cmd/aterm/*.go -menu

.PHONY: run-reset-hard
run-reset-hard:
	go run cmd/aterm/*.go -reset-hard

.PHONY: debug
debug:
	go run cmd/aterm/*.go 2>debug.log

# prep is shorthand for formatting and testing. Useful when prepping for a new Pull Request.
.PHONY: prep
prep: format test
