package main

type Leitor interface {
	Read(p []byte) (n int, err error)
}

// resto da lógica fica no módulo de auth
func Carregar() []byte {
	return nil
}
