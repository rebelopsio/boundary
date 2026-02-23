package domain

// Order is a data container with no behaviour.
type Order struct {
	ID       string
	Total    float64
	CustomerID string
}

// Customer is a data container with no behaviour.
type Customer struct {
	ID    string
	Email string
}
