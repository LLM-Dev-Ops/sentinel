.PHONY: help build test lint fmt vet clean install examples

# Variables
BINARY_NAME=sentinel-sdk
GO=go
GOTEST=$(GO) test
GOVET=$(GO) vet
GOFMT=$(GO) fmt

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-15s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

install: ## Install dependencies
	$(GO) mod download
	$(GO) mod tidy

build: ## Build the SDK
	$(GO) build -v ./...

test: ## Run tests
	$(GOTEST) -v -race -coverprofile=coverage.out ./...
	$(GO) tool cover -html=coverage.out -o coverage.html

test-short: ## Run short tests
	$(GOTEST) -v -short ./...

bench: ## Run benchmarks
	$(GOTEST) -bench=. -benchmem ./...

lint: ## Run linters
	@which golangci-lint > /dev/null || (echo "golangci-lint not installed. Run: curl -sSfL https://raw.githubusercontent.com/golangci/golangci-lint/master/install.sh | sh -s -- -b $(go env GOPATH)/bin" && exit 1)
	golangci-lint run ./...

fmt: ## Format code
	$(GOFMT) ./...

vet: ## Run go vet
	$(GOVET) ./...

clean: ## Clean build artifacts
	$(GO) clean
	rm -f coverage.out coverage.html
	rm -f examples/basic examples/error_handling

examples: ## Build example binaries
	$(GO) build -o examples/basic examples/basic.go
	$(GO) build -o examples/error_handling examples/error_handling.go

run-example-basic: ## Run basic example
	$(GO) run examples/basic.go

run-example-errors: ## Run error handling example
	$(GO) run examples/error_handling.go

check: fmt vet test ## Run all checks (format, vet, test)

coverage: test ## Generate coverage report
	$(GO) tool cover -func=coverage.out

# Development helpers
watch-test: ## Watch and run tests on file changes (requires entr)
	find . -name '*.go' | entr -c make test-short

doc: ## Start local documentation server
	@echo "Opening documentation at http://localhost:6060/pkg/github.com/llm-sentinel/sentinel-sdk-go/sentinel/"
	godoc -http=:6060

.DEFAULT_GOAL := help
