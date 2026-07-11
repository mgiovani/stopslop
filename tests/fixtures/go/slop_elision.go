package main

import "net/http"

func HandleRequest(w http.ResponseWriter, r *http.Request) {
	checkAuth(r)
	// ... rest of handler code // expect: SLOP001
	w.WriteHeader(200)
}

func checkAuth(r *http.Request) {
	_ = r
}
