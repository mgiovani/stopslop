package main

type HitCounter struct {
	count int
}

func (h *HitCounter) RecordHit() {
	// increment the count
	h.count += 1
}

func (h *HitCounter) RecordBatch(n int) {
	// increment the count
	h.count += n
}

func main() {
	h := &HitCounter{}
	h.RecordHit()
	h.RecordBatch(3)
}

// expect-line: 8 SLOP042
// expect-line: 13 SLOP042
