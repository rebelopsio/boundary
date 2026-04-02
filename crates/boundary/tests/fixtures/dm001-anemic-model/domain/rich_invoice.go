package domain

type Invoice struct {
	ID     string
	Total  float64
	Status string
}

func (i *Invoice) MarkPaid() error {
	return nil
}

func (i *Invoice) AddLineItem(item string) {
	// business logic
}
