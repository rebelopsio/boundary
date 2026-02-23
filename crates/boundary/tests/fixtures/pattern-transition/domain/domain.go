package domain

// Order is a domain entity (no interfaces — anemic signal).
type Order struct {
	ID    string
	Total float64
}

// Customer is a domain entity.
type Customer struct {
	ID    string
	Email string
}

// Product is a domain entity.
type Product struct {
	ID    string
	Price float64
}
