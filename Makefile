include .env

DEBUGGING_FLAG=-gcflags="all=-N -l"
DEV_VERSION_FLAG=-X github.com/jrozner/go-info.version=v0.0.0-development
DEV_COMMIT_FLAG=-X github.com/jrozner/go-info.commitHash=$(shell git rev-list -1 HEAD)
DEV_DATE_FLAG=-X github.com/jrozner/go-info.buildDate=$(shell date -u +'%Y-%m-%dT%H:%M:%SZ')
DEV_REPO_FLAG=-X github.com/theparanoids/aterm/cmd/aterm/config.codeRepoRaw=theparanoids/aterm
LD_FLAGS=-ldflags "$(DEV_VERSION_FLAG) $(DEV_COMMIT_FLAG) $(DEV_DATE_FLAG) $(DEV_REPO_FLAG)"

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

.PHONY: build-for-debug
build-for-debug:
	CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build $(LD_FLAGS) ${DEBUGGING_FLAG} -o dist/aterm/debug/aterm cmd/aterm/*.go

.PHONY: build-test
build-test: update
	CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build $(LD_FLAGS) -o dist/aterm/linux/aterm cmd/aterm/*.go

.PHONY: build-linux
build-linux: update
	CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build $(LD_FLAGS) -o dist/aterm/linux/aterm cmd/aterm/*.go

.PHONY: build-osx
build-osx: update
	CGO_ENABLED=0 GOOS=darwin GOARCH=amd64 go build $(LD_FLAGS) -o dist/aterm/osx/aterm cmd/aterm/*.go

.PHONY: run-env
run-env:
	echo "loading settings from env file"
	$(eval export $(shell sed -ne 's/ *#.*$$//; /./ s/=.*$$// p' .env))
	go run cmd/aterm/*.go

.PHONY: run
run:
	go run $(LD_FLAGS) cmd/aterm/*.go

.PHONY: run-menu
run-menu:
	go run $(LD_FLAGS) cmd/aterm/*.go -menu

.PHONY: run-version
run-version:
	go run $(LD_FLAGS) cmd/aterm/*.go -version

.PHONY: run-reset
run-reset:
	go run $(LD_FLAGS) cmd/aterm/*.go -reset

.PHONY: run-reset-hard
run-reset-hard:
	go run $(LD_FLAGS) cmd/aterm/*.go -reset-hard

.PHONY: run-help
run-help:
	go run $(LD_FLAGS) cmd/aterm/*.go -h

.PHONY: debug
debug:
	go run $(LD_FLAGS) cmd/aterm/*.go 2>debug.log

# prep is shorthand for formatting and testing. Useful when prepping for a new Pull Request.
.PHONY: prep
prep: format test

.PHONY: debug-menu
debug-menu: build-for-debug
	dist/aterm/debug/aterm -menu -pid

.PHONY: debug-run
debug-run: build-for-debug
	dist/aterm/debug/aterm -pid
