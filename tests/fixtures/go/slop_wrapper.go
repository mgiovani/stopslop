package main

func GetUser(id string) string { // expect: SLOP039
	return fetchUser(id)
}

func fetchUser(id string) string {
	return "user-" + id
}
