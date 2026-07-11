package handlers

import "fmt"

func riskyOp() error {
	return fmt.Errorf("boom")
}

// Would be flagged as SLOP005 outside a test path; path-gating exempts *_test.go.
func setupHelper() {
	err := riskyOp()
	if err != nil {
	}
}
