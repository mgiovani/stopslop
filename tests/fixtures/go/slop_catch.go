package handlers

import "fmt"

func riskyOp() error {
	return fmt.Errorf("boom")
}

func emptyErrCheck() error {
	err := riskyOp()
	if err != nil { // expect: SLOP005
	}
	return nil
}

func recoverSwallow() {
	defer func() {
		if err := recover(); err != nil { // expect: SLOP005
		}
	}()
	_ = riskyOp()
}
