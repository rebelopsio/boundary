package flat

// Product is a concrete CRUD entity.
type Product struct {
	ID    string
	Name  string
	Price float64
}

// Order is a concrete CRUD entity.
type Order struct {
	ID        string
	ProductID string
	Quantity  int
}

// Customer is a concrete CRUD entity.
type Customer struct {
	ID    string
	Email string
}
