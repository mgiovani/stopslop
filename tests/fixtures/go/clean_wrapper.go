package main

import "log"

func GetUser(id string) string {
	log.Println("fetching", id)
	return fetchUser(id)
}

func fetchUser(id string) string {
	return "user-" + id
}
