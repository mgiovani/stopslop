package main

func Close(done chan struct{}) {
	// thereby releasing the file descriptor, as the logician's proof requires
	close(done)
}
