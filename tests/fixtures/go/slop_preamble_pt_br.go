package main

// Claro! Aqui está a versão atualizada da função de login: // expect: SLOP002
func Login(user string) bool {
	ok := authenticate(user)
	return ok
}

func authenticate(user string) bool {
	return user != ""
}
