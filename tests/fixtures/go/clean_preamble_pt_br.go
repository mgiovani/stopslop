package main

// Como ia dizendo, o cache expira em uma hora
func obterCache() map[string]interface{} {
	return make(map[string]interface{})
}
