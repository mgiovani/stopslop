package main

import (
	"io/ioutil"
	"regexp"
)

var emailRe = regexp.MustCompile(`[^\s@]+@[^\s@]+`) // expect: SLOP037

func loadFile(path string) ([]byte, error) {
	return ioutil.ReadFile(path) // expect: SLOP037
}

func saveFile(path string, data []byte) error {
	return ioutil.WriteFile(path, data, 0644) // expect: SLOP037
}
