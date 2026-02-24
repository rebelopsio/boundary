package domain

// Order is the order domain entity.
type Order struct {
	ID     string
	UserID string
	Total  float64
}

// OrderRepository is the order domain port.
type OrderRepository interface {
	FindByID(id string) (*Order, error)
}
