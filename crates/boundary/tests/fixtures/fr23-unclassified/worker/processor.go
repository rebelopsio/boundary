package worker

// Processor is an unclassified component (no layer pattern matches "worker/").
type Processor struct {
	Queue string
}

// Scheduler is also unclassified.
type Scheduler struct {
	Interval int
}
