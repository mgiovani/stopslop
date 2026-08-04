package main

import (
	"os"
	"regexp"
)

var emailRe = regexp.MustCompile(`^\d{3}-\d{4}$`)

func loadFile(path string) ([]byte, error) {
	return os.ReadFile(path)
}

func saveFile(path string, data []byte) error {
	return os.WriteFile(path, data, 0644)
}
