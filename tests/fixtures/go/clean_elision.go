package main

type Reader interface {
	Read(p []byte) (n int, err error)
}

// process the rest of the data
func Load() []byte {
	return nil
}
