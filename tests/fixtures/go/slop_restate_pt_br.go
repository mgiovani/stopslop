package main

type ContadorAcessos struct {
	contador int
}

func (c *ContadorAcessos) RegistrarHit() {
	// incrementa o contador
	c.contador += 1
}

func (c *ContadorAcessos) RegistrarLote(n int) {
	c.contador += n // incrementa o contador
}

func main() {
	c := &ContadorAcessos{}
	c.RegistrarHit()
	c.RegistrarLote(3)
}

// expect-line: 8 SLOP042
// expect-line: 13 SLOP042
