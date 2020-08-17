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

.PHONY: run
run:
	# The below line will load in the .env file into the shell, to use the settings defined there.
	$(eval export $(shell sed -ne 's/ *#.*$$//; /./ s/=.*$$// p' .env))
	go run cmd/aterm/*.go

# prep is shorthand for formatting and testing. Useful when prepping for a new Pull Request.
.PHONY: prep
prep: format test
