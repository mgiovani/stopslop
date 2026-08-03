package main

type Reader interface {
	Read(p []byte) (n int, err error)
}

func Process(data []byte) []byte {
	return data
}

func Placeholder() {
	// TODO(dana): implement full logic
}
