package main

func Handler(w int) {
}

type Service struct{}

func (s *Service) Process() {
}

// expect-line: 3 SLOP008
// expect-line: 8 SLOP008
