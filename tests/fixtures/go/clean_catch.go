package handlers

import "fmt"

func riskyOp() error {
	return fmt.Errorf("boom")
}

func propagateErr() error {
	err := riskyOp()
	if err != nil {
		return err
	}
	return nil
}

func recoveryDefault() int {
	err := riskyOp()
	if err != nil {
		return -1
	}
	return 0
}

func unrelatedEmptyIf(count int) {
	if count > 10 {
	}
}
