package main

import "os"

func apiURL() string {
	if v := os.Getenv("API_URL"); v != "" {
		return v
	}
	return "https://api.production.com"
}
