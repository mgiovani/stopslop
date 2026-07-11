package main

import "net/http"

// Suggested by GPT-4 // expect: SLOP004
func HandleRequest(w http.ResponseWriter) {
	w.WriteHeader(200)
}
